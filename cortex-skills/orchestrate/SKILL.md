---
name: orchestrate
description: Fan a task out into 2-5 parallel Cortex sub-agent panes. Trigger phrases include "orchestrate", "fan out", "parallel agents", and "split this into sub-agents". Each sub-agent runs claude in its own Cortex pane against one section of a shared plan file.
---

# Orchestrate

Split a task into N parallel sub-agents, each running in its own Cortex pane.

## When to use

Use when the user asks to "orchestrate", "fan out", "parallel agents", or "split this into sub-agents", or when a request is large and decomposes naturally into independent pieces.

Do NOT use for single-shot work. Splitting has setup cost; reserve it for genuinely parallelizable tasks.

## How to run it

1. Decide N — the number of panes — based on how the task naturally divides. Stay in the range [2, 5]. Prefer 2 or 3 unless the task genuinely demands more.
2. Create a temp plan file: `mktemp -t cortex-orchestrate.XXXXXX.md`. Capture the path it prints.
3. Write the plan file with this exact structure:

   ```
   # Goal
   <one-paragraph statement of what the user wants>

   ## Section 1
   <focused, standalone instructions for sub-agent 1>

   ## Section 2
   <focused, standalone instructions for sub-agent 2>
   ```

   Each `## Section N` heading must appear on its own line. Each section must be self-contained — the sub-agent reads only its own section.

4. Create a second temp file for the pane ids (`mktemp -t cortex-orchestrate-ids.XXXXXX`), then invoke the orchestrate CLI via the Bash tool:

   ```
   cortex orchestrate --plan-file <path-from-step-2> --panes <N> --out <ids-file>
   ```

   `cortex` is a console shim installed at `%LOCALAPPDATA%\Cortex\bin` (on the user PATH; installed by `scripts\install-cortex-prod.cmd`). If the shell can't resolve `cortex` — e.g. this Cortex instance launched before the shim existed, so its panes inherited a stale PATH — fall back to the absolute shim path `"$LOCALAPPDATA/Cortex/bin/cortex"` (Bash) / `"%LOCALAPPDATA%\Cortex\bin\cortex.cmd"` (cmd/PowerShell), and only as a last resort the raw EXE `"$LOCALAPPDATA/Cortex/Cortex.exe"`. Prefer the shim: the EXE is GUI-subsystem, so interactive shells won't wait for it or surface its exit code.

   Contract: on success the CLI prints one pane id per line on stdout *and* writes the same ids to the `--out` file; on failure (no `CORTEX_IPC_SOCKET` in the environment, dead socket, no workspace, bad arguments) it exits non-zero with an error on stderr. If stdout comes back empty despite exit 0, read the `--out` file — do not fall back to scanning the process table.

5. Tell the user: "Spawned N panes. Each sub-agent will present its plan — approve each one when it appears."

## Section design

- Be explicit about scope: which files, which behavior, which output.
- Be explicit about boundaries: tell each sub-agent what NOT to touch so they don't conflict.
- If sections must be ordered, say so in the goal — but prefer truly independent slices.
