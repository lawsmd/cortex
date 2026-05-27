//! Cortex hook-bridge IPC thin client.
//!
//! Invoked from inside `cortex-hook.{sh,ps1}` (Claude lifecycle hook
//! wrapper) as a parallel transport alongside the existing OSC 777 /
//! `/dev/tty` emit. Reads a JSON envelope from stdin, dials the
//! Cortex-side `HookBridgeServer` via the IPC socket whose path is in
//! `CORTEX_HOOK_IPC_SOCKET`, and posts the bincode-serialized envelope.
//!
//! Always exits 0 — failures are logged to `~/.claude/cortex-hook.log`
//! but never propagated, so a broken IPC path cannot break a claude
//! turn. The shell-script callsite wraps this with `timeout 2 ...`, so
//! we don't need an in-process timeout here.
//!
//! The shadow-MVP design lives in
//! `docs/ai/external-status-injection.md` § Layer A2.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use ipc::{Client, ConnectionAddress};
use warpui::r#async::executor::Background;

const CORTEX_HOOK_IPC_SOCKET_ENV: &str = "CORTEX_HOOK_IPC_SOCKET";

fn main() -> ExitCode {
    if let Err(err) = run() {
        append_log(format!("ipc=fail err={err:#}"));
    }
    // Never propagate failure — hook scripts must exit 0 so claude's
    // own state machine isn't disrupted by a broken side channel.
    ExitCode::SUCCESS
}

fn run() -> Result<()> {
    let socket_path = std::env::var(CORTEX_HOOK_IPC_SOCKET_ENV).with_context(|| {
        format!("{CORTEX_HOOK_IPC_SOCKET_ENV} not set in environment")
    })?;

    let mut stdin_buf = String::new();
    std::io::stdin()
        .read_to_string(&mut stdin_buf)
        .context("failed to read envelope JSON from stdin")?;
    let stdin_trimmed = stdin_buf.trim();
    if stdin_trimmed.is_empty() {
        return Err(anyhow!("stdin envelope was empty"));
    }

    let envelope: cortex_hook_proto::HookEnvelope = serde_json::from_str(stdin_trimmed)
        .with_context(|| format!("envelope JSON did not parse: {stdin_trimmed}"))?;

    let executor = Arc::new(Background::new(1, |_| "cortex-hook-emit".to_owned()));
    let ack = warpui::r#async::block_on(async {
        let client = Client::connect(
            ConnectionAddress::from(socket_path.clone()),
            executor.clone(),
        )
        .await
        .with_context(|| {
            format!("failed to connect to hook bridge socket {socket_path}")
        })?;
        let caller = ipc::service_caller::<cortex_hook_proto::HookEmitService>(Arc::new(client));
        caller
            .call(envelope)
            .await
            .context("hook bridge call failed")
    })?;

    if ack.accepted {
        append_log("ipc=ok".to_string());
        Ok(())
    } else {
        Err(anyhow!(
            "server rejected envelope: {}",
            ack.error.as_deref().unwrap_or("<no error string>")
        ))
    }
}

fn append_log(line: String) {
    let Some(log_path) = log_file_path() else {
        return;
    };
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(f, "[{ts}] {line}");
    }
}

fn log_file_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))?;
    Some(home.join(".claude").join("cortex-hook.log"))
}

/// Vendored copy of the wire types from `app/src/hook_bridge/service.rs`.
/// Pulling the real module would require linking the entire `warp-oss`
/// app crate into this thin binary, which would balloon cold-start.
/// The two definitions must stay in lockstep — change `HOOK_ENVELOPE_VERSION`
/// in both places when the schema changes.
mod cortex_hook_proto {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HookEnvelope {
        pub v: u32,
        pub agent: String,
        pub event: String,
        #[serde(default)]
        pub query: Option<String>,
        #[serde(default)]
        pub response: Option<String>,
        #[serde(default)]
        pub summary: Option<String>,
        #[serde(default)]
        pub tool_name: Option<String>,
        #[serde(default)]
        pub session_id: Option<String>,
        #[serde(default)]
        pub cwd: Option<String>,
        #[serde(default)]
        pub transcript_path: Option<String>,
    }

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
}
