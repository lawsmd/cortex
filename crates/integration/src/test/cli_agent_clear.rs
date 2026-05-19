//! Cortex divergence — integration test for the CLI-agent `/clear` viewport
//! pin. Exercises the full chain from a synthesized OSC-777 `session_clear`
//! event arriving on the PTY through to `ScrollState::post_clear_scroll_pinned`
//! being armed on the active `TerminalView`. Catches regressions in event
//! routing (OSC parser → `CLIAgentSessionsModel` → view handler) and the
//! `ScrollActiveBlockBottomToTop` wiring.
//!
//! Lower-level assertions on the geometry-derived offset live in the unit
//! test at `app/src/terminal/view.rs::scan_tests`; this test stays
//! deliberately coarse so it doesn't fight with the chase loop's deferred
//! re-pin timings.

use crate::test::integration_testing::terminal::{
    clear_blocklist_to_remove_bootstrapped_blocks, execute_command_for_single_terminal_in_tab,
};
use crate::test::new_step_with_default_assertions;
use crate::Builder;
use warp::integration_testing::terminal::util::ExpectedExitStatus;
use warp::integration_testing::terminal::wait_until_bootstrapped_single_pane_for_tab;
use warp::integration_testing::view_getters::single_terminal_view_for_tab;
use warpui::async_assert;

use super::new_builder;

/// A single shell command that:
///   1. Emits OSC-777 `session_start` so Cortex registers a CLI-agent
///      session on the active terminal view.
///   2. Emits a banner with a `╭` top-left corner so the post-`/clear`
///      banner scan in `find_lines_below_banner_top_in_block_output_for_clear`
///      has something to find in the active block's Output grid.
///   3. Emits OSC-777 `session_clear` so the view's `Cleared` handler runs
///      and arms `post_clear_scroll_pinned`.
///
/// Wire format reference: `app/src/terminal/cli_agent_sessions/event/v1.rs`.
/// Using `printf` (rather than `echo -e`) keeps the escape sequences portable
/// across bash, zsh, and fish — `printf` interprets `\033`/`\007` the same on
/// every POSIX shell.
const SESSION_START_BANNER_AND_CLEAR: &str = concat!(
    "printf '",
    // SessionStart — registers the listener via `register_cli_agent_listener_from_event`.
    "\\033]777;notify;warp://cli-agent;",
    "{\"v\":1,\"agent\":\"claude\",\"event\":\"session_start\"}",
    "\\007",
    // Banner content. The `╭` is the critical glyph; the rest is visual padding.
    "\\n╭───────────╮\\n│  banner  │\\n╰───────────╯\\n",
    // SessionClear — fires `CLIAgentSessionsModelEvent::Cleared` on the view.
    "\\033]777;notify;warp://cli-agent;",
    "{\"v\":1,\"agent\":\"claude\",\"event\":\"session_clear\"}",
    "\\007",
    "'",
);

pub fn test_cli_agent_session_clear_arms_post_clear_pin() -> Builder {
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(clear_blocklist_to_remove_bootstrapped_blocks())
        .with_step(execute_command_for_single_terminal_in_tab(
            0,
            SESSION_START_BANNER_AND_CLEAR.to_string(),
            ExpectedExitStatus::Success,
            // Banner content is rendered; OSC sequences are consumed by the
            // ANSI parser. We don't pin on an exact output match.
            (),
        ))
        .with_step(
            new_step_with_default_assertions("Assert /clear pin armed on view")
                .add_named_assertion("post_clear_scroll_pinned == true", |app, window_id| {
                    let terminal_view = single_terminal_view_for_tab(app, window_id, 0);
                    terminal_view.read(app, |view, _ctx| {
                        async_assert!(
                            view.post_clear_scroll_pinned(),
                            "expected post_clear_scroll_pinned=true after OSC-777 \
                             session_clear, got false — the chain OSC parser → \
                             CLIAgentSessionsModel → handle_cli_agent_sessions_event \
                             → ScrollActiveBlockBottomToTop is broken"
                        )
                    })
                }),
        )
}
