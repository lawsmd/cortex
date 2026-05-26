//! Wire types for the Cortex `OrchestrateService` IPC.
//!
//! The service is consumed by the `cortex orchestrate` CLI subcommand
//! (added in Phase 5), which is in turn spawned by the in-skill Bash step
//! of `cortex-skills/orchestrate/SKILL.md`. The parent orchestrator (a
//! Claude Code session running inside a Cortex pane) writes a plan file,
//! then invokes the CLI which connects to `CORTEX_IPC_SOCKET` and sends
//! an `OrchestrateRequest`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrateRequest {
    /// Absolute path to the plan file written by the parent agent. The
    /// UI-thread handler reads it to derive per-pane prompts.
    pub plan_file: PathBuf,
    /// How many sub-agent panes to spawn. Must be >= 1.
    pub panes: usize,
    /// Optional working directory; reserved for future use (v1 inherits
    /// the focused pane's cwd via `add_terminal_pane`).
    pub working_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrateResponse {
    /// String-rendered terminal pane IDs (one per spawned pane), in spawn
    /// order. Empty when `error` is `Some`.
    pub pane_ids: Vec<String>,
    /// Human-readable error string when the request could not be honored.
    pub error: Option<String>,
}

pub struct OrchestrateService;

impl ipc::Service for OrchestrateService {
    type Request = OrchestrateRequest;
    type Response = OrchestrateResponse;
}
