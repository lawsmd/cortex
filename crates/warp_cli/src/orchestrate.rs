//! `cortex orchestrate` subcommand — argument parsing only.
//!
//! This is a Cortex-internal IPC bridge, not a public CLI surface. The
//! parent Claude Code session (running inside a Cortex pane via the
//! `cortex-orchestrate` skill) writes a plan file, then invokes
//! `cortex orchestrate --plan-file <path> --panes <N>` from its Bash
//! tool. The CLI handler in `app/src/orchestrate/cli.rs` reads
//! `CORTEX_IPC_SOCKET`, connects to the in-process `OrchestrateService`,
//! and sends an `OrchestrateRequest`.
//!
//! The subcommand is hidden from `--help` because end users have no
//! reason to invoke it directly.

use std::path::PathBuf;

use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct OrchestrateArgs {
    /// Absolute path to the plan file containing per-section prompts. If
    /// relative, the CLI resolves it against the current working
    /// directory before sending the IPC request.
    #[arg(long = "plan-file")]
    pub plan_file: PathBuf,

    /// Number of sub-agent panes to spawn. Must be >= 1.
    #[arg(long = "panes")]
    pub panes: usize,

    /// Working directory for the spawned panes. Reserved for future
    /// use; v1 inherits the focused pane's cwd via `add_terminal_pane`.
    #[arg(long = "working-dir")]
    pub working_dir: Option<PathBuf>,
}
