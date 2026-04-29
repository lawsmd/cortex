# Cortex Terminal — agent guidance

> *Cortex was originally named Quest until 2026-04-29. The rename was mechanical — no scope/strategy change. If you find a stray "Quest" anywhere, treat it as a miss to fix.*

This file is auto-loaded by Claude Code when working in `~/cortex`. It travels with the repo via git, so it's available on every machine after `git pull`. It complements `WARP.md` (Warp's own engineering guide, kept intact for upstream-merge cleanliness) — defer to `WARP.md` for code style and Warp-internal architecture; use this file for *Cortex-specific* posture and procedures.

## What this is

**Cortex** is a personal fork of [Warp Terminal](https://github.com/warpdotdev/warp), forked 2026-04-29 the day after Warp open-sourced.

- Repo: `github.com/lawsmd/cortex` (private; eventual public release planned).
- Multi-OS: macOS, Windows 11, Arch Linux. Each machine clones to `~/cortex` and builds locally.
- Upstream remote: `https://github.com/warpdotdev/warp.git`.
- License: AGPL-3.0-only (workspace) + MIT (`crates/warpui_core`, `crates/warpui`).

## The three customization phases

These run in **parallel, no fixed order**:

1. **Re-skin** — restyle Warp's `warpui` framework toward a TUI-aesthetic look (gradient borders, dim grey palettes, thick/thin box-drawing chars). Stay in Rust; Charm stack is Go and can't be linked in. Full plan: `docs/roadmap/reskin.md`.
2. **Feature expansion** — port favorites from `~/.config/terminal/` into Cortex. Headlining: a 1,079-theme picker. Full plan: `docs/roadmap/feature-expansion.md`.
3. **AI strip** — remove AI bloat (Oz Agent, voice transcription, code-review panel, images-as-context). Feature-flag first, delete second. Slowest phase, last. Full plan: `docs/roadmap/ai-strip.md`.

## The dev loop (macOS)

After code changes, rebuild and relaunch:

```bash
./scripts/install-cortex.sh   # builds + symlinks ~/Applications/Cortex.app
```

Then CMD+Q the running Cortex and relaunch via Raycast (or Spotlight). The script symlinks `~/Applications/Cortex.app` at the build output (`target/debug/bundle/osx/Cortex.app`), so once it's run the first time, subsequent rebuilds are picked up without any copy step.

Branding (display name "Cortex", bundle id `dev.lawsmd.Cortex`, icon) is baked into:

- `app/Cargo.toml`'s `[package.metadata.bundle.bin.warp-oss]` section — `name` and `identifier`
- `app/channels/oss/icon/no-padding/512x512.png` — the source PNG `cargo bundle` reads (regenerate from `scripts/cortex-icon.icns` via `sips` if the icon ever changes)

Edit those to change Cortex's identity. **Don't** post-process the built bundle — the build output is already correctly branded.

OSS channel === Cortex channel in this fork. If we ever ship multiple Cortex variants (preview, dev, etc.) we'll add a separate `cortex` channel; until then, "OSS" in the upstream code means "Cortex" here.

## Fork management — TL;DR

```
$ git remote -v
origin    https://github.com/lawsmd/cortex.git
upstream  https://github.com/warpdotdev/warp.git
```

- **Trunk:** `main` (Cortex fork only — upstream Warp's trunk is still `master`, so `upstream/master` is what we merge from).
- **Feature branches** for substantial work: `reskin/<thing>`, `feat/<thing>`, `ai-strip/<thing>`. Merge back to `main` with `git merge --no-ff`.
- **Commit messages:** conventional-commits-lite — `feat: ...`, `fix: ...`, `chore: ...`, `reskin: ...`, `ai-strip: ...`, `docs: ...`, `merge-upstream: ...`. Imperative subject, body explains *why*.
- **Push to `origin`** liberally — repo is private, off-machine backup is free.
- **Never push to `upstream`.** You can't anyway, but mind the muscle memory.

The fork is **`fork: false` per GitHub's API** — flipping visibility to private detached the GitHub fork relationship. Practical effect: no PRs from `lawsmd/cortex` upstream, but the `upstream` remote still works for fetches and merges.

Full guide: `docs/fork-management.md`.

## Upstream updates — TL;DR

```bash
cd ~/cortex
git fetch upstream
git checkout main
git merge upstream/master   # upstream's trunk is master; resolve conflicts; see below
git push origin main
```

- **Cadence:** monthly default, or per-meaningful-feature when Warp ships something we want. **Pause** during big phase 1 work.
- **Conflict philosophy:**
  - **Heavily customized files** (re-skin work, AI-strip files) → prefer Cortex's version, manually backport upstream improvements if any.
  - **Untouched files** (most of `warp_terminal/`, `lsp/`, etc.) → prefer upstream.
  - **Lightly customized** → read the conflict carefully, take upstream + re-apply our tweak.
  - **Hairy conflicts** → use a `cortex-merge-YYYY-MM-DD` branch, finalize there, then `git merge --no-ff` back to main.
- **The `repo-sync/watermark/private-to-public` tag** appears in upstream fetches. Internal Warp tooling. **Don't push it to origin** — avoid `git push origin --tags`.

Full guide: `docs/upstream-updates.md`.

## Where to look for what

| Topic | File |
|---|---|
| Setup per OS | `docs/setup/{macos,windows,arch}.md` |
| Architecture (crate layout, license split, default-members) | `docs/architecture.md` |
| The Rust UI stack (`warpui`/`warpui_core`) | `docs/ui-stack.md` |
| Phase plans | `docs/roadmap/{reskin,feature-expansion,ai-strip}.md` |
| Fork management (full) | `docs/fork-management.md` |
| Upstream updates (full) | `docs/upstream-updates.md` |
| Warp's coding conventions and architecture | `WARP.md` (root, committed by Warp; **do not duplicate its content here — defer to it**) |

## Public-release posture

The fork is private now and will stay private until the user decides to flip it. Bake these in from day one so the eventual flip is trivial:

- **No secrets in commits.** `~/.config/terminal/.env` (PAT) stays out of `~/cortex/`. Verify with `git -C ~/cortex log --all -p | grep -iE 'ghp_|gho_'` periodically.
- **Preserve Warp's `LICENSE`, `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`** — AGPL hygiene + courtesy. Don't accidentally delete them in restyle/cleanup commits.
- **Clean commit history.** Meaningful messages, no "WIP" cruft. Future-public means future-readable.
- **Cortex README in the repo root** should clearly note ancestry: "Cortex is a personal fork of [Warp](https://github.com/warpdotdev/warp)." (Add this just before going public; `WARP.md` covers some of this for now.)

Full pre-flight checklist for going public: `docs/fork-management.md § Public-release path`.

## Note on the `docs/` directory

`docs/` is **gitignored** (see `.gitignore` line 61). It contains personal reference material — roadmaps, architecture notes, restyle plans, the full operational tutorials. The user Syncthing's `~/cortex/docs/` directly across machines, so the docs sync without going through git.

What this means in practice:

- **You (the agent) can read `docs/*.md` from this checkout** — they exist on disk on the user's machines.
- **A fresh public clone won't have `docs/`** — references in this CLAUDE.md to `docs/...` paths will be dead links for anyone who clones `lawsmd/cortex` from GitHub. That's intentional; this CLAUDE.md is the single committed entry point.
- **Don't move docs back into git.** Resist the temptation. If something genuinely should be public-facing, write it as a separate file (e.g., `README.md` for the repo root, or `WARP.md`-style docs).

## Doing work in this repo

A few Cortex-specific operating principles on top of `WARP.md`'s coding conventions:

- **When restyling (`reskin/*`):** keep changes additive in `crates/warpui_extras` where possible. Adding a new element type is preferable to modifying an existing one — fewer upstream-merge conflicts.
- **When stripping AI (`ai-strip/*`):** feature-flag first via Cargo features, delete second. Verify the flagged-off build works for ~a week before deleting.
- **When merging upstream:** never resolve conflicts under time pressure. If a merge is hairy, branch off (`cortex-merge-YYYY-MM-DD`) and take it slow.
- **Before declaring a UI change "done":** actually run `./script/run` and look at it. Type checking proves correctness, not look-and-feel.

## Things to never do

- **Force-push to `main` once shared.** Force-pushes rewrite history; if you've shared the branch, anyone who pulled the old history breaks.
- **Push to `upstream`** (you can't anyway, but the reflex is dangerous).
- **Commit `~/.config/terminal/.env`** or any other file containing the GITHUB_PAT, API keys, credentials. Even deleting it in a follow-up commit doesn't scrub history.
- **Skip pre-commit hooks (`--no-verify`)** without diagnosing why they're failing first.
- **Push the `repo-sync/watermark/private-to-public` tag** to origin. It's Warp's internal sync metadata; let it stay in local refs only.
