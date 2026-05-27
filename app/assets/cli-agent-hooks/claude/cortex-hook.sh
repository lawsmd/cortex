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
# Phase A1 diagnostic. Captures errno / bash error / pid tree / tty status
# on every `/dev/tty` write failure so we can pinpoint why the OSC bridge
# stops reaching Cortex. Safe to leave on — only writes on failure.
# See docs/ai/external-status-injection.md (Layer A diagnostic, 2026-05-19).
diag_log="$HOME/.claude/cortex-hook.log.diagnostic"

# ---- Cortex detection ---------------------------------------------------
# Short-circuit cleanly when claude is running outside Cortex (e.g. macOS
# Terminal.app, vscode integrated terminal). The user's claude config is
# shared across terminals; we must no-op there to avoid spurious side
# effects. Don't even log — runs outside Cortex are common for users with
# multiple terminals and we don't want to grow a log file in the no-op path.
if [ -z "${WARP_CLI_AGENT_PROTOCOL_VERSION:-}" ]; then
    exit 0
fi

# ---- Lane guard ---------------------------------------------------------
# Cortex's prod and dev lanes each install their own copy of this script
# (cortex-hook.sh vs cortex-hook-dev.sh) and each registers a hook entry in
# ~/.claude/settings.json that points at their own copy. When claude fires
# a hook event from a shell hosted by either Cortex window, BOTH scripts
# get invoked. Only the one matching the active lane should emit; the
# other must exit silently. Lane is encoded in the filename and the active
# lane is signaled by $WARP_DATA_PROFILE (set to "dev" by
# launch-cortex-dev.sh, unset by prod).
case "$0" in
    *-dev.sh) script_lane=dev ;;
    *)        script_lane=prod ;;
esac
if [ "${WARP_DATA_PROFILE:-}" = "dev" ]; then
    env_lane=dev
else
    env_lane=prod
fi
if [ "$script_lane" != "$env_lane" ]; then
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
    # SessionEnd: routed below — `/clear` ends the session with
    # reason="clear" and is the ONLY hook signal that fires for `/clear`
    # (UserPromptSubmit doesn`t fire for slash commands; ESC[2J isn`t
    # emitted either). Discriminate so the Cortex view layer can react.
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

# `/clear` end-of-session discriminator. SessionEnd fires with reason="clear"
# only when the user runs `/clear` inside claude. Other reasons ("other",
# future values) fall through to the normal stop mapping above.
if cortex_event == "stop" and event_name in ("session_end", "sessionend") \
        and obj.get("reason") == "clear":
    cortex_event = "session_clear"

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
# Happy path: write to /dev/tty. On older claude builds the hook subprocess
# inherits claude's controlling terminal (the pane PTY slave) and the write
# goes straight through Cortex's ANSI parser as if the pane shell had emitted
# the bytes.
#
# claude 2.1.139+ regression (Layer A, docs/ai/external-status-injection.md):
# hook subprocesses are spawned detached from the controlling tty (likely
# via setsid / posix_spawn with POSIX_SPAWN_SETSID), so /dev/tty returns
# ENXIO ("Device not configured"). When that happens, walk up the process
# tree to find an ancestor with a real controlling tty — the pane shell
# (claude → -zsh → warp-oss terminal-server) is the first hit — and write
# to /dev/<that-pts> instead. ps -o tty= is the cheapest read; ~5ms per
# emit including the walk.
emit_status=fail
emit_target=
# Capture bash's stderr from the redirect (e.g. "Device not configured" when
# /dev/tty isn't available) so we can write it to the diagnostic log without
# leaking it to the user's terminal. Stdout of the brace group is consumed
# by the redirect, so only stderr reaches the command substitution after `2>&1`.
emit_err=$({ printf '\033]777;notify;warp://cli-agent;%s\a' "$envelope" > /dev/tty; } 2>&1) && {
    emit_status=ok
    emit_target=/dev/tty
}

# Fallback: walk the process tree for a usable pts. Bounded at 12 hops so
# worst-case cost stays predictable. The first ancestor with a non-"??" tty
# wins — that's the pane shell, whose controlling tty is the pane PTY slave.
# We never walk past it (cortex / warp-oss processes themselves have no
# controlling tty), so there's no risk of writing OSC bytes to a tty outside
# the pane PTY session.
discovered_pts=
if [ "$emit_status" = fail ]; then
    walk_pid=$PPID
    walk_depth=0
    while [ -n "$walk_pid" ] && [ "$walk_pid" != "1" ] && [ "$walk_depth" -lt 12 ]; do
        walk_tty=$(ps -o tty= -p "$walk_pid" 2>/dev/null | tr -d ' ')
        case "$walk_tty" in
            "" | "?" | "??")
                ;;
            /*)
                discovered_pts=$walk_tty
                break
                ;;
            *)
                discovered_pts=/dev/$walk_tty
                break
                ;;
        esac
        walk_pid=$(ps -o ppid= -p "$walk_pid" 2>/dev/null | tr -d ' ')
        walk_depth=$((walk_depth + 1))
    done
    if [ -n "$discovered_pts" ]; then
        retry_err=$({ printf '\033]777;notify;warp://cli-agent;%s\a' "$envelope" > "$discovered_pts"; } 2>&1) && {
            emit_status=ok
            emit_target=$discovered_pts
        }
        if [ "$emit_status" = fail ]; then
            emit_err="$emit_err | pts=$discovered_pts retry_err=$retry_err"
        fi
    fi
fi

# ---- Shadow IPC emit -----------------------------------------------------
# Cortex Phase 2 (Layer A2, docs/ai/external-status-injection.md): fire the
# envelope through the Cortex hook-bridge IPC server in addition to the OSC
# path above. OSC remains authoritative; IPC delivery is logged separately
# (`ipc=ok|fail|skip`) so the Diagnostics page can show transport health for
# both channels independently and a future Phase 3 can flip authority once
# pane-routing is wired up.
#
# All failures swallowed — IPC is the SHADOW transport; it must never
# affect the hook's exit code or what claude sees.
ipc_status=skip
if [ -n "${CORTEX_HOOK_IPC_SOCKET:-}" ] && command -v cortex-hook-emit >/dev/null 2>&1; then
    if printf '%s' "$envelope" | timeout 2 cortex-hook-emit >/dev/null 2>&1; then
        ipc_status=ok
    else
        ipc_status=fail
    fi
fi

if [ "$emit_status" = fail ]; then
    # Diagnostic block: errno-equivalent info to figure out WHY /dev/tty
    # isn't writable. Cheapest possible probe — `tty`, `ps`, `ls` are all
    # standard utilities present on every macOS/Linux box Cortex targets.
    {
        printf '[%s] DIAG event=%s envelope_len=%d\n' \
            "$(date +'%Y-%m-%d %H:%M:%S')" \
            "$cortex_event" \
            "${#envelope}"
        printf '  bash_err=%s\n' "$emit_err"
        printf '  tty=%s\n' "$(tty 2>&1)"
        printf '  dev_tty_ls=%s\n' "$(ls -la /dev/tty 2>&1)"
        printf '  ids: pid=%d ppid=%d uid=%s sid=%s pgid=%s\n' \
            "$$" "$PPID" \
            "$(id -u 2>/dev/null)" \
            "$(ps -o sid= -p $$ 2>/dev/null | tr -d ' ')" \
            "$(ps -o pgid= -p $$ 2>/dev/null | tr -d ' ')"
        printf '  proc_tree (root→leaf):\n'
        # Walk up from $$ collecting (pid ppid sid pgid command) until we
        # hit PID 1 or 12 hops, whichever first.
        cur=$$
        depth=0
        while [ -n "$cur" ] && [ "$cur" != "1" ] && [ "$depth" -lt 12 ]; do
            line=$(ps -o pid,ppid,sid,pgid,command= -p "$cur" 2>/dev/null | tail -n +2)
            [ -z "$line" ] && break
            printf '    %s\n' "$line"
            cur=$(ps -o ppid= -p "$cur" 2>/dev/null | tr -d ' ')
            depth=$((depth + 1))
        done
    } >> "$diag_log" 2>/dev/null
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
        if [ "$emit_target" = /dev/tty ]; then
            printf '[%s] emit ok event=%s bytes=%d ipc=%s\n' \
                "$(date +'%Y-%m-%d %H:%M:%S')" \
                "$cortex_event" \
                "$total_len" \
                "$ipc_status" \
                >> "$log_path" 2>/dev/null
        else
            printf '[%s] emit ok event=%s bytes=%d via=%s ipc=%s\n' \
                "$(date +'%Y-%m-%d %H:%M:%S')" \
                "$cortex_event" \
                "$total_len" \
                "$emit_target" \
                "$ipc_status" \
                >> "$log_path" 2>/dev/null
        fi
    else
        printf '[%s] emit FAIL: write to /dev/tty and pts walk failed (event=%s envelope_len=%d pts=%s ipc=%s)\n' \
            "$(date +'%Y-%m-%d %H:%M:%S')" \
            "$cortex_event" \
            "$envelope_len" \
            "${discovered_pts:-<none>}" \
            "$ipc_status" \
            >> "$log_path" 2>/dev/null
    fi
} 2>/dev/null

exit 0
