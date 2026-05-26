//! Cortex `/orchestrate` IPC bridge.
//!
//! Boots a per-process [`ipc::Server`] exposing [`service::OrchestrateService`],
//! stashes the socket path in a process-global cell that the terminal
//! env-var injector (Phase 4) reads as `CORTEX_IPC_SOCKET`, and dispatches
//! each request onto the main UI thread to split the active tab's pane
//! group into N panes — each seeded with `claude --permission-mode plan`
//! (or `--dangerously-skip-permissions`, gated by the AI settings toggle
//! introduced in Phase 2).
//!
//! Lifted from the `single_instance_manager.rs` (UriService) bridge
//! template, with the cross-platform unix-socket transport and a
//! main-thread dispatcher in place of the Windows-only URI handler.

use std::sync::OnceLock;

use ipc::ServerBuilder;
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::pane_group::pane::{build_local_claude_child_command, ClaudePermissionMode};
use crate::pane_group::Direction;
use crate::root_view::active_workspace;
use crate::settings::CortexSettings;

mod cli;
mod service;
mod service_impl;

pub use cli::run_cli;
pub use service::{OrchestrateRequest, OrchestrateResponse, OrchestrateService};

use service_impl::OrchestrateServiceImpl;

/// Environment variable that carries the orchestrate IPC socket path
/// from the Cortex app to every spawned terminal. Read by the
/// `cortex orchestrate` CLI handler to locate the running service.
pub const CORTEX_IPC_SOCKET_ENV: &str = "CORTEX_IPC_SOCKET";

/// Process-global socket path of the orchestrate IPC server, populated by
/// [`OrchestrateBridge::new`] at app startup. Phase 4's terminal env-var
/// injector reads it via [`orchestrate_ipc_socket_path`] and threads the
/// value into every spawned local terminal as `CORTEX_IPC_SOCKET`.
static ORCHESTRATE_IPC_SOCKET_PATH: OnceLock<String> = OnceLock::new();

/// Returns the orchestrate IPC server's socket path, if the bridge has
/// successfully booted. Returns `None` if `add_singleton_model` was never
/// called or if the server failed to start.
pub fn orchestrate_ipc_socket_path() -> Option<&'static str> {
    ORCHESTRATE_IPC_SOCKET_PATH.get().map(String::as_str)
}

/// Singleton entity that owns the orchestrate IPC server handle and the
/// main-thread stream task that processes incoming requests. Dropping
/// this entity tears down the server.
pub struct OrchestrateBridge {
    _server: Option<ipc::Server>,
}

impl OrchestrateBridge {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let (tx, rx) = async_channel::unbounded();
        let build_result = ServerBuilder::default()
            .with_service(OrchestrateServiceImpl::new(tx))
            .build_and_run(ctx.background_executor());

        let server = match build_result {
            Ok((server, connection_address)) => {
                let socket_path = connection_address.to_string();
                if let Err(existing) = ORCHESTRATE_IPC_SOCKET_PATH.set(socket_path.clone()) {
                    log::warn!(
                        "OrchestrateBridge initialized twice; keeping existing socket {existing} \
                         and dropping new one at {socket_path}"
                    );
                }
                ctx.spawn_stream_local(
                    rx,
                    |_bridge, (request, reply), ctx| {
                        let response = handle_orchestrate_request(request, ctx);
                        if reply.send(response).is_err() {
                            log::warn!(
                                "Orchestrate IPC caller hung up before receiving reply."
                            );
                        }
                    },
                    |_, _| {},
                );
                Some(server)
            }
            Err(err) => {
                log::error!("Failed to start OrchestrateService IPC server: {err:#?}");
                None
            }
        };

        Self { _server: server }
    }
}

impl Entity for OrchestrateBridge {
    type Event = ();
}

impl SingletonEntity for OrchestrateBridge {}

/// Per-section prompt seeded into each spawned sub-agent pane. Kept
/// dumb-on-purpose: the orchestration intelligence lives in the SKILL.md
/// system prompt (Phase 6), this just hands the child a pointer to the
/// section it owns.
fn prompt_for_section(plan_file: &std::path::Path, section_index: usize) -> String {
    format!(
        "Read {} and execute Section {}. Present the plan for my approval before doing any work.",
        plan_file.display(),
        section_index + 1
    )
}

/// UI-thread side of the orchestrate request. Reads the plan-mode
/// setting, resolves the active window's workspace, splits the focused
/// tab's pane group N times (alternating Right/Down), and seeds each new
/// pane with a `claude` invocation via `set_pending_command_queue`.
fn handle_orchestrate_request(
    request: OrchestrateRequest,
    ctx: &mut ModelContext<OrchestrateBridge>,
) -> OrchestrateResponse {
    if request.panes == 0 {
        return OrchestrateResponse {
            pane_ids: Vec::new(),
            error: Some("Orchestrate request asked for 0 panes; nothing to do.".to_string()),
        };
    }

    let mode = if *CortexSettings::as_ref(ctx).orchestrated_subagents_start_in_plan_mode {
        ClaudePermissionMode::Plan
    } else {
        ClaudePermissionMode::DangerouslySkip
    };

    let Some(workspace) = active_workspace(ctx) else {
        return OrchestrateResponse {
            pane_ids: Vec::new(),
            error: Some(
                "No active Cortex window/workspace found; cannot split panes.".to_string(),
            ),
        };
    };

    let plan_file = request.plan_file.clone();
    let panes_requested = request.panes;

    let pane_ids: Vec<String> = workspace.update(ctx, |workspace, ctx| {
        let pane_group = workspace.active_tab_pane_group().clone();
        pane_group.update(ctx, |pane_group, ctx| {
            let mut ids = Vec::with_capacity(panes_requested);
            for i in 0..panes_requested {
                let direction = if i % 2 == 0 {
                    Direction::Right
                } else {
                    Direction::Down
                };
                let new_pane_id = pane_group.add_terminal_pane(direction, None, ctx);
                ids.push(format!("{new_pane_id:?}"));
                let Some(terminal_view) =
                    pane_group.terminal_view_from_pane_id(new_pane_id, ctx)
                else {
                    log::warn!(
                        "OrchestrateBridge: freshly-split pane {new_pane_id:?} had no terminal view"
                    );
                    continue;
                };
                let command = build_local_claude_child_command(
                    &prompt_for_section(&plan_file, i),
                    mode,
                );
                terminal_view.update(ctx, |terminal_view, ctx| {
                    terminal_view.set_pending_command_queue(vec![command], ctx);
                });
            }
            ids
        })
    });

    OrchestrateResponse {
        pane_ids,
        error: None,
    }
}
