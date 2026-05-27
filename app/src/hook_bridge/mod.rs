//! Cortex hook-bridge IPC server.
//!
//! Boots a per-process [`ipc::Server`] exposing [`service::HookEmitService`],
//! stashes the socket path in a process-global cell that the terminal
//! env-var injector reads as `CORTEX_HOOK_IPC_SOCKET`, and accepts
//! envelopes posted by [`cortex-hook-emit`](../bin/cortex_hook_emit.rs)
//! from inside Claude lifecycle hooks.
//!
//! **Shadow-mode MVP, 2026-05-27.** Today the on-receive handler only
//! logs the envelope and bumps an "events received" counter visible on
//! the Cortex Settings → Diagnostics page. The OSC 777 / pts-walk path
//! in `cortex-hook.{sh,ps1}` remains the authoritative transport that
//! feeds `CLIAgentSessionsModel`. The IPC path runs in parallel so
//! Phase 2 ships the full transport plumbing without changing any
//! byte-routing behavior — a future Phase 3 flips authority once a
//! routing strategy (`CORTEX_PANE_ID` threading vs peer-PID lookup)
//! has been settled. See `docs/ai/external-status-injection.md`
//! § Layer A2 for the full rationale.
//!
//! Lifted from the [`OrchestrateBridge`](crate::orchestrate::OrchestrateBridge)
//! pattern with the same main-thread dispatcher shape.

use std::sync::OnceLock;
use std::time::Instant;

use ipc::ServerBuilder;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

mod service;
mod service_impl;

pub use service::{HookAck, HookEnvelope, HOOK_ENVELOPE_VERSION};

use service_impl::HookEmitServiceImpl;

/// Environment variable that carries the hook-bridge IPC socket path
/// from the Cortex app to every spawned terminal. `cortex-hook-emit`
/// (and through it, `cortex-hook.{sh,ps1}`) reads this to locate the
/// running service.
pub const CORTEX_HOOK_IPC_SOCKET_ENV: &str = "CORTEX_HOOK_IPC_SOCKET";

/// Process-global socket path of the hook-bridge IPC server. Populated
/// by [`HookBridgeServer::new`] at app startup. The terminal env-var
/// injector (`local_tty/unix.rs` and `local_tty/windows/environment.rs`)
/// reads it via [`hook_bridge_ipc_socket_path`] and threads the value
/// into every spawned local terminal.
static HOOK_BRIDGE_IPC_SOCKET_PATH: OnceLock<String> = OnceLock::new();

/// Returns the hook-bridge IPC server's socket path, if the bridge has
/// successfully booted. Returns `None` if `add_singleton_model` was
/// never called or if the server failed to start.
pub fn hook_bridge_ipc_socket_path() -> Option<&'static str> {
    HOOK_BRIDGE_IPC_SOCKET_PATH.get().map(String::as_str)
}

/// Singleton that owns the hook-bridge IPC server handle and the
/// main-thread stream task that processes incoming envelopes. Holds
/// the diagnostic counters surfaced on the Cortex Settings →
/// Diagnostics page.
pub struct HookBridgeServer {
    _server: Option<ipc::Server>,
    events_received: u64,
    last_envelope_at: Option<Instant>,
    last_error: Option<String>,
    decode_errors: u64,
}

impl HookBridgeServer {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let (tx, rx) = async_channel::unbounded();
        let builder = ServerBuilder::default()
            .with_service(HookEmitServiceImpl::new(tx));

        // Same MSYS2 workaround as the orchestrate bridge: on Windows,
        // the default `/tmp/...sock`-style connection address gets path-
        // mangled when it crosses the bash launcher into an env var, so
        // the client never finds the server. Force a bare pipe name.
        #[cfg(windows)]
        let builder = builder.with_fixed_address(format!(
            "cortex-hook-{}",
            rand::random::<i64>().unsigned_abs()
        ));

        let build_result = builder.build_and_run(ctx.background_executor());

        let server = match build_result {
            Ok((server, connection_address)) => {
                let socket_path = connection_address.to_string();
                if let Err(existing) = HOOK_BRIDGE_IPC_SOCKET_PATH.set(socket_path.clone()) {
                    log::warn!(
                        target: "hook_bridge",
                        "HookBridgeServer initialized twice; keeping existing socket \
                         {existing} and dropping new one at {socket_path}"
                    );
                }
                log::info!(
                    target: "hook_bridge",
                    "HookBridgeServer listening on {socket_path}"
                );
                ctx.spawn_stream_local(
                    rx,
                    |bridge, (envelope, reply), ctx| {
                        let ack = bridge.handle_envelope(envelope, ctx);
                        if reply.send(ack).is_err() {
                            log::warn!(
                                target: "hook_bridge",
                                "Hook IPC caller hung up before receiving ack."
                            );
                        }
                    },
                    |_, _| {},
                );
                Some(server)
            }
            Err(err) => {
                log::error!(
                    target: "hook_bridge",
                    "Failed to start HookBridgeServer: {err:#?}"
                );
                None
            }
        };

        Self {
            _server: server,
            events_received: 0,
            last_envelope_at: None,
            last_error: None,
            decode_errors: 0,
        }
    }

    pub fn events_received(&self) -> u64 {
        self.events_received
    }

    pub fn last_envelope_at(&self) -> Option<Instant> {
        self.last_envelope_at
    }

    /// Reserved for future Diagnostics enhancements (e.g. surfacing the
    /// most recent decode error inline). Currently no caller; kept on
    /// the public API so a follow-up can wire it without re-plumbing.
    #[allow(dead_code)]
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn decode_errors(&self) -> u64 {
        self.decode_errors
    }

    fn handle_envelope(
        &mut self,
        envelope: HookEnvelope,
        ctx: &mut ModelContext<Self>,
    ) -> HookAck {
        // Shadow MVP: log + count, no dispatch. The OSC 777 path drives
        // CLIAgentSessionsModel. See the module-level doc for the
        // Phase 3 flip plan.
        self.events_received = self.events_received.saturating_add(1);
        self.last_envelope_at = Some(Instant::now());

        if envelope.v != HOOK_ENVELOPE_VERSION {
            self.decode_errors = self.decode_errors.saturating_add(1);
            let msg = format!(
                "hook envelope version {} unsupported (server speaks {})",
                envelope.v, HOOK_ENVELOPE_VERSION
            );
            log::warn!(target: "hook_bridge", "{msg}");
            self.last_error = Some(msg.clone());
            ctx.notify();
            return HookAck {
                accepted: false,
                error: Some(msg),
            };
        }

        log::debug!(
            target: "hook_bridge",
            "shadow envelope received: agent={} event={} session_id={:?} cwd={:?}",
            envelope.agent,
            envelope.event,
            envelope.session_id,
            envelope.cwd,
        );

        ctx.notify();
        HookAck {
            accepted: true,
            error: None,
        }
    }
}

impl Entity for HookBridgeServer {
    type Event = ();
}

impl SingletonEntity for HookBridgeServer {}

/// Plain-data snapshot of the bridge's counters for the Diagnostics page.
#[derive(Debug, Clone)]
pub struct HookBridgeSnapshot {
    pub events_received: u64,
    pub last_envelope_at: Option<Instant>,
    pub decode_errors: u64,
    pub socket_bound: bool,
}

pub fn snapshot(app: &AppContext) -> HookBridgeSnapshot {
    let bridge = HookBridgeServer::as_ref(app);
    HookBridgeSnapshot {
        events_received: bridge.events_received(),
        last_envelope_at: bridge.last_envelope_at(),
        decode_errors: bridge.decode_errors(),
        socket_bound: hook_bridge_ipc_socket_path().is_some(),
    }
}
