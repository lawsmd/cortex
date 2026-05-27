//! Wire types for the Cortex hook-bridge IPC.
//!
//! The bridge is the IPC transport for vanilla `clauded` hook
//! envelopes. In the **shadow-mode MVP** (2026-05-27, see
//! `docs/ai/external-status-injection.md` § Layer A2) the server-side
//! handler only logs + counts incoming envelopes; the OSC 777 path
//! remains authoritative for routing into `CLIAgentSessionsModel`.
//! The wire format mirrors the OSC 777 JSON envelope one-for-one so
//! the Phase 3 flip (when IPC becomes the sole transport) is a
//! mechanical change.

use serde::{Deserialize, Serialize};

/// Current wire protocol version. Bump on a breaking schema change.
pub const HOOK_ENVELOPE_VERSION: u32 = 1;

/// Same shape as the JSON body emitted to OSC 777 (`{"v":1,"agent":"claude",...}`),
/// re-serialized as bincode over the wire. Keeping field-for-field
/// parity means a Phase 3 transition that flips authority can reuse
/// the OSC parser's mapping logic without translation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEnvelope {
    pub v: u32,
    /// `"claude"` today; future agents will reuse this transport.
    pub agent: String,
    /// `"prompt_submit" | "stop" | "permission_request" | "idle_prompt"
    /// | "session_clear"`. Server logs unknown variants and drops them.
    pub event: String,
    pub query: Option<String>,
    pub response: Option<String>,
    pub summary: Option<String>,
    pub tool_name: Option<String>,
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub transcript_path: Option<String>,
}

/// Server reply. In the shadow MVP `accepted` is always `true` unless
/// the server is shutting down or the envelope failed bincode decode.
/// Errors don't propagate to the hook script (which exits 0 either
/// way), but the field is here for future use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookAck {
    pub accepted: bool,
    pub error: Option<String>,
}

pub struct HookEmitService;

impl ipc::Service for HookEmitService {
    type Request = HookEnvelope;
    type Response = HookAck;
}
