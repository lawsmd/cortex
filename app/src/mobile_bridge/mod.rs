//! Cortex Mobile Companion bridge.
//!
//! Boots a small WebSocket server — axum on a private tokio runtime, mirroring
//! [`http_server::HttpServer`] — that a future Android companion app connects
//! to in order to mirror and control the user's open terminal panes from a
//! phone. The companion is reached over a Tailscale tailnet rather than the
//! public internet.
//!
//! **Milestone status (M4a — remote access):** gated on the
//! `cortex.mobile.enabled` setting, speaks the JSON protocol in [`protocol`],
//! and serves a self-contained xterm.js web client at `GET /` (so a phone needs
//! no app install — it just opens the URL). Capabilities:
//!   * `ListPanes` → `PaneList` — walk every window's tabs/panes, decorating
//!     each terminal with its CLI-agent name + status (M1).
//!   * `Subscribe` → an initial `Snapshot` (an ANSI redraw of the pane's current
//!     screen) followed by throttled `Snapshot` frames — the bridge re-serializes
//!     the pane's grid once per coalesced output burst (server-side reflow) rather
//!     than forwarding raw PTY bytes, so the phone, which reflows to its own width,
//!     never ghosts a desktop-width cursor redraw; `Unsubscribe` stops the stream
//!     (M2).
//!   * `Input` → write raw bytes into a pane's PTY via
//!     [`crate::terminal::view::TerminalView::write_viewer_bytes_to_pty`], the
//!     same funnel local keystrokes take, so it also clears the pane's
//!     attention/Blocked state when answering a CLI agent (M3).
//!   * `Paste` → paste a composed block of text (bracketed-paste aware, with an
//!     optional trailing Enter), backing the mobile client's "compose then send"
//!     box so multi-line prompts arrive as one paste rather than line-by-line.
//!
//! **Binding & auth.** The bridge always binds `127.0.0.1` (loopback, no token
//! — local clients keep working). When `cortex.mobile.bind_address` *and*
//! `cortex.mobile.token` are both set, it *additionally* binds that one address
//! (intended to be this machine's Tailscale `100.x` IP, so exposure is
//! tailnet-only, not the whole LAN) and requires the token as a `?token=` query
//! param on every connection to it. A bind address without a token is refused —
//! the bridge is never exposed unauthenticated. Token lives only in the local
//! (never-synced) settings; OS secure storage + QR pairing are M4b, along with
//! proactive `PaneList`/`PaneState` pushes and attention badges.
//!
//! **Threading.** The WebSocket tasks run on the bridge's private tokio runtime
//! and cannot touch pane/workspace state directly. Each request is handed to
//! the warpui main thread over an `async_channel` — the same cross-thread
//! hand-off the `/orchestrate` IPC bridge uses (`app/src/orchestrate/`). Unlike
//! M1's request/reply, a `Subscribe` produces an open-ended stream, so replies
//! flow back through a per-connection outbound queue drained by a writer task,
//! and a background task streams throttled screen snapshots. That task taps the
//! pane's existing `pty_reads` broadcast as a *change signal* (its bytes are
//! discarded) and re-serializes the grid — which is `Send + Sync` behind a
//! `FairMutex`, so the snapshot runs off the main thread. The initial snapshot
//! and the broadcast receiver are taken under one model lock, so the receiver's
//! first wake reflects only changes after that snapshot — no gap, no overlap.

mod protocol;
mod snapshot;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use futures::future::{select, AbortHandle, Either};
use futures::{SinkExt, StreamExt};
use parking_lot::FairMutex;
use warp_terminal::model::escape_sequences;
use warpui::r#async::Timer;
use warpui::{Entity, ModelContext, SingletonEntity, ViewHandle};

use crate::pane_group::pane::PaneId;
use crate::pane_group::{Direction, PaneGroup};
use crate::settings::CortexSettings;
use crate::terminal::cli_agent_sessions::{CLIAgentSessionStatus, CLIAgentSessionsModel};
use crate::terminal::{TerminalModel, TerminalView};
use crate::workspace::WorkspaceRegistry;

use protocol::{
    MobileRequest, MobileResponse, PaneEntry, ProjectEntry, TabEntry, WindowEntry,
};

/// Bound on a connection's outbound queue (snapshot/replies awaiting the socket
/// write). The reflow streamer uses a blocking send, so when a phone falls
/// behind this fills, the streamer stalls, the pane's `pty_reads` broadcast
/// overflows, and the next coalesced frame simply re-snapshots the current
/// screen — so a slow phone drops *intermediate* frames, never desyncs.
const OUTBOUND_CHANNEL_CAPACITY: usize = 512;

/// Primary-screen capture depth for the *streamed* reflow frames (the initial
/// subscribe uses the larger [`snapshot::MAX_PRIMARY_SNAPSHOT_ROWS`]). Smaller
/// than the initial frame because every burst re-sends the whole thing at up to
/// ~10 fps — enough to fill the phone viewport plus a little scrollback without
/// shipping 200 rows per tick.
const MAX_STREAM_SNAPSHOT_ROWS: usize = 120;

/// How often the writer task sends a WebSocket ping when the outbound queue is
/// idle. Browsers auto-reply with a pong, so this keeps the NAT/WireGuard path
/// warm and surfaces a dead socket within this window instead of never (axum
/// does not ping on its own). It does not, by itself, stop a backgrounded phone
/// tab from being suspended — the client's refocus-reconnect handles that.
const KEEPALIVE_INTERVAL_SECS: u64 = 25;

/// One unit of work handed from a WebSocket task (on the bridge's private tokio
/// runtime) to the warpui main thread. Mirrors `orchestrate`'s job hand-off,
/// but carries the connection's outbound sender instead of a one-shot reply,
/// because a single request may produce many response frames over time.
enum MobileJob {
    /// A parsed request to serve for connection `conn`, replying via `out`.
    Request {
        request: MobileRequest,
        conn: ConnectionId,
        out: async_channel::Sender<MobileResponse>,
    },
    /// Connection `conn` closed; tear down its live subscriptions.
    Disconnect { conn: ConnectionId },
}

/// Per-connection identity, assigned by the server as phones connect. Scopes
/// subscriptions so one phone's `Unsubscribe`/disconnect can't touch another's.
type ConnectionId = u64;

/// Shared axum state, cloned into every WebSocket connection. One value per
/// bound listener: the loopback listener gets `require_token = false`, the
/// remote (tailnet) listener gets `require_token = true`. Both share the same
/// request channel and connection-id counter so ids never collide.
#[derive(Clone)]
struct BridgeState {
    /// Sender half of the request channel onto the main thread.
    requests: async_channel::Sender<MobileJob>,
    /// Hands out a fresh [`ConnectionId`] per connection.
    next_conn_id: Arc<AtomicU64>,
    /// Whether this listener requires the auth token (true for the remote bind).
    require_token: bool,
    /// The configured shared secret (empty when none is set).
    token: Arc<str>,
}

/// Auth carried as a `?token=` query param on the WebSocket upgrade request.
#[derive(serde::Deserialize)]
struct AuthQuery {
    #[serde(default)]
    token: Option<String>,
}

/// Singleton entity owning the mobile companion server's private tokio runtime
/// and the set of live pane subscriptions.
pub struct MobileBridge {
    /// Held only to keep the server's runtime alive for the life of the app.
    _runtime: Option<tokio::runtime::Runtime>,
    /// Live PTY-mirror subscriptions keyed by (connection, pane). The
    /// [`AbortHandle`] cancels the background forwarder feeding that pane's
    /// output to a phone. Entries are dropped on `Unsubscribe` or disconnect.
    subscriptions: HashMap<(ConnectionId, PaneId), AbortHandle>,
}

impl MobileBridge {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let (enabled, port, token, bind_address) = {
            use settings::Setting;
            let cortex = CortexSettings::as_ref(ctx);
            (
                *cortex.mobile_server_enabled.value(),
                *cortex.mobile_server_port.value(),
                cortex.mobile_server_token.value().clone(),
                cortex.mobile_server_bind_address.value().clone(),
            )
        };

        if !enabled {
            log::info!("Cortex mobile bridge disabled (cortex.mobile.enabled = false).");
            return Self {
                _runtime: None,
                subscriptions: HashMap::new(),
            };
        }

        // WebSocket tasks → main thread request channel. The receiver is drained
        // on the warpui main thread below; the sender is cloned into axum state.
        let (requests_tx, requests_rx) = async_channel::unbounded::<MobileJob>();

        let runtime = Self::spawn_server(port, bind_address, token, requests_tx)
            .inspect_err(|err| {
                log::warn!("Failed to start Cortex mobile bridge server: {err:#}");
            })
            .ok();

        // Drain incoming jobs on the main thread, where pane/workspace state is
        // legal to touch. If the server failed to start, its sender was dropped,
        // the channel is closed, and this task simply ends.
        ctx.spawn_stream_local(
            requests_rx,
            |bridge, job, ctx| match job {
                MobileJob::Request { request, conn, out } => {
                    bridge.handle_request(request, conn, out, ctx);
                }
                MobileJob::Disconnect { conn } => bridge.drop_connection(conn),
            },
            |_, _| {},
        );

        Self {
            _runtime: runtime,
            subscriptions: HashMap::new(),
        }
    }

    fn spawn_server(
        port: u16,
        bind_address: String,
        token: String,
        requests: async_channel::Sender<MobileJob>,
    ) -> Result<tokio::runtime::Runtime, std::io::Error> {
        // Private runtime, matching `http_server`: we don't yet have a shared
        // tokio runtime to hang this off of. `enable_time` is required for the
        // per-connection keepalive `tokio::time::interval` (without it, the first
        // tick panics the writer task and no responses are ever sent).
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_io()
            .enable_time()
            .build()?;

        // Shared across every listener so connection ids never collide.
        let next_conn_id = Arc::new(AtomicU64::new(0));
        let token: Arc<str> = Arc::from(token.into_boxed_str());

        // Loopback: always bound, no token required — local clients (the test
        // page, the served UI opened on this machine) keep working unchanged.
        {
            let state = BridgeState {
                requests: requests.clone(),
                next_conn_id: next_conn_id.clone(),
                require_token: false,
                token: token.clone(),
            };
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            runtime.spawn(serve_on(addr, build_router(state), "loopback"));
        }

        // Remote: bound to one specific address so the port stays off every
        // other network (LAN, café Wi-Fi) — only the tailnet. The address is
        // either an explicit `cortex.mobile.bind_address` override, or (when
        // that's empty) auto-detected from this machine's Tailscale interface.
        // The phone reaches us by MagicDNS hostname, which always resolves to it.
        //
        // The bind is run by `serve_remote_retrying`, which re-resolves the
        // address and retries on a backoff until the bind succeeds — so a Cortex
        // launch that wins the race against Tailscale assigning this machine's
        // `100.x` IP (the one-shot bind would fail once with WSAEADDRNOTAVAIL /
        // os error 10049 and strand the phone on loopback-only) still ends up
        // listening once the address appears, with no restart needed.
        let bind_address = bind_address.trim().to_string();
        if token.is_empty() {
            // Never expose the bridge unauthenticated. Worth a warning only when
            // the user evidently intended remote access (a pinned address, or a
            // tailnet interface is already present).
            let intended = !bind_address.is_empty() || detect_tailscale_ipv4().is_some();
            if intended {
                log::warn!(
                    "Cortex mobile bridge: a remote bind address is available but \
                     cortex.mobile.token is empty; refusing to expose the bridge without auth. \
                     Set cortex.mobile.token to enable remote access."
                );
            }
        } else {
            // Parse the manual override once; empty or unparseable falls through
            // to auto-detect, which is re-evaluated on each bind attempt.
            let manual: Option<IpAddr> = if bind_address.is_empty() {
                None
            } else {
                match bind_address.parse::<IpAddr>() {
                    Ok(ip) => Some(ip),
                    Err(err) => {
                        log::warn!(
                            "Cortex mobile bridge: invalid cortex.mobile.bind_address \
                             {bind_address:?}: {err}; falling back to Tailscale auto-detect."
                        );
                        None
                    }
                }
            };
            let resolve = move || -> Option<(IpAddr, &'static str)> {
                match manual {
                    Some(ip) => Some((ip, "manual cortex.mobile.bind_address")),
                    None => {
                        detect_tailscale_ipv4().map(|ip| (ip, "auto-detected Tailscale interface"))
                    }
                }
            };
            let state = BridgeState {
                requests: requests.clone(),
                next_conn_id: next_conn_id.clone(),
                require_token: true,
                token: token.clone(),
            };
            runtime.spawn(serve_remote_retrying(
                port,
                resolve,
                build_router(state),
                "remote (token-gated)",
            ));
        }

        Ok(runtime)
    }

    // ── Main-thread request handling ────────────────────────────────────────

    /// Routes a parsed request on the warpui main thread. Each arm does its own
    /// state reads and replies through `out`.
    fn handle_request(
        &mut self,
        request: MobileRequest,
        conn: ConnectionId,
        out: async_channel::Sender<MobileResponse>,
        ctx: &mut ModelContext<Self>,
    ) {
        match request {
            MobileRequest::ListPanes => {
                let _ = out.try_send(list_panes(ctx));
            }
            MobileRequest::Subscribe { pane_id } => self.subscribe(pane_id, conn, out, ctx),
            MobileRequest::Unsubscribe { pane_id } => self.unsubscribe(pane_id, conn),
            MobileRequest::Input { pane_id, bytes_b64 } => inject_input(pane_id, bytes_b64, out, ctx),
            MobileRequest::Paste {
                pane_id,
                text,
                submit,
            } => paste_text(pane_id, text, submit, out, ctx),
            MobileRequest::NewTab => new_tab(out, ctx),
            MobileRequest::OpenProject { name } => open_project(name, out, ctx),
            MobileRequest::NewPane { tab_id } => new_pane(tab_id, out, ctx),
        }
    }

    /// Begin mirroring a pane: send a screen snapshot, then forward live output.
    fn subscribe(
        &mut self,
        pane_id_json: serde_json::Value,
        conn: ConnectionId,
        out: async_channel::Sender<MobileResponse>,
        ctx: &mut ModelContext<Self>,
    ) {
        // The opaque token round-trips back to the live PaneId.
        let pane_id: PaneId = match serde_json::from_value(pane_id_json.clone()) {
            Ok(id) => id,
            Err(err) => {
                let _ = out.try_send(MobileResponse::Error {
                    message: format!("invalid pane_id: {err}"),
                });
                return;
            }
        };

        let Some(view) = find_terminal_view(pane_id, ctx) else {
            let _ = out.try_send(MobileResponse::Error {
                message: "pane not found or is not a terminal".to_string(),
            });
            return;
        };
        let model = view.as_ref(ctx).model.clone();

        // Snapshot the screen and attach a live receiver under the SAME lock, so
        // the receiver's first wake observes only changes *after* this snapshot.
        let (snapshot, receiver) = {
            let model = model.lock();
            (
                snapshot::screen_snapshot(&model, snapshot::MAX_PRIMARY_SNAPSHOT_ROWS),
                model.event_proxy.cortex_new_pty_reads_receiver(),
            )
        };

        // Initial frame — a deeper snapshot for on-attach history. `streaming:
        // false` lands it in the client's primary buffer so its scrollback seeds
        // browsable history before live alt-buffer frames begin.
        let _ = out.try_send(MobileResponse::Snapshot {
            pane_id: pane_id_json.clone(),
            ansi_b64: b64(snapshot.ansi.as_bytes()),
            cols: snapshot.cols,
            rows: snapshot.rows,
            streaming: false,
        });

        // Then stream throttled reflow snapshots on a background thread; cancel
        // any prior subscription this connection had for the same pane.
        let (task, abort) = ctx.background_executor().spawn_abortable(
            forward_reflow_snapshots(
                model,
                receiver,
                out,
                pane_id_json,
                MAX_STREAM_SNAPSHOT_ROWS,
            ),
        );
        task.detach();
        if let Some(previous) = self.subscriptions.insert((conn, pane_id), abort) {
            previous.abort();
        }
    }

    /// Stop mirroring a pane for one connection.
    fn unsubscribe(&mut self, pane_id_json: serde_json::Value, conn: ConnectionId) {
        if let Ok(pane_id) = serde_json::from_value::<PaneId>(pane_id_json) {
            if let Some(abort) = self.subscriptions.remove(&(conn, pane_id)) {
                abort.abort();
            }
        }
    }

    /// Tear down every subscription belonging to a closed connection.
    fn drop_connection(&mut self, conn: ConnectionId) {
        self.subscriptions.retain(|(c, _), abort| {
            if *c == conn {
                abort.abort();
                false
            } else {
                true
            }
        });
    }
}

impl Entity for MobileBridge {
    type Event = ();
}

impl SingletonEntity for MobileBridge {}

// ── Main-thread helpers ──────────────────────────────────────────────────────

/// Walks every registered window → tab → pane and builds a [`MobileResponse::PaneList`].
fn list_panes(ctx: &mut ModelContext<MobileBridge>) -> MobileResponse {
    let workspaces = WorkspaceRegistry::as_ref(ctx).all_workspaces(ctx);
    let mut windows = Vec::with_capacity(workspaces.len());
    // Saved projects, loaded once for the whole tree walk: per-tab we match the
    // primary terminal's cwd against this list to surface the project NAME (the
    // phone's pane header shows "ProjectName: N"). The file is small and local,
    // so a fresh read per `list_panes` poll keeps newly-added projects current.
    let saved_projects = crate::saved_projects::load_projects();

    for (window_id, workspace) in workspaces {
        // Snapshot the per-tab pane-group handles + active index first, so we
        // don't hold a borrow of `workspace` across the pane reads below.
        let (groups, active_index): (Vec<(usize, ViewHandle<PaneGroup>)>, usize) = {
            let ws = workspace.as_ref(ctx);
            let active = ws.active_tab_index();
            let groups = (0..ws.tab_count())
                .filter_map(|i| ws.get_pane_group_view(i).cloned().map(|g| (i, g)))
                .collect();
            (groups, active)
        };

        let mut tabs = Vec::with_capacity(groups.len());
        for (index, group) in groups {
            let title = group.as_ref(ctx).display_title(ctx);
            // Cortex: the tab's saved-project accent (the same color the rounded
            // pane-border / header-tint features use), as `#rrggbb`, so the phone
            // sidebar can colour-code each project and its panes. `None` when the
            // tab maps to no project and has no manual/directory color. Read off
            // the shared focus state, where the workspace syncs the resolved color.
            let focus_state = group.as_ref(ctx).focus_state_handle();
            let color = focus_state
                .as_ref(ctx)
                .cortex_pane_border_color()
                .map(|fill| {
                    warp_core::ui::color::hex_color::coloru_to_hex_string(&fill.into_solid())
                });
            let pane_ids = group.as_ref(ctx).visible_pane_ids();

            // Resolve the tab's mapped Cortex project name from its first
            // terminal pane's working directory — same source the saved-project
            // color resolves from, just the name side of the same lookup.
            let project_name = pane_ids
                .iter()
                .find(|id| id.is_terminal_pane())
                .and_then(|id| group.as_ref(ctx).terminal_view_from_pane_id(*id, ctx))
                .and_then(|tv| tv.as_ref(ctx).pwd())
                .and_then(|cwd| {
                    crate::saved_projects::project_for_path(
                        std::path::Path::new(&cwd),
                        &saved_projects,
                    )
                    .map(|p| p.name.clone())
                });

            let mut panes = Vec::with_capacity(pane_ids.len());
            for pane_id in pane_ids {
                let kind = pane_id.pane_type().to_string();
                let (agent, status, cols, rows) = if pane_id.is_terminal_pane() {
                    let (agent, status) = pane_agent_status(&group, pane_id, ctx);
                    // The pane's current PTY grid size, so the phone can keep its
                    // xterm grid sized to match — Claude/Ink computes its redraw
                    // cursor moves from this width, so a mismatch ghosts the TUI.
                    let (cols, rows) = group
                        .as_ref(ctx)
                        .terminal_view_from_pane_id(pane_id, ctx)
                        .map(|tv| {
                            let m = tv.as_ref(ctx).model.lock();
                            let s = m.block_list().size();
                            (Some(s.columns()), Some(s.rows()))
                        })
                        .unwrap_or((None, None));
                    (agent, status, cols, rows)
                } else {
                    (None, None, None, None)
                };
                panes.push(PaneEntry {
                    pane_id: serde_json::to_value(pane_id).unwrap_or(serde_json::Value::Null),
                    kind,
                    agent,
                    status,
                    cols,
                    rows,
                });
            }

            tabs.push(TabEntry {
                tab_id: format!("{:?}", group.id()),
                title,
                active: index == active_index,
                color,
                project_name,
                panes,
            });
        }

        windows.push(WindowEntry {
            window_id: format!("{window_id:?}"),
            tabs,
        });
    }

    // The saved projects (loaded once above), surfaced to the phone's new-tab
    // picker as name + accent color.
    let projects = saved_projects
        .iter()
        .map(|p| ProjectEntry {
            name: p.name.clone(),
            color: p.color.clone(),
        })
        .collect();

    MobileResponse::PaneList { windows, projects }
}

/// Resolves the CLI-agent name + status for a terminal pane, if a session is
/// tracked for it. Keyed by the terminal view's id — the same key
/// [`CLIAgentSessionsModel`] uses internally.
fn pane_agent_status(
    group: &ViewHandle<PaneGroup>,
    pane_id: PaneId,
    ctx: &ModelContext<MobileBridge>,
) -> (Option<String>, Option<String>) {
    let Some(terminal_view) = group.as_ref(ctx).terminal_view_from_pane_id(pane_id, ctx) else {
        return (None, None);
    };
    match CLIAgentSessionsModel::as_ref(ctx).session(terminal_view.id()) {
        Some(session) => (
            Some(session.agent.display_name().to_string()),
            Some(status_str(&session.status).to_string()),
        ),
        None => (None, None),
    }
}

fn status_str(status: &CLIAgentSessionStatus) -> &'static str {
    match status {
        CLIAgentSessionStatus::InProgress => "in_progress",
        CLIAgentSessionStatus::Success => "success",
        CLIAgentSessionStatus::Blocked { .. } => "blocked",
    }
}

/// Finds the terminal view backing a pane, searching every window's tabs.
/// Returns `None` if the pane is gone or isn't a terminal.
fn find_terminal_view(
    pane_id: PaneId,
    ctx: &mut ModelContext<MobileBridge>,
) -> Option<ViewHandle<TerminalView>> {
    for (_window_id, workspace) in WorkspaceRegistry::as_ref(ctx).all_workspaces(ctx) {
        let groups: Vec<ViewHandle<PaneGroup>> = {
            let ws = workspace.as_ref(ctx);
            (0..ws.tab_count())
                .filter_map(|i| ws.get_pane_group_view(i).cloned())
                .collect()
        };
        for group in groups {
            if let Some(view) = group.as_ref(ctx).terminal_view_from_pane_id(pane_id, ctx) {
                return Some(view);
            }
        }
    }
    None
}

// ── Tab / pane creation (the mobile sidebar "+" affordances) ─────────────────

/// The first registered workspace (window). Single-window is the norm for this
/// personal tool; a multi-window setup picks the first deterministically.
fn first_workspace(
    ctx: &mut ModelContext<MobileBridge>,
) -> Option<ViewHandle<crate::workspace::Workspace>> {
    WorkspaceRegistry::as_ref(ctx)
        .all_workspaces(ctx)
        .into_iter()
        .next()
        .map(|(_window_id, workspace)| workspace)
}

/// Finds a tab's pane group by the opaque `tab_id` the phone received in a
/// `PaneList` (the debug form of the group's id — the same string `list_panes`
/// emits as `TabEntry::tab_id`).
fn find_pane_group_by_tab_id(
    tab_id: &str,
    ctx: &mut ModelContext<MobileBridge>,
) -> Option<ViewHandle<PaneGroup>> {
    for (_window_id, workspace) in WorkspaceRegistry::as_ref(ctx).all_workspaces(ctx) {
        let groups: Vec<ViewHandle<PaneGroup>> = {
            let ws = workspace.as_ref(ctx);
            (0..ws.tab_count())
                .filter_map(|i| ws.get_pane_group_view(i).cloned())
                .collect()
        };
        for group in groups {
            if format!("{:?}", group.id()) == tab_id {
                return Some(group);
            }
        }
    }
    None
}

/// Open a fresh blank terminal tab in the first window — the new-tab picker's
/// "New terminal" item. Produces no direct reply beyond pushing a refreshed
/// `PaneList` so the phone sees the tab without waiting for the next poll.
fn new_tab(out: async_channel::Sender<MobileResponse>, ctx: &mut ModelContext<MobileBridge>) {
    let Some(workspace) = first_workspace(ctx) else {
        let _ = out.try_send(MobileResponse::Error {
            message: "no open window to add a tab to".to_string(),
        });
        return;
    };
    workspace.update(ctx, |ws, ctx| ws.add_terminal_tab(false, ctx));
    let _ = out.try_send(list_panes(ctx));
}

/// Open a saved project (matched by name) as a new tab in its directory, tinted
/// with its project color — the same path the desktop's saved-project picker
/// takes. Reloads `projects.json` to resolve the cwd/color so no filesystem path
/// rides the wire.
fn open_project(
    name: String,
    out: async_channel::Sender<MobileResponse>,
    ctx: &mut ModelContext<MobileBridge>,
) {
    let Some(project) = crate::saved_projects::load_projects()
        .into_iter()
        .find(|p| p.name == name)
    else {
        let _ = out.try_send(MobileResponse::Error {
            message: format!("saved project not found: {name}"),
        });
        return;
    };
    let Some(workspace) = first_workspace(ctx) else {
        let _ = out.try_send(MobileResponse::Error {
            message: "no open window to add a tab to".to_string(),
        });
        return;
    };
    let crate::saved_projects::Project {
        name, cwd, color, ..
    } = project;
    workspace.update(ctx, |ws, ctx| {
        ws.open_saved_project_directory(cwd, name, &color, ctx);
    });
    let _ = out.try_send(list_panes(ctx));
}

/// Add a terminal pane (split right) to the tab identified by its opaque
/// `tab_id` — the per-tab "+" square in the mobile sidebar. Mirrors the desktop
/// split-right action; the new pane uses the default shell.
fn new_pane(
    tab_id: String,
    out: async_channel::Sender<MobileResponse>,
    ctx: &mut ModelContext<MobileBridge>,
) {
    let Some(group) = find_pane_group_by_tab_id(&tab_id, ctx) else {
        let _ = out.try_send(MobileResponse::Error {
            message: "tab not found".to_string(),
        });
        return;
    };
    group.update(ctx, |group, ctx| {
        group.add_terminal_pane(Direction::Right, None, ctx);
    });
    let _ = out.try_send(list_panes(ctx));
}

/// Decode base64 input and write it into a pane's PTY on the main thread, as if
/// the user typed it locally. `write_viewer_bytes_to_pty` routes through the
/// same funnel as keyboard input — including the attention/Blocked → InProgress
/// reset — so answering a Claude prompt from the phone registers as a reply,
/// exactly like a local keystroke. Input produces no direct response; its effect
/// comes back as `Output` once the PTY echoes. Failures (bad base64, unknown or
/// non-terminal pane) are reported back as an `Error` frame.
fn inject_input(
    pane_id_json: serde_json::Value,
    bytes_b64: String,
    out: async_channel::Sender<MobileResponse>,
    ctx: &mut ModelContext<MobileBridge>,
) {
    let bytes = match BASE64.decode(bytes_b64.as_bytes()) {
        Ok(bytes) => bytes,
        Err(err) => {
            let _ = out.try_send(MobileResponse::Error {
                message: format!("invalid input base64: {err}"),
            });
            return;
        }
    };

    let pane_id: PaneId = match serde_json::from_value(pane_id_json) {
        Ok(id) => id,
        Err(err) => {
            let _ = out.try_send(MobileResponse::Error {
                message: format!("invalid pane_id: {err}"),
            });
            return;
        }
    };

    let Some(view) = find_terminal_view(pane_id, ctx) else {
        let _ = out.try_send(MobileResponse::Error {
            message: "pane not found or is not a terminal".to_string(),
        });
        return;
    };

    view.update(ctx, |view, ctx| {
        view.write_viewer_bytes_to_pty(bytes, ctx);
    });
}

/// Paste a composed block of `text` into a pane, mirroring the desktop's
/// `Ctrl+V`: newlines are normalized to `\r`, and the text is wrapped in
/// bracketed-paste markers **only when the focused app has bracketed paste
/// enabled** — so multi-line content (a prompt, a code block) lands in the input
/// buffer as one paste instead of executing line-by-line. When `submit` is set,
/// a trailing `\r` is appended *after* the closing marker, so the receiving app
/// sees "paste finished, then Enter" — the phone's one-tap "Send". Writes through
/// the same `write_viewer_bytes_to_pty` funnel as `Input`, so it also clears a
/// Claude pane's Blocked → InProgress attention state. Failures (bad or non-
/// terminal pane) come back as an `Error` frame.
fn paste_text(
    pane_id_json: serde_json::Value,
    text: String,
    submit: bool,
    out: async_channel::Sender<MobileResponse>,
    ctx: &mut ModelContext<MobileBridge>,
) {
    let pane_id: PaneId = match serde_json::from_value(pane_id_json) {
        Ok(id) => id,
        Err(err) => {
            let _ = out.try_send(MobileResponse::Error {
                message: format!("invalid pane_id: {err}"),
            });
            return;
        }
    };

    let Some(view) = find_terminal_view(pane_id, ctx) else {
        let _ = out.try_send(MobileResponse::Error {
            message: "pane not found or is not a terminal".to_string(),
        });
        return;
    };

    view.update(ctx, |view, ctx| {
        // Whether to bracket-wrap depends on the focused app's *current* mode, so
        // read it under the model lock — then drop the guard before writing, as
        // `write_viewer_bytes_to_pty` locks the model itself.
        let bracketed = {
            let mut model = view.model.lock();
            model.needs_bracketed_paste()
        };
        let bytes = build_paste_bytes(&text, submit, bracketed);
        view.write_viewer_bytes_to_pty(bytes, ctx);
    });
}

/// Build the byte sequence for a paste: newlines normalized to `\r`, optionally
/// surrounded by bracketed-paste markers, with an optional trailing `\r` (after
/// the closing marker) to submit. Mirrors `TerminalView::paste`'s handling.
fn build_paste_bytes(text: &str, submit: bool, bracketed: bool) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\r").replace('\n', "\r");
    let mut out = Vec::with_capacity(normalized.len() + 16);
    if bracketed {
        out.extend_from_slice(escape_sequences::BRACKETED_PASTE_START);
        out.extend_from_slice(normalized.as_bytes());
        out.extend_from_slice(escape_sequences::BRACKETED_PASTE_END);
    } else {
        out.extend_from_slice(normalized.as_bytes());
    }
    if submit {
        out.push(b'\r');
    }
    out
}

/// Base64-encode bytes for a JSON text frame.
fn b64(bytes: &[u8]) -> String {
    BASE64.encode(bytes)
}

// ── Background reflow-snapshot streaming ─────────────────────────────────────

/// Streams a pane's screen to a connection as throttled, reflow-friendly
/// [`MobileResponse::Snapshot`] frames.
///
/// Forwarding *raw* PTY bytes desyncs the mirror: Claude/Ink TUIs redraw their
/// bottom region with cursor moves computed for the desktop's wide grid, and
/// when the phone reflows them to its own narrow width those moves land in the
/// wrong cells and ghost. Instead, this re-serializes the pane's *current* grid
/// — which xterm repaints cleanly at any width — once per coalesced output
/// burst, capped at `FRAME_MIN_INTERVAL`. The pane's `pty_reads` broadcast is
/// used only as a change signal (and to keep `receiver_count()` non-zero so the
/// pane keeps broadcasting); its bytes are discarded.
///
/// Ends when the pane closes, the outbound queue closes (phone gone), or the
/// subscription is aborted. Runs on the background executor and touches only the
/// `Send + Sync` model mutex — never main-thread state.
async fn forward_reflow_snapshots(
    model: Arc<FairMutex<TerminalModel>>,
    mut receiver: async_broadcast::Receiver<Arc<Vec<u8>>>,
    out: async_channel::Sender<MobileResponse>,
    pane_id: serde_json::Value,
    max_rows: usize,
) {
    // ~10 fps ceiling: a sustained TUI animation (Claude's spinner, /compact)
    // yields at most one full-frame repaint per tick, coalescing every burst in
    // between. Lower it (raise fps) if updates feel choppy; raise it to cut
    // bandwidth/phone CPU during heavy animation.
    const FRAME_MIN_INTERVAL: Duration = Duration::from_millis(100);

    // After a burst goes this quiet, emit one `streaming: false` *settle* frame:
    // a deep snapshot the client lands in its primary buffer so scrollback holds
    // browsable history (it also drops the client out of the alt buffer). Kept
    // comfortably longer than Claude's sub-second inter-token gaps so a normal
    // streaming pause doesn't thrash the alt↔primary switch.
    const SETTLE_IDLE: Duration = Duration::from_millis(700);

    // True once we've sent at least one live (alt-buffer) frame since the last
    // settle. While true we only wait up to SETTLE_IDLE for more output before
    // settling; while false we block indefinitely for the next burst.
    let mut streaming = false;

    loop {
        // Wait for the next output. Mid-burst, race the wait against the settle
        // timer so a lull produces one history frame and then goes quiet.
        if streaming {
            let recv_fut = receiver.recv();
            let settle = Timer::after(SETTLE_IDLE);
            futures::pin_mut!(recv_fut, settle);
            match select(recv_fut, settle).await {
                // More output (or a benign overflow) → fall through and repaint.
                Either::Left((Ok(_), _)) => {}
                Either::Left((Err(async_broadcast::RecvError::Overflowed(_)), _)) => {}
                Either::Left((Err(async_broadcast::RecvError::Closed), _)) => break,
                // Quiet for SETTLE_IDLE → settle: a deep primary-buffer snapshot
                // that also pops the client out of the alt buffer.
                Either::Right((_, _)) => {
                    let frame = {
                        let model = model.lock();
                        let snap =
                            snapshot::screen_snapshot(&model, snapshot::MAX_PRIMARY_SNAPSHOT_ROWS);
                        MobileResponse::Snapshot {
                            pane_id: pane_id.clone(),
                            ansi_b64: b64(snap.ansi.as_bytes()),
                            cols: snap.cols,
                            rows: snap.rows,
                            streaming: false,
                        }
                    };
                    if out.send(frame).await.is_err() {
                        return; // phone gone
                    }
                    streaming = false;
                    continue;
                }
            }
        } else {
            // Idle-sleep until the pane produces output worth repainting.
            match receiver.recv().await {
                Ok(_) => {}
                // A full re-snapshot supersedes any dropped chunk, so overflow is
                // benign — just render the current screen.
                Err(async_broadcast::RecvError::Overflowed(_)) => {}
                Err(async_broadcast::RecvError::Closed) => break,
            }
        }

        // Coalesce everything already queued into this single frame.
        while receiver.try_recv().is_ok() {}

        // Re-serialize the authoritative grid into a live (alt-buffer) frame. The
        // lock is released at the end of this block — never held across an await.
        let frame = {
            let model = model.lock();
            let snap = snapshot::screen_snapshot(&model, max_rows);
            MobileResponse::Snapshot {
                pane_id: pane_id.clone(),
                ansi_b64: b64(snap.ansi.as_bytes()),
                cols: snap.cols,
                rows: snap.rows,
                streaming: true,
            }
        };
        if out.send(frame).await.is_err() {
            return; // phone gone
        }
        streaming = true;

        // Rate-cap before looping back to wait for the next burst.
        Timer::after(FRAME_MIN_INTERVAL).await;
    }
}

// ── WebSocket transport (tokio runtime side) ────────────────────────────────

/// The self-contained xterm.js web client served at `GET /`, embedded in the
/// binary so a phone needs no app install and the build doesn't depend on any
/// file outside the crate.
const CLIENT_HTML: &str = include_str!("client.html");

/// xterm.js (5.5.0) vendored locally and served at `/xterm.js` + `/xterm.css`
/// (referenced by the client with relative paths). Killing the CDN dependency
/// makes the client work offline and lets the APK bundle these as sibling
/// assets — a prerequisite for the native `file://` WebView shell.
const XTERM_JS: &str = include_str!("vendor/xterm.js");
const XTERM_CSS: &str = include_str!("vendor/xterm.css");

/// PWA icon served at `/icon.png` — the same Cortex icon the macOS bundle uses.
const ICON_PNG: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/channels/oss/icon/no-padding/512x512.png"));

/// Web-app manifest served at `/manifest.webmanifest`. Makes the served client
/// installable ("Add to Home Screen") as a chrome-less standalone app; the
/// native APK supplies its own manifest, so this is for the install-as-PWA path.
const MANIFEST_JSON: &str = r##"{
  "name": "Cortex Mobile",
  "short_name": "Cortex",
  "description": "Mirror and control your Cortex terminal panes.",
  "display": "standalone",
  "orientation": "any",
  "background_color": "#0d0f12",
  "theme_color": "#0d0f12",
  "start_url": "./",
  "scope": "./",
  "icons": [
    { "src": "./icon.png", "sizes": "512x512", "type": "image/png", "purpose": "any maskable" }
  ]
}"##;

/// Builds the route table for one listener. `/` serves the web client; `/ws`
/// is the protocol endpoint; the static routes serve the vendored xterm assets,
/// the PWA manifest, and the icon. Auth (if any) is carried in `state`.
fn build_router(state: BridgeState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .route("/xterm.js", get(xterm_js))
        .route("/xterm.css", get(xterm_css))
        .route("/manifest.webmanifest", get(manifest))
        .route("/icon.png", get(icon_png))
        .with_state(state)
}

/// Finds this machine's Tailscale IPv4, if any, by scanning local interfaces
/// for an address in the Tailscale CGNAT range `100.64.0.0/10`
/// (100.64.0.0 – 100.127.255.255). Returns the first match. Used to bind the
/// remote listener without a hardcoded IP, so the bridge re-finds its tailnet
/// address each launch even when it changes.
fn detect_tailscale_ipv4() -> Option<IpAddr> {
    let ifaces = if_addrs::get_if_addrs()
        .inspect_err(|err| {
            log::warn!("Cortex mobile bridge: could not enumerate interfaces: {err:#}");
        })
        .ok()?;
    ifaces.into_iter().find_map(|iface| match iface.addr {
        if_addrs::IfAddr::V4(v4) => {
            let o = v4.ip.octets();
            (o[0] == 100 && (64..=127).contains(&o[1])).then(|| IpAddr::V4(v4.ip))
        }
        _ => None,
    })
}

/// Binds `addr` and serves `router` until the runtime shuts down. `label`
/// distinguishes loopback vs. remote in the log. Loopback always binds on the
/// first try, so this one-shot form is used only there; the remote listener
/// uses [`serve_remote_retrying`].
async fn serve_on(addr: SocketAddr, router: Router, label: &'static str) {
    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => {
            log::info!("Cortex mobile bridge listening on http://{addr}/ (ws://{addr}/ws) [{label}]");
            if let Err(err) = axum::serve(listener, router).await {
                log::warn!("Cortex mobile bridge server stopped [{label}]: {err:#}");
            }
        }
        Err(err) => {
            log::warn!("Cortex mobile bridge failed to bind {addr} [{label}]: {err:#}");
        }
    }
}

/// Like [`serve_on`], but for the remote (tailnet) listener: re-resolves the
/// target address and retries the bind on a capped backoff until it succeeds,
/// then serves until shutdown. This makes the bridge self-healing against a
/// boot-time race where Cortex launches before Tailscale has assigned this
/// machine's `100.x` address — the one-shot bind would fail with
/// `WSAEADDRNOTAVAIL` (os error 10049) and strand the phone on loopback-only for
/// the whole session. `resolve` is re-run on every attempt so the auto-detect
/// path picks up a late-appearing interface (the manual-override path just
/// returns its fixed address). Failures are logged once per streak (reset on a
/// successful bind) so a never-up Tailscale doesn't spam the log. If the served
/// listener ever returns (interface dropped mid-session), the loop re-establishes
/// it; on app exit the whole runtime is torn down, ending this task.
async fn serve_remote_retrying(
    port: u16,
    resolve: impl Fn() -> Option<(IpAddr, &'static str)> + Send + 'static,
    router: Router,
    label: &'static str,
) {
    use std::time::Duration;
    const MAX_BACKOFF: Duration = Duration::from_secs(30);
    let mut backoff = Duration::from_secs(1);
    let mut warned = false;
    loop {
        match resolve() {
            Some((ip, source)) => {
                let addr = SocketAddr::new(ip, port);
                match tokio::net::TcpListener::bind(addr).await {
                    Ok(listener) => {
                        log::info!(
                            "Cortex mobile bridge listening on http://{addr}/ (ws://{addr}/ws) \
                             [{label}] ({source})"
                        );
                        backoff = Duration::from_secs(1);
                        warned = false;
                        if let Err(err) = axum::serve(listener, router.clone()).await {
                            log::warn!("Cortex mobile bridge server stopped [{label}]: {err:#}");
                        }
                        // Served then returned — re-establish on the next loop.
                        continue;
                    }
                    Err(err) => {
                        if !warned {
                            log::warn!(
                                "Cortex mobile bridge: cannot bind {addr} [{label}] yet ({err}); \
                                 retrying until it's available (often a boot-time race with \
                                 Tailscale assigning {ip})."
                            );
                            warned = true;
                        }
                    }
                }
            }
            None => {
                if !warned {
                    log::info!(
                        "Cortex mobile bridge: no remote bind address available yet [{label}]; \
                         waiting for the Tailscale interface (retrying)."
                    );
                    warned = true;
                }
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

async fn index() -> Html<String> {
    Html(client_html())
}

/// Static asset handlers. Each pins an explicit content-type so browsers parse
/// the vendored JS/CSS, the manifest, and the icon correctly.
async fn xterm_js() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        XTERM_JS,
    )
}
async fn xterm_css() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
        XTERM_CSS,
    )
}
async fn manifest() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/manifest+json")],
        MANIFEST_JSON,
    )
}
async fn icon_png() -> impl IntoResponse {
    ([(axum::http::header::CONTENT_TYPE, "image/png")], ICON_PNG)
}

/// Release: the embedded client. Debug: read `client.html` from the source tree
/// on every request, so mobile-UI tweaks show up with just a browser refresh —
/// no rebuild — while iterating on the phone. Falls back to the embedded copy if
/// the source file can't be read.
#[cfg(debug_assertions)]
fn client_html() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/mobile_bridge/client.html");
    std::fs::read_to_string(path).unwrap_or_else(|_| CLIENT_HTML.to_string())
}

#[cfg(not(debug_assertions))]
fn client_html() -> String {
    CLIENT_HTML.to_string()
}

async fn ws_handler(
    State(state): State<BridgeState>,
    Query(auth): Query<AuthQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    if state.require_token {
        let authorized =
            !state.token.is_empty() && auth.token.as_deref() == Some(&*state.token);
        if !authorized {
            log::warn!(
                "Cortex mobile bridge: rejected a remote connection with a missing/invalid token."
            );
            return (StatusCode::UNAUTHORIZED, "invalid or missing token").into_response();
        }
    }
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Per-connection loop. The socket is split: a writer task drains the outbound
/// queue (request replies, snapshots, and live output) to the wire, while the
/// reader dispatches each inbound JSON request onto the main thread.
async fn handle_socket(socket: WebSocket, state: BridgeState) {
    let conn = state.next_conn_id.fetch_add(1, Ordering::Relaxed);
    log::info!("Cortex mobile bridge: client {conn} connected.");

    let (mut sink, mut stream) = socket.split();
    let (out_tx, out_rx) = async_channel::bounded::<MobileResponse>(OUTBOUND_CHANNEL_CAPACITY);

    // Writer task: serialize each queued response and write it to the socket,
    // and emit a keepalive ping whenever the queue goes idle (so the connection
    // stays warm and a dead socket is noticed promptly).
    let writer = tokio::spawn(async move {
        let mut keepalive =
            tokio::time::interval(std::time::Duration::from_secs(KEEPALIVE_INTERVAL_SECS));
        keepalive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        keepalive.tick().await; // consume the immediate first tick
        loop {
            tokio::select! {
                recv = out_rx.recv() => {
                    let Ok(response) = recv else { break }; // channel closed
                    let payload = serde_json::to_string(&response).unwrap_or_else(|err| {
                        format!("{{\"type\":\"error\",\"message\":\"failed to serialize response: {err}\"}}")
                    });
                    if sink.send(Message::Text(payload.into())).await.is_err() {
                        break;
                    }
                }
                _ = keepalive.tick() => {
                    if sink.send(Message::Ping(Default::default())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Reader loop: each inbound text frame is a JSON request dispatched to the
    // main thread, which replies/streams back through `out_tx`.
    while let Some(result) = stream.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(err) => {
                log::info!("Cortex mobile bridge: client {conn} recv error: {err:#}");
                break;
            }
        };
        match msg {
            Message::Text(text) => dispatch_text(text.as_str(), conn, &out_tx, &state).await,
            Message::Close(_) => break,
            // Input rides in as a Text `input` frame (dispatched above). Raw
            // binary frames are unused; ping/pong are handled by axum below us.
            Message::Binary(_) | Message::Ping(_) | Message::Pong(_) => {}
        }
    }

    // Tear down: drop this connection's subscriptions on the main thread, then
    // stop the writer. Forwarders also self-terminate once `out_tx` is gone.
    let _ = state.requests.send(MobileJob::Disconnect { conn }).await;
    drop(out_tx);
    writer.abort();
    log::info!("Cortex mobile bridge: client {conn} disconnected.");
}

/// Parses one text frame and dispatches it onto the main thread. Replies (and
/// any later streamed frames) come back through `out`, so this doesn't await a
/// result — it only reports parse failures and a dead dispatcher inline.
async fn dispatch_text(
    text: &str,
    conn: ConnectionId,
    out: &async_channel::Sender<MobileResponse>,
    state: &BridgeState,
) {
    let request = match serde_json::from_str::<MobileRequest>(text) {
        Ok(request) => request,
        Err(err) => {
            let _ = out
                .send(MobileResponse::Error {
                    message: format!("invalid request: {err}"),
                })
                .await;
            return;
        }
    };

    let job = MobileJob::Request {
        request,
        conn,
        out: out.clone(),
    };
    if state.requests.send(job).await.is_err() {
        let _ = out
            .send(MobileResponse::Error {
                message: "mobile bridge dispatcher is not running".to_string(),
            })
            .await;
    }
}
