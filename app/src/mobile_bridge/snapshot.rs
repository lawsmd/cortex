//! Screen-snapshot serializer for the mobile mirror.
//!
//! When a phone subscribes to an already-running pane, xterm.js starts blank —
//! a TUI in the alternate screen (vim, htop, a full-screen Claude prompt) only
//! repaints on change, so without seeding it the phone would show nothing until
//! the next keystroke. This module serializes the pane's *current* screen into
//! an ANSI redraw string that, fed to a fresh xterm.js, reproduces it. The
//! live PTY stream then takes over and keeps it faithful byte-for-byte.
//!
//! This is the "one genuinely new primitive" from the mobile-companion plan,
//! and it turns out to be small: Cortex's grid already knows how to emit ANSI.
//! [`GridHandler::bounds_to_string`] with `include_esc_sequences = true` walks
//! the requested cell range emitting full SGR (colors, truecolor, bold/italic/
//! underline/inverse/…) plus `\r\n` line breaks. We only have to pick the
//! bounds and wrap the result with a screen-clear and a cursor reposition.
//!
//! Built entirely on public grid/model APIs, so it lives here in the
//! Cortex-owned bridge rather than reaching into the terminal core.

use warp_terminal::model::Point;

use crate::terminal::model::grid::RespectDisplayedOutput;
use crate::terminal::model::RespectObfuscatedSecrets;
use crate::terminal::TerminalModel;

/// How many rows back to capture for a primary-screen (normal scrolling) pane.
/// Enough to give a shell or a Claude REPL useful context on attach without
/// replaying the whole history; the live stream fills in everything after.
const MAX_PRIMARY_SNAPSHOT_ROWS: usize = 200;

/// A column index past any real terminal width. `bounds_to_string` clamps the
/// final row to its actual content length, so passing this as the end column
/// captures full-width rows without needing the grid's (private) `columns()`.
const FAR_COLUMN: usize = 10_000;

/// A pane's current screen plus the grid size it was captured at.
pub(super) struct ScreenSnapshot {
    /// ANSI redraw string: clear + paint + cursor reposition.
    pub ansi: String,
    /// Desktop pane width/height in cells, so the client can render the same
    /// grid and scale it to the phone rather than reflowing to its own width.
    pub cols: usize,
    pub rows: usize,
}

/// Serialize a pane's current screen as an ANSI redraw string plus its grid size.
///
/// Caller must hold the model lock; this only reads. Pair it with
/// `event_proxy.cortex_new_pty_reads_receiver()` under the *same* lock so the
/// receiver observes exactly the bytes that follow this snapshot.
pub(super) fn screen_snapshot(model: &TerminalModel) -> ScreenSnapshot {
    // Terminal grid size (one PTY = one size, primary or alt), so the phone can
    // match the desktop's column count instead of reflowing the stream.
    let size = model.block_list().size();
    let (cols, rows) = (size.columns(), size.rows());

    let alt_screen = model.is_alt_screen_active();

    // Alt-screen (TUI) holds the whole picture in its grid; the primary screen
    // keeps history, so we serialize the active block's output grid instead.
    let grid = if alt_screen {
        model.alt_screen().grid_handler()
    } else {
        model.block_list().active_block().output_grid().grid_handler()
    };

    let cursor = grid.cursor_point();
    let last_content_row = grid.max_content_row();

    let (start_row, end_row) = if alt_screen {
        (0, last_content_row)
    } else {
        let end = last_content_row.max(cursor.row);
        (end.saturating_sub(MAX_PRIMARY_SNAPSHOT_ROWS), end)
    };

    let body = grid.bounds_to_string(
        Point::new(start_row, 0),
        Point::new(end_row, FAR_COLUMN),
        // include_esc_sequences — this is what makes it an ANSI redraw.
        true,
        // Keep obfuscated secrets masked on the wire, matching the terminal's
        // own redaction (e.g. typed passwords show as '*').
        RespectObfuscatedSecrets::Yes,
        true,
        RespectDisplayedOutput::No,
    );

    // Clear screen + scrollback, home the cursor, paint, then place the cursor.
    let mut out = String::with_capacity(body.len() + 32);
    out.push_str("\u{1b}[2J\u{1b}[3J\u{1b}[H");
    out.push_str(&body);

    // In the alt screen the cursor is screen-relative, so we can place it
    // exactly. On the primary screen `body` ends at the bottom of the captured
    // region, which is where the prompt/cursor naturally sits; the live stream
    // corrects it on the next output, so we don't emit an absolute move there.
    if alt_screen {
        let row = cursor.row.saturating_sub(start_row) + 1;
        let col = cursor.col + 1;
        out.push_str(&format!("\u{1b}[{row};{col}H"));
    }

    ScreenSnapshot {
        ansi: out,
        cols,
        rows,
    }
}
