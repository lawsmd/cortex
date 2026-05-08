pub mod event;
pub mod listener;
#[cfg(not(target_family = "wasm"))]
pub(crate) mod plugin_manager;
// Cortex-only: bridge for vanilla `clauded` (no warp@claude-code-warp plugin).
// Installs a hook script into ~/.claude/settings.json that translates claude's
// hook lifecycle events to OSC 777 messages this model already understands.
// See docs/ai/external-status-injection.md.
#[cfg(not(target_family = "wasm"))]
pub mod cortex_claude_hooks;

use std::collections::{HashMap, HashSet};

use warpui::{Entity, EntityId, ModelContext, ModelHandle, SingletonEntity};

use crate::ai::blocklist::InputConfig;

use self::listener::CLIAgentSessionListener;
use super::CLIAgent;
use event::{CLIAgentEvent, CLIAgentEventType};

/// Status of a tracked CLI agent session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CLIAgentSessionStatus {
    InProgress,
    Success,
    Blocked { message: Option<String> },
}

impl CLIAgentSessionStatus {
    pub fn to_conversation_status(&self) -> crate::ai::agent::conversation::ConversationStatus {
        use crate::ai::agent::conversation::ConversationStatus;
        match self {
            CLIAgentSessionStatus::InProgress => ConversationStatus::InProgress,
            CLIAgentSessionStatus::Success => ConversationStatus::Success,
            CLIAgentSessionStatus::Blocked { message } => ConversationStatus::Blocked {
                blocked_action: message.clone().unwrap_or_default(),
            },
        }
    }
}

/// Rich context accumulated from CLI agent session events.
#[derive(Debug, Clone, Default)]
pub struct CLIAgentSessionContext {
    pub cwd: Option<String>,
    pub project: Option<String>,
    pub session_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input_preview: Option<String>,
    pub summary: Option<String>,
    pub query: Option<String>,
    pub response: Option<String>,
}

/// State of the rich input editor for composing a prompt to send to a CLI agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CLIAgentInputState {
    /// The rich input editor is not open.
    Closed,
    /// The rich input editor is open.
    Open {
        /// How this session was opened (for telemetry).
        entrypoint: CLIAgentInputEntrypoint,
        /// The input config that was active before opening rich input.
        previous_input_config: InputConfig,
        /// Whether the previous lock state was established while the input buffer was empty.
        previous_was_lock_set_with_empty_buffer: bool,
    },
}

/// Why the CLI agent rich input was closed (for telemetry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum CLIAgentRichInputCloseReason {
    /// User explicitly closed (Escape, Ctrl-G, footer button).
    Manual,
    /// Auto-closed due to agent status change (e.g. Blocked).
    AutoToggle,
    /// Auto-dismissed after submitting a prompt.
    Submit,
    /// Closed for another reason (chip removed, session ended, shared session sync).
    Other,
}

/// How a [`CLIAgentInputState`] was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum CLIAgentInputEntrypoint {
    /// User pressed Ctrl-G while a CLI agent was active.
    CtrlG,
    /// User clicked the rich input button in the CLI agent footer.
    FooterButton,
    /// Automatically opened when the CLI agent resumed work (left a blocked state)
    /// and the auto-show setting is enabled.
    AutoShow,
    /// Rich input was opened to mirror a shared-session participant's state.
    SharedSessionSync,
}

impl CLIAgentSessionContext {
    pub(crate) fn display_title(&self) -> Option<String> {
        self.latest_user_prompt().or_else(|| self.title_like_text())
    }

    pub(crate) fn latest_user_prompt(&self) -> Option<String> {
        self.query
            .as_deref()
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .map(str::to_owned)
    }

    /// Returns summary text suitable as a fallback title when no user prompt is available.
    pub(crate) fn title_like_text(&self) -> Option<String> {
        self.summary
            .as_deref()
            .map(str::trim)
            .filter(|summary| !summary.is_empty())
            .map(str::to_owned)
    }
}

/// A tracked CLI agent session.
#[derive(Debug, Clone)]
pub struct CLIAgentSession {
    pub agent: CLIAgent,
    pub status: CLIAgentSessionStatus,
    pub session_context: CLIAgentSessionContext,
    /// Rich input editor state.
    pub input_state: CLIAgentInputState,
    /// Whether status-driven auto-toggle is enabled for this session.
    pub should_auto_toggle_input: bool,
    /// Plugin-backed event listener, if the CLI agent plugin is installed.
    /// `None` for sessions created by command detection alone.
    /// Dropping this handle cleans up the listener's PTY event subscription.
    pub listener: Option<ModelHandle<CLIAgentSessionListener>>,
    /// The plugin version reported by the `SessionStart` event.
    /// `None` if the plugin predates version reporting or hasn't connected yet.
    pub plugin_version: Option<String>,
    /// `None` when the session is local.
    /// `Some("user@hostname")` when running over SSH (warpified or legacy).
    /// Used as a key for per-host plugin install failure tracking.
    pub remote_host: Option<String>,
    /// Draft text saved from the rich input composer when it was closed.
    /// Restored into the editor when the composer is reopened.
    pub draft_text: Option<String>,
    /// When the session was detected via a custom toolbar command pattern,
    /// the first word of the command (the binary/alias the user typed).
    /// Used to customize plugin instructions and force manual install mode.
    pub custom_command_prefix: Option<String>,
}

impl CLIAgentSession {
    pub fn is_remote(&self) -> bool {
        self.remote_host.is_some()
    }

    /// Clears state populated by `PermissionRequest`. Called whenever the
    /// session leaves the permission flow (the user replied, a new prompt
    /// is submitted, or the session ends successfully) so the permission
    /// summary doesn't leak into later UI surfaces — most visibly the tab
    /// title, which can fall back to `summary` when `query` is unset.
    fn clear_permission_scoped_state(&mut self) {
        self.session_context.summary = None;
        self.session_context.tool_name = None;
        self.session_context.tool_input_preview = None;
    }

    /// Applies an event to this session, updating context and status.
    /// Returns the new status if it changed, or `None` if the event was irrelevant.
    fn apply_event(&mut self, event: &CLIAgentEvent) -> Option<CLIAgentSessionStatus> {
        self.session_context.cwd = event.cwd.clone().or(self.session_context.cwd.take());
        self.session_context.project = event
            .project
            .clone()
            .or(self.session_context.project.take());
        self.session_context.session_id = event
            .session_id
            .clone()
            .or(self.session_context.session_id.take());

        let new_status = match &event.event {
            CLIAgentEventType::PromptSubmit => {
                self.session_context.query = event.payload.query.clone();
                self.session_context.response = None;
                self.clear_permission_scoped_state();
                CLIAgentSessionStatus::InProgress
            }
            CLIAgentEventType::ToolComplete => {
                if !matches!(self.status, CLIAgentSessionStatus::Blocked { .. }) {
                    return None;
                }
                CLIAgentSessionStatus::InProgress
            }
            CLIAgentEventType::Stop => {
                // Only update query if the Stop event explicitly carries one;
                // otherwise preserve the prior PromptSubmit's query. Hook-
                // bridged Stop events (cortex-hook.ps1) don't include a query
                // field — claude's Stop hook input doesn't surface it. The
                // tab-animation Tier 1 gate at app/src/tab.rs reads
                // session_context.query.is_some() as "the agent has been
                // prompted at least once" and uses it to discriminate against
                // bare command-detection sessions; clobbering it to None here
                // closes that gate after a successful turn-end and lets the
                // title heuristic falsely report Running indefinitely.
                if event.payload.query.is_some() {
                    self.session_context.query = event.payload.query.clone();
                }
                self.session_context.response = event.payload.response.clone();
                self.clear_permission_scoped_state();
                CLIAgentSessionStatus::Success
            }
            CLIAgentEventType::PermissionRequest => {
                self.session_context.summary = event.payload.summary.clone();
                self.session_context.tool_name = event.payload.tool_name.clone();
                self.session_context.tool_input_preview = event.payload.tool_input_preview.clone();
                CLIAgentSessionStatus::Blocked {
                    message: event.payload.summary.clone(),
                }
            }
            CLIAgentEventType::QuestionAsked => CLIAgentSessionStatus::Blocked {
                message: event
                    .payload
                    .summary
                    .clone()
                    .or_else(|| Some("Waiting for your answer".to_owned())),
            },
            CLIAgentEventType::PermissionReplied => {
                if !matches!(self.status, CLIAgentSessionStatus::Blocked { .. }) {
                    return None;
                }
                self.clear_permission_scoped_state();
                CLIAgentSessionStatus::InProgress
            }
            // IdlePrompt means the agent is sitting at its prompt waiting for input.
            // This should not affect status — otherwise it would override Success after a Stop event.
            CLIAgentEventType::IdlePrompt => return None,
            CLIAgentEventType::SessionStart => {
                self.plugin_version = event.payload.plugin_version.clone();
                return None;
            }
            CLIAgentEventType::Unknown(_) => return None,
        };

        self.status = new_status.clone();
        Some(new_status)
    }
}

/// Events emitted by `CLIAgentSessionsModel` for subscribers (e.g., `AgentNotificationsModel`).
#[allow(dead_code)] // `agent` fields on Started/InputSessionChanged/Ended are used for logging and future subscribers.
#[derive(Debug, Clone)]
pub enum CLIAgentSessionsModelEvent {
    Started {
        terminal_view_id: EntityId,
        agent: CLIAgent,
    },
    StatusChanged {
        terminal_view_id: EntityId,
        agent: CLIAgent,
        status: CLIAgentSessionStatus,
        session_context: Box<CLIAgentSessionContext>,
    },
    InputSessionChanged {
        terminal_view_id: EntityId,
        agent: CLIAgent,
        /// The input state BEFORE this change. When transitioning from
        /// `Open` → `Closed`, contains the saved input config to restore.
        previous_input_state: CLIAgentInputState,
        /// The input state AFTER this change.
        new_input_state: CLIAgentInputState,
    },
    Ended {
        terminal_view_id: EntityId,
        agent: CLIAgent,
    },
    /// The agent session has been updated. Subscribers may use this as a trigger for best-effort
    /// saving of state derived from the agent's session.
    SessionUpdated {
        terminal_view_id: EntityId,
        agent: CLIAgent,
    },
}

impl CLIAgentSessionsModelEvent {
    pub fn terminal_view_id(&self) -> EntityId {
        match self {
            CLIAgentSessionsModelEvent::Started {
                terminal_view_id, ..
            }
            | CLIAgentSessionsModelEvent::StatusChanged {
                terminal_view_id, ..
            }
            | CLIAgentSessionsModelEvent::InputSessionChanged {
                terminal_view_id, ..
            }
            | CLIAgentSessionsModelEvent::Ended {
                terminal_view_id, ..
            }
            | CLIAgentSessionsModelEvent::SessionUpdated {
                terminal_view_id, ..
            } => *terminal_view_id,
        }
    }
}

/// Singleton model that tracks pane-scoped CLI agent state and plugin-enriched session context.
pub struct CLIAgentSessionsModel {
    sessions: HashMap<EntityId, CLIAgentSession>,
    /// Tracks (agent, remote_host) pairs where an auto plugin operation (install or update) has failed.
    /// Shared across all views so failure in one tab is reflected everywhere.
    plugin_auto_failures: HashSet<(CLIAgent, Option<String>)>,
    /// Terminal views whose CLI-agent session ended on a non-active tab and
    /// that haven't been viewed since. Drives the AttentionNeeded breath
    /// pulse on completed-while-backgrounded panes; Tier 1's classifier
    /// (`app/src/tab.rs`) consults [`Self::is_attention_pending`] to upgrade
    /// `Success → AttentionNeeded` when set. Cleared by `mark_session_viewed`
    /// (called from the workspace's `PaneFocused` handler) and by a fresh
    /// `PromptSubmit` (the user re-engaged so the prior turn is acknowledged).
    attention_pending: HashSet<EntityId>,
}

impl Entity for CLIAgentSessionsModel {
    type Event = CLIAgentSessionsModelEvent;
}

impl SingletonEntity for CLIAgentSessionsModel {}

impl CLIAgentSessionsModel {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            plugin_auto_failures: HashSet::new(),
            attention_pending: HashSet::new(),
        }
    }

    pub fn session(&self, terminal_view_id: EntityId) -> Option<&CLIAgentSession> {
        self.sessions.get(&terminal_view_id)
    }

    /// Whether this session's most recent turn ended without the user having
    /// viewed the source pane since. Set on `Stop`/`SessionEnd` and cleared
    /// on `PromptSubmit` or via [`Self::mark_session_viewed`]. The Tier 1
    /// tab-animation classifier reads this to fire the AttentionNeeded breath
    /// pulse on completed-while-backgrounded panes.
    pub fn is_attention_pending(&self, terminal_view_id: EntityId) -> bool {
        self.attention_pending.contains(&terminal_view_id)
    }

    /// Clears the attention-pending flag for this session, if set. Called
    /// from the workspace's `PaneFocused` handler — reaching the pane is the
    /// "I've seen it" signal that satisfies the user-chosen
    /// pulse-until-viewed semantics. Notifies subscribers so the tab bar
    /// re-paints and drops the breath wash on the next frame.
    pub fn mark_session_viewed(
        &mut self,
        terminal_view_id: EntityId,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.attention_pending.remove(&terminal_view_id) {
            ctx.notify();
        }
    }

    /// Tab-level variant of [`Self::mark_session_viewed`]: clears the
    /// attention-pending flag for every view in the iterator (typically
    /// every terminal view in the active tab's pane group). Viewing a tab
    /// is, definitionally, viewing every pane it contains — the per-pane
    /// granularity of `mark_session_viewed` would otherwise leave the
    /// non-focused panes stuck pulsing even though the user is clearly
    /// looking at them.
    pub fn mark_sessions_viewed(
        &mut self,
        terminal_view_ids: impl IntoIterator<Item = EntityId>,
        ctx: &mut ModelContext<Self>,
    ) {
        let mut any = false;
        for id in terminal_view_ids {
            if self.attention_pending.remove(&id) {
                any = true;
            }
        }
        if any {
            ctx.notify();
        }
    }

    /// Eager status reset: typing into a pane that's stuck at `Blocked`
    /// means the user is replying to the permission/question prompt. The
    /// hook bridge (cortex-hook.{ps1,sh}) has no signal for
    /// `PermissionReplied`/`ToolComplete` (only plugin-attached sessions
    /// emit those), so without this, status stays `Blocked` from the moment
    /// the prompt is shown until the eventual `Stop` event — often 30+
    /// seconds, sometimes longer if claude immediately hits another
    /// permission prompt. Tier 1 reads `Blocked → AttentionNeeded` the
    /// whole time, even when claude is actively working again, which leaves
    /// the tab pulsing the breath wash through the entire post-approval
    /// run.
    ///
    /// Called from `write_user_bytes_to_pty` parallel to
    /// `mark_session_viewed`. Plan-mode approval and other in-TUI
    /// permission replies are keyboard-driven, so every byte the user types
    /// in response flows through that single funnel.
    pub fn mark_session_unblocked_by_input(
        &mut self,
        terminal_view_id: EntityId,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(session) = self.sessions.get_mut(&terminal_view_id) else {
            return;
        };
        if !matches!(session.status, CLIAgentSessionStatus::Blocked { .. }) {
            return;
        }
        session.status = CLIAgentSessionStatus::InProgress;
        let agent = session.agent;
        let session_context = session.session_context.clone();
        ctx.emit(CLIAgentSessionsModelEvent::StatusChanged {
            terminal_view_id,
            agent,
            status: CLIAgentSessionStatus::InProgress,
            session_context: Box::new(session_context),
        });
        ctx.notify();
    }

    /// Returns `true` if the rich input editor is currently open for this terminal.
    pub fn is_input_open(&self, terminal_view_id: EntityId) -> bool {
        self.sessions
            .get(&terminal_view_id)
            .is_some_and(|s| matches!(s.input_state, CLIAgentInputState::Open { .. }))
    }

    /// Registers a plugin-backed listener on the session for this terminal.
    ///
    /// If a session for the same agent already exists (e.g. created earlier by
    /// command detection), it is upgraded with the listener and plugin context.
    /// Otherwise a new session is created.
    ///
    /// The optional `cwd` / `project` / `session_id` fields supply initial
    /// context when available (e.g. from a `SessionStart` event). Passing
    /// `None` for all three is fine — happens when the plugin is installed
    /// mid-session and there is no start event to extract context from.
    #[allow(clippy::too_many_arguments)]
    pub fn register_listener(
        &mut self,
        terminal_view_id: EntityId,
        agent: CLIAgent,
        cwd: Option<String>,
        project: Option<String>,
        session_id: Option<String>,
        plugin_version: Option<String>,
        remote_host: Option<String>,
        should_auto_toggle_input: bool,
        listener: ModelHandle<CLIAgentSessionListener>,
        ctx: &mut ModelContext<Self>,
    ) {
        log::info!(
            target: "cli_agent_pipeline",
            "L5 register_listener terminal_view_id={terminal_view_id:?} agent={agent:?}"
        );
        if let Some(session) = self
            .sessions
            .get_mut(&terminal_view_id)
            .filter(|s| s.agent == agent)
        {
            // Upgrade existing session with plugin context.
            // Idle-seed status; see new-session-path comment below.
            session.status = CLIAgentSessionStatus::Success;
            session.listener = Some(listener);
            session.plugin_version = plugin_version;
            session.remote_host = remote_host;
            session.should_auto_toggle_input = should_auto_toggle_input;
            session.session_context.cwd = cwd.or(session.session_context.cwd.take());
            session.session_context.project = project.or(session.session_context.project.take());
            session.session_context.session_id =
                session_id.or(session.session_context.session_id.take());
            return;
        }

        self.set_session(
            terminal_view_id,
            CLIAgentSession {
                agent,
                // Idle seed. A plugin listener attaches on plugin handshake
                // (every `clauded` launch with the warp@claude-code-warp
                // plugin), which is NOT the same as "the agent has started a
                // turn." The first real PromptSubmit / PermissionRequest /
                // Stop overwrites this via apply_event. Seeding InProgress
                // was the source of the idle-launch comet bug: the Tier 1
                // gate at app/src/tab.rs trusts session.status whenever
                // listener.is_some() (the "bridged" escape hatch), so a
                // placeholder InProgress fired Running before the user typed
                // anything. Success + attention_pending=false yields None in
                // Tier 1 — correct idle. See docs/ai/external-status-injection.md
                // § Phase E.
                status: CLIAgentSessionStatus::Success,
                session_context: CLIAgentSessionContext {
                    cwd,
                    project,
                    session_id,
                    ..Default::default()
                },
                input_state: CLIAgentInputState::Closed,
                should_auto_toggle_input,
                listener: Some(listener),
                plugin_version,
                remote_host,
                draft_text: None,
                custom_command_prefix: None,
            },
            ctx,
        );
    }

    pub fn remove_session(&mut self, terminal_view_id: EntityId, ctx: &mut ModelContext<Self>) {
        if let Some(session) = self.sessions.remove(&terminal_view_id) {
            self.attention_pending.remove(&terminal_view_id);
            ctx.emit(CLIAgentSessionsModelEvent::Ended {
                terminal_view_id,
                agent: session.agent,
            });
        }
    }

    /// Updates the session's status and context from a parsed CLI agent event.
    pub fn update_from_event(
        &mut self,
        terminal_view_id: EntityId,
        event: &CLIAgentEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(session) = self.sessions.get_mut(&terminal_view_id) else {
            return;
        };

        let event_type = &event.event;
        if let Some(new_status) = session.apply_event(event) {
            let agent = session.agent;
            ctx.emit(CLIAgentSessionsModelEvent::StatusChanged {
                terminal_view_id,
                agent,
                status: new_status,
                session_context: Box::new(session.session_context.clone()),
            });
        }

        // Maintain the pulse-until-viewed set independently of status so
        // tests/state-machine consumers don't have to thread it through
        // CLIAgentSession itself. Stop arms the pulse; PromptSubmit clears
        // it (a fresh turn means the prior result has been acknowledged).
        match event_type {
            CLIAgentEventType::Stop => {
                self.attention_pending.insert(terminal_view_id);
            }
            CLIAgentEventType::PromptSubmit => {
                self.attention_pending.remove(&terminal_view_id);
            }
            _ => {}
        }

        if matches!(
            event_type,
            CLIAgentEventType::SessionStart
                | CLIAgentEventType::PromptSubmit
                | CLIAgentEventType::ToolComplete
        ) {
            ctx.emit(CLIAgentSessionsModelEvent::SessionUpdated {
                terminal_view_id,
                agent: session.agent,
            });
        }
    }

    pub fn open_input(
        &mut self,
        terminal_view_id: EntityId,
        entrypoint: CLIAgentInputEntrypoint,
        previous_input_config: InputConfig,
        previous_was_lock_set_with_empty_buffer: bool,
        should_auto_toggle_input: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(session) = self.sessions.get_mut(&terminal_view_id) else {
            return;
        };

        let previous_input_state = session.input_state;
        session.input_state = CLIAgentInputState::Open {
            entrypoint,
            previous_input_config,
            previous_was_lock_set_with_empty_buffer,
        };
        session.should_auto_toggle_input = should_auto_toggle_input;

        ctx.emit(CLIAgentSessionsModelEvent::InputSessionChanged {
            terminal_view_id,
            agent: session.agent,
            previous_input_state,
            new_input_state: session.input_state,
        });
    }

    pub fn close_input(
        &mut self,
        terminal_view_id: EntityId,
        should_auto_toggle_input: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(session) = self.sessions.get_mut(&terminal_view_id) else {
            return;
        };
        if session.input_state == CLIAgentInputState::Closed {
            return;
        }

        let previous_input_state = session.input_state;
        session.input_state = CLIAgentInputState::Closed;
        session.should_auto_toggle_input = should_auto_toggle_input;
        ctx.emit(CLIAgentSessionsModelEvent::InputSessionChanged {
            terminal_view_id,
            agent: session.agent,
            previous_input_state,
            new_input_state: CLIAgentInputState::Closed,
        });
    }

    pub fn set_session(
        &mut self,
        terminal_view_id: EntityId,
        session: CLIAgentSession,
        ctx: &mut ModelContext<Self>,
    ) {
        let agent = session.agent;
        // Close any open rich input before replacing, so subscribers can
        // restore input config before the session ends.
        self.close_input(terminal_view_id, false, ctx);
        if let Some(old) = self.sessions.insert(terminal_view_id, session) {
            ctx.emit(CLIAgentSessionsModelEvent::Ended {
                terminal_view_id,
                agent: old.agent,
            });
        }

        ctx.emit(CLIAgentSessionsModelEvent::Started {
            terminal_view_id,
            agent,
        });
    }

    /// Records that an auto plugin operation (install or update) failed for the given agent/host.
    /// `remote_host` is `None` for local sessions, `Some("user@hostname")` for remote.
    #[cfg(not(target_family = "wasm"))]
    pub fn record_plugin_auto_failure(&mut self, agent: CLIAgent, remote_host: Option<String>) {
        self.plugin_auto_failures.insert((agent, remote_host));
    }

    /// Saves draft text from the rich input composer for the given terminal.
    /// Stores `None` for empty or whitespace-only text.
    pub fn set_draft(&mut self, terminal_view_id: EntityId, text: String) {
        if let Some(session) = self.sessions.get_mut(&terminal_view_id) {
            session.draft_text = if text.trim().is_empty() {
                None
            } else {
                Some(text)
            };
        }
    }

    /// Clears any saved draft text for the given terminal.
    pub fn clear_draft(&mut self, terminal_view_id: EntityId) {
        if let Some(session) = self.sessions.get_mut(&terminal_view_id) {
            session.draft_text = None;
        }
    }

    /// Returns and clears the draft text for the given terminal, if any.
    pub fn take_draft(&mut self, terminal_view_id: EntityId) -> Option<String> {
        self.sessions
            .get_mut(&terminal_view_id)
            .and_then(|s| s.draft_text.take())
    }

    /// Whether an auto plugin operation has previously failed for this agent on this host.
    pub fn has_plugin_auto_failed(&self, agent: CLIAgent, remote_host: &Option<String>) -> bool {
        self.plugin_auto_failures
            .contains(&(agent, remote_host.clone()))
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
