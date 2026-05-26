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

4. Invoke the orchestrate CLI via the Bash tool:

   ```
   cortex orchestrate --plan-file <path-from-step-2> --panes <N>
   ```

   The CLI prints one pane id per line on success, or returns a non-zero exit with an error.

5. Tell the user: "Spawned N panes. Each sub-agent will present its plan — approve each one when it appears."

## Section design

- Be explicit about scope: which files, which behavior, which output.
- Be explicit about boundaries: tell each sub-agent what NOT to touch so they don't conflict.
- If sections must be ordered, say so in the goal — but prefer truly independent slices.
