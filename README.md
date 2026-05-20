# Cortex

> A personal fork of [Warp](https://github.com/warpdotdev/warp) by [@lawsmd](https://github.com/lawsmd) — a customization and convenience layer on top of the upstream terminal. Use at your own risk; this is built for me, not as a product.

<!-- TODO: drop the tabs/animation GIF here once it's pulled off the home machine -->

## What Cortex is

Cortex is Warp with a different coat of paint and a handful of extra knobs. Almost all the work is still Warp's — Cortex is the layer on top, not a rewrite.

If you want the real, supported terminal, grab [Warp](https://www.warp.dev). If you want my personal flavor of it, you're in the right place.

## What's different

### A library of ~1,079 themes
Cortex bundles around 1,079 terminal color schemes drawn from open-source community projects. They live in the theme picker, are searchable, and you can star favorites. None of these themes are original work in this repository — full credit is in the Credits section below.

### Tabs that show what each session is doing
<!-- GIF goes here once it's available -->

Cortex tabs pulse, breathe, and animate based on the state of the CLI agent (Claude Code, Codex, etc.) running inside them. You can glance at the sidebar and see which sessions are busy and which are idle. Each project also gets a color ring on its tab.

### Saved Projects, with sub-projects
A persistent saved-projects list in the vertical tab sidebar. Add a project once, open a session into it from the "+" picker any time after. Projects can also declare sub-projects — handy for monorepos and config trees.

### A Cortex Settings panel
A dedicated settings pane (separate from Warp's own settings) for Cortex-only knobs: tab styling, working-pane separators, and an AI section with a toggle to let `/orchestrate` spawn local Claude Code or Codex agents as children. Open it from the command menu.

## Building it yourself

Cortex uses Warp's build scripts:

```bash
./script/bootstrap   # platform-specific setup
./script/run         # build and run
```

On macOS and Windows, Cortex adds a two-lane prod/dev workflow — a stable daily-driver build, plus a separate live-rebuild dev build. The launchers are in `scripts/launch-cortex-dev.{sh,bat}`. See `WARP.md` for Warp's engineering guide and the `CLAUDE.md` in the repo root for Cortex-specific dev notes.

## Credits

- **[Warp](https://github.com/warpdotdev/warp)** — the terminal Cortex sits on top of. None of this exists without their work, and especially their decision to open-source the client. Cortex inherits Warp's license split (MIT for `warpui_core` and `warpui`, AGPL-3.0 for the rest).
- **Color themes** — bundled from four open-source community libraries:
  - [Gogh](https://github.com/Gogh-Co/Gogh) — ~361 schemes (MIT)
  - [iTerm2-Color-Schemes](https://github.com/mbadolato/iTerm2-Color-Schemes) — ~381 schemes (MIT; individual schemes retain their original copyright)
  - [base16](https://github.com/chriskempson/base16) — ~179 schemes (MIT)
  - [terminal.sexy](https://github.com/stayradiated/terminal.sexy) — ~157 schemes (MIT)

  Each theme's display name preserves its source tag (e.g. *"3024 (base16)"*, *"Adventure Time (Gogh)"*). The hand-curated default themes shipped with Warp are separate from this bundled library.

## License

Cortex inherits Warp's licensing. The `warpui_core` and `warpui` crates are [MIT](LICENSE-MIT); everything else is [AGPL-3.0](LICENSE-AGPL).

## Support

There isn't any. Cortex is a personal fork — I don't accept bug reports or PRs against it. If you find something broken upstream in Warp, file it on [`warpdotdev/warp`](https://github.com/warpdotdev/warp/issues).
