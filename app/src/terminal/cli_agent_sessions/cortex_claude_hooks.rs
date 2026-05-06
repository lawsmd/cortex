//! Cortex-only: install/uninstall the claude hook bridge script in the user's
//! `~/.claude/settings.json`. The bridge translates claude's first-class hook
//! lifecycle events into OSC 777 messages that Cortex's CLIAgentSessionsModel
//! already understands, giving vanilla `clauded` (no warp@claude-code-warp
//! plugin) full participation in the rich-status pipeline.
//!
//! Architecture and rationale: see `docs/ai/external-status-injection.md`.
//!
//! Windows-first by user direction. Other OSes get a no-op stub until the
//! macOS/Linux follow-up lands; then the script port + unix-flavored install
//! drop in here too.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

#[cfg(target_os = "windows")]
const HOOK_SCRIPT: &[u8] =
    include_bytes!("../../../assets/cli-agent-hooks/claude/cortex-hook.ps1");

/// Substring embedded in every Cortex-managed hook entry's command string.
/// Used to find/replace our entries on idempotent re-install and to find/remove
/// them on uninstall, without depending on a top-level `_cortex_managed`
/// JSON field that claude might warn about as an unknown setting.
const COMMAND_MARKER: &str = "cortex-hook.ps1";

/// The claude hook events we route through. Each gets one or more
/// Cortex-managed entries in `settings.json.hooks.<event>`. Order matches the
/// order claude fires them across a turn. Used by the uninstall path to
/// purge every slot we ever touch.
const HOOK_EVENTS: &[&str] = &["UserPromptSubmit", "Notification", "Stop", "SessionEnd"];

/// One hook entry to install. `Notification` is split by `matcher` into the
/// two subtypes claude actually exposes (`permission_prompt`, `idle_prompt`)
/// so the hook script doesn't have to substring-match the message body.
struct HookSpec {
    event: &'static str,
    /// The `matcher` value claude uses to discriminate Notification subtypes.
    /// `None` for events where matchers are irrelevant (UserPromptSubmit,
    /// Stop, SessionEnd).
    matcher: Option<&'static str>,
    /// First positional arg passed to `cortex-hook.ps1`. The script routes
    /// purely on this — no message-body parsing.
    arg: &'static str,
}

const HOOK_SPECS: &[HookSpec] = &[
    HookSpec {
        event: "UserPromptSubmit",
        matcher: None,
        arg: "user_prompt_submit",
    },
    HookSpec {
        event: "Notification",
        matcher: Some("permission_prompt"),
        arg: "permission_request",
    },
    HookSpec {
        event: "Notification",
        matcher: Some("idle_prompt"),
        arg: "idle_prompt",
    },
    HookSpec {
        event: "Stop",
        matcher: None,
        arg: "stop",
    },
    HookSpec {
        event: "SessionEnd",
        matcher: None,
        arg: "session_end",
    },
];

#[derive(Debug)]
pub enum HookInstallError {
    Io(io::Error),
    Json(serde_json::Error),
    NoHomeDir,
}

impl std::fmt::Display for HookInstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookInstallError::Io(e) => write!(f, "I/O error: {e}"),
            HookInstallError::Json(e) => write!(f, "JSON error: {e}"),
            HookInstallError::NoHomeDir => write!(f, "could not determine home directory"),
        }
    }
}

impl std::error::Error for HookInstallError {}

impl From<io::Error> for HookInstallError {
    fn from(e: io::Error) -> Self {
        HookInstallError::Io(e)
    }
}

impl From<serde_json::Error> for HookInstallError {
    fn from(e: serde_json::Error) -> Self {
        HookInstallError::Json(e)
    }
}

/// Idempotent: extracts the hook script to a stable path, then merges our
/// hook entries into the user's claude `settings.json`. Safe to call on
/// every claude detection — the file mutations are no-ops when the on-disk
/// state already matches.
///
/// On non-Windows OSes this is a no-op stub until the macOS/Linux follow-up
/// from the plan ships its own script ports.
///
/// A future maintenance UI will likely want a `CortexSettings` flag like
/// `cli_agent_hooks.claude_installed` so it can show "Installed ✓" without
/// touching the filesystem. Defer until that UI lands — the install path is
/// idempotent on its own, so the flag would be a UX optimization, not a
/// correctness requirement.
pub fn ensure_claude_hooks_installed() -> Result<(), HookInstallError> {
    #[cfg(target_os = "windows")]
    {
        let script_path = ensure_hook_script_extracted()?;
        merge_hook_entries(&script_path)?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Cortex hook bridge is Windows-first per
        // docs/ai/external-status-injection.md § Phase 5.
        Ok(())
    }
}

/// Removes any Cortex-managed entries from the user's `settings.json`.
/// Identified by `COMMAND_MARKER` substring in the command string. Other
/// hooks (e.g. SideQuest's `claude-status.sh`) are left untouched.
#[allow(dead_code)] // Reserved for the future maintenance UI.
pub fn uninstall_claude_hooks() -> Result<(), HookInstallError> {
    let settings_path = claude_settings_path()?;
    if !settings_path.exists() {
        return Ok(());
    }
    let raw = fs::read_to_string(&settings_path)?;
    let mut json: Value = serde_json::from_str(&raw)?;
    let mut changed = false;
    if let Some(hooks) = json.get_mut("hooks").and_then(Value::as_object_mut) {
        for event in HOOK_EVENTS {
            if let Some(arr) = hooks.get_mut(*event).and_then(Value::as_array_mut) {
                let before = arr.len();
                arr.retain(|entry| !is_cortex_managed(entry));
                if arr.len() != before {
                    changed = true;
                }
            }
        }
    }
    if changed {
        atomic_write_json(&settings_path, &json)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn ensure_hook_script_extracted() -> Result<PathBuf, HookInstallError> {
    let dst = hook_script_path()?;
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    // Skip rewriting if the on-disk bytes already match. Avoids touching mtimes
    // on every claude detection, and avoids racing with claude reading the
    // script if it happens to be loading right now.
    let needs_write = match fs::read(&dst) {
        Ok(existing) => existing.as_slice() != HOOK_SCRIPT,
        Err(_) => true,
    };
    if needs_write {
        fs::write(&dst, HOOK_SCRIPT)?;
    }
    Ok(dst)
}

/// Stable, lane-independent location: prod and dev both extract here, so a
/// single absolute path embedded in `settings.json` keeps working when the
/// user switches between launching prod vs dev Cortex.
#[cfg(target_os = "windows")]
fn hook_script_path() -> Result<PathBuf, HookInstallError> {
    let local = dirs::data_local_dir().ok_or(HookInstallError::NoHomeDir)?;
    Ok(local.join("Cortex").join("hooks").join("claude").join("cortex-hook.ps1"))
}

fn claude_settings_path() -> Result<PathBuf, HookInstallError> {
    if let Ok(claude_home) = std::env::var("CLAUDE_HOME") {
        return Ok(PathBuf::from(claude_home).join("settings.json"));
    }
    let home = dirs::home_dir().ok_or(HookInstallError::NoHomeDir)?;
    Ok(home.join(".claude").join("settings.json"))
}

#[cfg(target_os = "windows")]
fn merge_hook_entries(script_path: &Path) -> Result<(), HookInstallError> {
    let settings_path = claude_settings_path()?;
    let mut json: Value = if settings_path.exists() {
        let raw = fs::read_to_string(&settings_path)?;
        if raw.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&raw)?
        }
    } else {
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)?;
        }
        json!({})
    };

    if !json.is_object() {
        // settings.json exists but isn't an object — bail rather than clobber.
        return Err(HookInstallError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "claude settings.json is not a JSON object",
        )));
    }

    // Snapshot pre-state so we only touch the file when something actually
    // changes. Cheaper than tracking per-entry deltas now that the install
    // path is purge-then-rebuild instead of find-or-append.
    let before = serde_json::to_string(&json)?;

    {
        let root = json.as_object_mut().expect("checked is_object above");
        let hooks = root
            .entry("hooks".to_string())
            .or_insert_with(|| json!({}));
        if !hooks.is_object() {
            return Err(HookInstallError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "claude settings.json `hooks` is not a JSON object",
            )));
        }
        let hooks_obj = hooks.as_object_mut().expect("checked");

        // Step 1: Purge every Cortex-managed entry from every slot we touch.
        // Cleans up stale entries from prior install shapes (e.g. the older
        // matcher-less Notification entry that Phase C replaces with two
        // matcher-specific ones) without leaving orphans behind.
        for event in HOOK_EVENTS {
            if let Some(arr) = hooks_obj.get_mut(*event).and_then(Value::as_array_mut) {
                arr.retain(|entry| !is_cortex_managed(entry));
            }
        }

        // Step 2: Append the desired entries fresh.
        for spec in HOOK_SPECS {
            let arr = hooks_obj
                .entry(spec.event.to_string())
                .or_insert_with(|| json!([]));
            if !arr.is_array() {
                return Err(HookInstallError::Io(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("claude settings.json `hooks.{}` is not an array", spec.event),
                )));
            }
            arr.as_array_mut()
                .expect("checked")
                .push(build_managed_entry(script_path, spec));
        }
    }

    let after = serde_json::to_string(&json)?;
    if before != after {
        atomic_write_json(&settings_path, &json)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn build_managed_entry(script_path: &Path, spec: &HookSpec) -> Value {
    let path_str = script_path.to_string_lossy();
    let command = format!(
        "powershell -NoProfile -ExecutionPolicy Bypass -File \"{path_str}\" {arg}",
        arg = spec.arg,
    );
    let mut entry = json!({
        "hooks": [
            {
                "type": "command",
                "command": command,
            }
        ]
    });
    if let Some(matcher) = spec.matcher {
        entry
            .as_object_mut()
            .expect("entry constructed as object literal")
            .insert("matcher".to_string(), Value::String(matcher.to_string()));
    }
    entry
}

fn is_cortex_managed(entry: &Value) -> bool {
    let Some(hooks) = entry.get("hooks").and_then(Value::as_array) else {
        return false;
    };
    hooks.iter().any(|sub| {
        sub.get("command")
            .and_then(Value::as_str)
            .is_some_and(|s| s.contains(COMMAND_MARKER))
    })
}

/// Pretty-printed atomic write: write to a sibling `.tmp` file, then rename
/// over the destination. Pretty (2-space indent) matches the formatting style
/// claude itself emits when it rewrites its own settings.json.
fn atomic_write_json(path: &Path, value: &Value) -> Result<(), HookInstallError> {
    let serialized = serde_json::to_string_pretty(value)?;
    let tmp = path.with_extension("json.cortex-tmp");
    fs::write(&tmp, serialized.as_bytes())?;
    // std::fs::rename replaces an existing destination atomically on Windows
    // (uses MoveFileEx with MOVEFILE_REPLACE_EXISTING semantics).
    fs::rename(&tmp, path)?;
    Ok(())
}
