#!/usr/bin/env bash
# Cortex bridge hook for vanilla `clauded` (claude --dangerously-skip-permissions).
#
# Translates claude's first-class hook lifecycle events into the OSC 777 wire
# format that Cortex's CLIAgentSessionsModel already understands. macOS/Linux
# companion to cortex-hook.ps1; same behaviour, simpler emit (no ConPTY
# AttachConsole dance — Unix `/dev/tty` writes go straight through the
# pane PTY back to Cortex's ANSI parser).
#
# Schema reference: app/src/terminal/cli_agent_sessions/event/{mod.rs,v1.rs}
#
# The first positional arg is the cortex-side event name (NOT the claude
# event name) — Notification is split upstream by `matcher` into two distinct
# entries so this script doesn't have to discriminate subtypes:
#     /bin/bash cortex-hook.sh user_prompt_submit
#     /bin/bash cortex-hook.sh stop
#     /bin/bash cortex-hook.sh permission_request
#     /bin/bash cortex-hook.sh idle_prompt
#     /bin/bash cortex-hook.sh session_end
#     /bin/bash cortex-hook.sh pre_compact
#
# Claude pipes the rest of the event payload (session_id, transcript path,
# event-specific fields) as JSON over stdin.
#
# Diagnostics: ~/.claude/cortex-hook.log
# Discovery (every invocation, raw): ~/.claude/cortex-hook-discovery.log

# Never let an error in the hook break claude. Every external call is best-
# effort; we always exit 0.
set +e

hook_event="${1:-}"
log_path="$HOME/.claude/cortex-hook.log"
discovery_log="$HOME/.claude/cortex-hook-discovery.log"

# ---- Cortex detection ---------------------------------------------------
# Short-circuit cleanly when claude is running outside Cortex (e.g. macOS
# Terminal.app, vscode integrated terminal). The user's claude config is
# shared across terminals; we must no-op there to avoid spurious side
# effects. Don't even log — runs outside Cortex are common for users with
# multiple terminals and we don't want to grow a log file in the no-op path.
if [ -z "${WARP_CLI_AGENT_PROTOCOL_VERSION:-}" ]; then
    exit 0
fi

# ---- Locate python3 ------------------------------------------------------
# python3 handles JSON parse + envelope build because robust JSON in pure
# bash is a tarpit. Reliably present on macOS (Xcode CLT, required for
# Cortex builds) and on every Linux distro Cortex targets.
if command -v python3 >/dev/null 2>&1; then
    PYTHON=python3
elif [ -x /usr/bin/python3 ]; then
    PYTHON=/usr/bin/python3
else
    # No python3 — bail silently rather than mangle the envelope by hand.
    exit 0
fi

# ---- Read claude's stdin JSON -------------------------------------------
# Read the whole payload into a variable so we can use it for both the
# discovery log AND the python3 envelope build.
stdin_raw=$(cat)

# ---- Discovery probe -----------------------------------------------------
# Append every hook invocation's positional arg + raw stdin to a discovery
# log. Used to confirm whether claude ever fires Notification subtypes we
# don't yet handle (e.g. follow-up questions). Errors are swallowed; never
# blocks the real path.
{
    arg_display="${hook_event:-<none>}"
    stdin_flat=$(printf '%s' "$stdin_raw" | tr -d '\r' | tr '\n' ' ')
    printf '%s arg=%s stdin=%s\n' \
        "$(date +'%Y-%m-%dT%H:%M:%S%z')" \
        "$arg_display" \
        "$stdin_flat" \
        >> "$discovery_log" 2>/dev/null
} 2>/dev/null

# ---- Build the CLIAgentEvent envelope ------------------------------------
# IMPORTANT: the wire format is FLAT. Even though the in-memory Rust struct
# groups payload fields under a `payload` field for ergonomics
# (event/mod.rs CLIAgentEventPayload), the v1 wire-format parser
# (event/v1.rs RawEvent) expects every field at the top level of the JSON
# body. Nesting them under `payload` causes serde to silently parse them as
# None — closing Tier 1's `prompted` gate forever. See bug #2 in
# docs/ai/external-status-injection.md.
#
# python3 reads stdin (the original claude payload), prints two lines on
# success: the cortex event name on line 1, the compact envelope JSON on
# line 2. Empty output = unmapped event; bail.
result=$(printf '%s' "$stdin_raw" | "$PYTHON" -c '
import json, os, re, sys

arg = sys.argv[1] if len(sys.argv) > 1 else ""

try:
    raw = sys.stdin.read()
    obj = json.loads(raw) if raw.strip() else {}
except Exception:
    obj = {}

mapping = {
    "user_prompt_submit": "prompt_submit",
    "userpromptsubmit":   "prompt_submit",
    "stop":               "stop",
    "session_end":        "stop",
    "sessionend":         "stop",
    "permission_request": "permission_request",
    "idle_prompt":        "idle_prompt",
    # `/compact` (manual) and auto-compaction. Reuses prompt_submit so
    # apply_event flips status to InProgress for the duration of the
    # compaction call. There is no PostCompact hook in current claude;
    # the next Stop or UserPromptSubmit clears the running animation.
    # We do NOT block compaction — exit 0 with empty stdout (the OSC 777
    # goes to /dev/tty, never to claude'"'"'s stdin/stdout).
    "pre_compact":        "prompt_submit",
    "precompact":         "prompt_submit",
}

# Prefer the positional arg (set by ~/.claude/settings.json). Fall back to
# claude'"'"'s own hook_event_name for stale entries from before the matcher
# split.
event_name = arg.lower() if arg else ""
if not event_name:
    raw_name = obj.get("hook_event_name") or ""
    event_name = re.sub(r"([a-z])([A-Z])", r"\1_\2", raw_name).lower()

cortex_event = mapping.get(event_name)

# Stale invocation: an old matcher-less Notification entry left over from a
# pre-Phase-C settings.json. Fall back to the legacy substring discrimination
# so the bridge keeps working during the brief window between settings
# rewrite and old entries being purged on a fresh install.
if not cortex_event and event_name == "notification":
    msg = obj.get("message") or ""
    if "permission" in msg.lower():
        cortex_event = "permission_request"
    elif msg:
        cortex_event = "idle_prompt"

if not cortex_event:
    sys.exit(0)

try:
    version = int(os.environ.get("WARP_CLI_AGENT_PROTOCOL_VERSION", "1"))
except ValueError:
    version = 1

envelope = {
    "v": version,
    "agent": "claude",
    "event": cortex_event,
}

# Per-event payload — FLAT, not nested under a `payload` key.
if cortex_event == "prompt_submit" and obj.get("prompt"):
    envelope["query"] = str(obj["prompt"])
elif cortex_event == "prompt_submit" and event_name in ("pre_compact", "precompact"):
    # PreCompact has no `prompt` field but does carry `trigger` ("manual"
    # vs "auto"). Surface it as the query so the discovery log + any
    # downstream consumer can see what kicked off the compaction.
    trigger = obj.get("trigger")
    envelope["query"] = "compact ({})".format(trigger) if trigger else "compact"
elif cortex_event == "permission_request":
    if obj.get("message"):
        envelope["summary"] = str(obj["message"])
    if obj.get("tool_name"):
        envelope["tool_name"] = str(obj["tool_name"])
elif cortex_event == "idle_prompt":
    if obj.get("message"):
        envelope["summary"] = str(obj["message"])
# Stop / session_end carry no event-specific payload; apply_event maps
# Stop → Success regardless of payload content.

# Common fields shared across every event type.
for key in ("session_id", "cwd", "transcript_path"):
    if obj.get(key):
        envelope[key] = str(obj[key])

sys.stdout.write(cortex_event + "\n")
sys.stdout.write(json.dumps(envelope, separators=(",", ":")))
' "$hook_event" 2>/dev/null)

if [ -z "$result" ]; then
    exit 0
fi

cortex_event=$(printf '%s' "$result" | head -n 1)
envelope=$(printf '%s' "$result" | tail -n +2)

if [ -z "$envelope" ] || [ -z "$cortex_event" ]; then
    exit 0
fi

# ---- Emit OSC 777 to the controlling TTY --------------------------------
# Wire format: \x1b]777;notify;warp://cli-agent;<JSON>\x07
#
# Hook subprocesses on Unix inherit claude's controlling terminal, which is
# the pane PTY slave that Cortex reads from on the master side. Writing to
# /dev/tty bypasses any stdout/stderr capture claude does for non-forwarded
# events (Stop, Notification, SessionEnd) — the bytes flow straight through
# the PTY back to Cortex's ANSI parser as if the pane shell had emitted
# them. No process-tree walking or AttachConsole equivalent is needed
# because Unix has no ConPTY-style private-console boundary.
emit_status=fail
# Brace group + outer 2>/dev/null so bash-level redirect-open errors
# (e.g. "Device not configured" when /dev/tty isn't available) don't leak
# to the user's terminal. The inner printf 2>/dev/null is redundant for
# the redirection but harmless.
if { printf '\033]777;notify;warp://cli-agent;%s\a' "$envelope" > /dev/tty; } 2>/dev/null; then
    emit_status=ok
fi

# ---- Per-emit log line ---------------------------------------------------
{
    envelope_len=${#envelope}
    # Total OSC sequence length = `\e]777;notify;warp://cli-agent;` (29
    # bytes including the leading ESC) + envelope + `\a` (1 byte BEL) =
    # envelope_len + 30. Matches the byte-count semantics the PowerShell
    # script logs.
    total_len=$((envelope_len + 30))
    if [ "$emit_status" = ok ]; then
        printf '[%s] emit ok event=%s bytes=%d\n' \
            "$(date +'%Y-%m-%d %H:%M:%S')" \
            "$cortex_event" \
            "$total_len" \
            >> "$log_path" 2>/dev/null
    else
        printf '[%s] emit FAIL: write to /dev/tty failed (event=%s envelope_len=%d)\n' \
            "$(date +'%Y-%m-%d %H:%M:%S')" \
            "$cortex_event" \
            "$envelope_len" \
            >> "$log_path" 2>/dev/null
    fi
} 2>/dev/null

exit 0
