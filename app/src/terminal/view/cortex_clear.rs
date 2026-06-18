//! Cortex-only: the per-pane "smart clear" button.
//!
//! One button, two behaviors:
//! - A CLI agent with a known clear command (currently Claude Code) is running
//!   in the pane → submit that command (`/clear`) to the agent's PTY, exactly
//!   as if the user typed it.
//! - Otherwise → clear the pane's blocks, same as the "Clear Blocks" action
//!   (Cmd/Ctrl-Shift-K).
//!
//! The button renders in two mutually exclusive placements:
//! - In the pane header, left of the overflow/close buttons, whenever the
//!   header renders (split panes, shared sessions, fullscreen agent view).
//! - As a floating overlay in the pane's top-right corner when the header is
//!   hidden (the default single-pane layout), following the find bar's
//!   `Align::top_right` pattern.

#[cfg(feature = "local_tty")]
use std::time::Duration;

use warp_core::ui::theme::Fill;
use warpui::elements::{Align, ConstrainedBox, Container};
use warpui::ui_components::components::UiComponent;
#[cfg(feature = "local_tty")]
use warpui::r#async::Timer;
use warpui::{AppContext, Element, SingletonEntity, ViewContext};

use crate::appearance::Appearance;
use crate::pane_group::pane::view::PaneHeaderAction;
use crate::pane_group::BackingView;
use crate::terminal::cli_agent_sessions::CLIAgentSessionsModel;
use crate::terminal::CLIAgent;
use crate::ui_components::buttons::icon_button_with_color;
use crate::ui_components::icons::Icon;

use super::{TerminalAction, TerminalView};

/// The slash command that clears a CLI agent's session context, per agent.
/// Agents without a mapping fall back to clearing the pane's blocks; future
/// agents (Gemini, Codex, …) are one match arm each.
fn cortex_agent_clear_command(agent: CLIAgent) -> Option<&'static str> {
    match agent {
        CLIAgent::Claude => Some("/clear"),
        _ => None,
    }
}

/// Delay between writing the agent's clear command and the carriage return
/// that submits it. Mirrors `CLI_AGENT_PTY_WRITE_DELAY` in `use_agent_footer`
/// (private there): every agent with a clear command (currently only Claude)
/// submits via the `DelayedEnter` strategy, where a too-rapid `\r` can beat
/// the agent's input layer and leave the command sitting unsubmitted.
#[cfg(feature = "local_tty")]
const CORTEX_CLEAR_PTY_WRITE_DELAY: Duration = Duration::from_millis(50);

impl TerminalView {
    /// Resolves the pane's CLI agent: the session entry can outlive the
    /// agent's block (same caveat as the dropped-image paste path in
    /// `use_agent_footer`), so require both a session agent AND an
    /// active+long-running foreground block before treating the pane as
    /// agent-controlled. The session lookup uses `app`, not the terminal model
    /// lock, so callers that already hold the model guard pass `long_running`
    /// in rather than re-locking.
    pub(crate) fn cortex_cli_agent(&self, app: &AppContext, long_running: bool) -> Option<CLIAgent> {
        let agent = CLIAgentSessionsModel::as_ref(app)
            .session(self.view_id)?
            .agent;
        long_running.then_some(agent)
    }

    /// The CLI agent currently running in this pane. Takes the terminal model
    /// lock, so call this ONLY where that lock is NOT already held — click
    /// handlers and the pane-header build (`render_header_actions`).
    ///
    /// NEVER call it from `TerminalView::render`, which holds
    /// `self.model.lock()` for its whole body: `self.model` is a non-reentrant
    /// `FairMutex`, so a second lock during render deadlocks the UI thread
    /// before first paint — the window then never uncloaks (invisible) and the
    /// hung thread can't process `WM_CLOSE` (unclosable). Inside render, read
    /// the long-running flag from the guard render already holds and call
    /// `cortex_cli_agent` instead. (This deadlock was the cause of the
    /// 2026-06-15 invisible-window incident.)
    pub(crate) fn cortex_active_cli_agent(&self, app: &AppContext) -> Option<CLIAgent> {
        let long_running = self
            .model
            .lock()
            .block_list()
            .active_block()
            .is_active_and_long_running();
        self.cortex_cli_agent(app, long_running)
    }

    /// Clears the pane: submits the agent's clear command when a CLI agent
    /// with a known one is running, otherwise clears the pane's blocks.
    pub(crate) fn cortex_smart_clear(&mut self, ctx: &mut ViewContext<Self>) {
        #[cfg(feature = "local_tty")]
        if let Some(agent) = self.cortex_active_cli_agent(ctx) {
            if let Some(command) = cortex_agent_clear_command(agent) {
                self.cortex_submit_clear_to_pty(agent, command, ctx);
                return;
            }
        }
        log::info!(
            "[cli-agent-clear] smart-clear on view {:?}: no CLI-agent clear command; \
             clearing pane blocks",
            self.view_id
        );
        self.clear_buffer(ctx);
    }

    /// Cortex-only: submit a CLI agent's clear command to its PTY with
    /// `[cli-agent-clear]` tracing, so a failed `/clear` — where the command
    /// lands in the agent's input but never executes (the delayed Enter is
    /// lost or eaten by the agent's slash-command autocomplete) — is visible
    /// in the runtime log and can be correlated against the hook bridge's
    /// `cortex-hook-discovery.log`.
    ///
    /// This reimplements the `DelayedEnter` arm of
    /// `write_cli_agent_text_then_submit` rather than calling
    /// `submit_text_to_cli_agent_pty`, for two reasons: that path is silent
    /// (no tracing), and its post-submit `maybe_close_rich_input_after_submit`
    /// is both private to `use_agent_footer` and a no-op here (the smart-clear
    /// button submits straight to the PTY without ever opening the Ctrl-G
    /// composer). Every agent with a clear command uses `DelayedEnter`, so no
    /// per-agent strategy switch is needed.
    #[cfg(feature = "local_tty")]
    fn cortex_submit_clear_to_pty(
        &mut self,
        agent: CLIAgent,
        command: &'static str,
        ctx: &mut ViewContext<Self>,
    ) {
        let view_id = self.view_id;
        // `is_agent_in_control()` is the exact predicate `write_user_bytes_to_pty`
        // early-returns on, so logging it tells us whether the text write below
        // will be suppressed (i.e. `/clear` never even appears in the input).
        let agent_in_control = self
            .model
            .lock()
            .block_list()
            .active_block()
            .is_agent_in_control();
        log::info!(
            "[cli-agent-clear] smart-clear on view {view_id:?}: submitting {command:?} to \
             {agent:?} PTY (text now, Enter in {}ms); agent_in_control={agent_in_control}",
            CORTEX_CLEAR_PTY_WRITE_DELAY.as_millis(),
        );

        self.write_user_bytes_to_pty(command.as_bytes().to_vec(), ctx);

        ctx.spawn(
            Timer::after(CORTEX_CLEAR_PTY_WRITE_DELAY),
            move |me, _, ctx| {
                let suppressed = me
                    .model
                    .lock()
                    .block_list()
                    .active_block()
                    .is_agent_in_control();
                if suppressed {
                    log::warn!(
                        "[cli-agent-clear] smart-clear on view {view_id:?}: delayed Enter \
                         SUPPRESSED — active block reports agent-in-control, so {command:?} \
                         will sit unsubmitted in the agent's input"
                    );
                } else {
                    log::info!(
                        "[cli-agent-clear] smart-clear on view {view_id:?}: firing delayed \
                         Enter for {command:?}"
                    );
                }
                me.write_user_bytes_to_pty(b"\r".to_vec(), ctx);
            },
        );
    }

    /// Tooltip for the smart-clear button. Takes the resolved `agent` (rather
    /// than re-resolving it) so it never touches the terminal model lock — see
    /// `cortex_active_cli_agent` for why that matters during render.
    fn cortex_clear_tooltip_text(&self, agent: Option<CLIAgent>) -> String {
        match agent.and_then(cortex_agent_clear_command) {
            Some(command) => format!("Clear agent session ({command})"),
            None => "Clear blocks".to_owned(),
        }
    }

    /// The smart-clear button for the pane header, rendered left of the
    /// overflow/close buttons. Routed through `PaneHeaderAction::CustomAction`
    /// like the neighboring cancel/details buttons in `pane_impl`.
    pub(crate) fn render_cortex_clear_header_button(
        &self,
        agent: Option<CLIAgent>,
        icon_color: Option<Fill>,
        button_size: Option<f32>,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let color = icon_color.unwrap_or_else(|| theme.sub_text_color(theme.background()));
        let ui_builder = appearance.ui_builder().clone();
        let tooltip_text = self.cortex_clear_tooltip_text(agent);

        let button = icon_button_with_color(
            appearance,
            Icon::SlashCircle,
            false, /* active */
            self.mouse_states.cortex_clear_header_button.clone(),
            color,
        )
        .with_tooltip(move || ui_builder.tool_tip(tooltip_text.clone()).build().finish())
        .build()
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action::<PaneHeaderAction<TerminalAction, TerminalAction>>(
                PaneHeaderAction::CustomAction(TerminalAction::CortexClearPane),
            );
        })
        .finish();

        if let Some(size) = button_size {
            ConstrainedBox::new(button)
                .with_width(size)
                .with_height(size)
                .finish()
        } else {
            button
        }
    }

    /// Whether the floating placement should render: only when the pane
    /// header (which hosts the button otherwise) is hidden, and the find bar
    /// isn't occupying the same top-right corner.
    pub(crate) fn cortex_should_show_clear_overlay(&self, app: &AppContext) -> bool {
        !BackingView::should_render_header(self, app)
            && !self.find_model.as_ref(app).is_find_bar_open()
    }

    /// The smart-clear button floating in the pane's top-right corner, for
    /// panes whose header doesn't render (the default single-pane layout).
    /// Dispatches `TerminalAction` directly since it lives in the terminal
    /// view's own element tree rather than the pane header's.
    pub(crate) fn render_cortex_clear_overlay(
        &self,
        agent: Option<CLIAgent>,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let ui_builder = appearance.ui_builder().clone();
        let tooltip_text = self.cortex_clear_tooltip_text(agent);

        let button = icon_button_with_color(
            appearance,
            Icon::SlashCircle,
            false, /* active */
            self.mouse_states.cortex_clear_overlay_button.clone(),
            theme.sub_text_color(theme.background()),
        )
        .with_tooltip(move || ui_builder.tool_tip(tooltip_text.clone()).build().finish())
        .build()
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(TerminalAction::CortexClearPane);
        })
        .finish();

        Align::new(
            Container::new(button)
                .with_padding_top(8.)
                .with_padding_right(16.)
                .finish(),
        )
        .top_right()
        .finish()
    }
}
