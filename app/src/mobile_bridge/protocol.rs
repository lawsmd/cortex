//! Wire protocol for the Cortex mobile companion bridge.
//!
//! A JSON envelope rides the WebSocket. Both directions are internally tagged
//! by a `type` field, snake-cased — so a request is `{"type":"list_panes"}`
//! and a response is `{"type":"pane_list", ...}`. Byte payloads (screen
//! snapshots and live PTY output) are base64-encoded so they survive the text
//! channel.
//!
//! **Milestone status (M3 — input injection):** building on M1's
//! [`MobileRequest::ListPanes`] → [`MobileResponse::PaneList`] and M2's
//! read-only mirror, the bridge now also accepts keystrokes:
//!   * [`MobileRequest::Subscribe`] → an initial [`MobileResponse::Snapshot`]
//!     (an ANSI redraw of the pane's current screen) followed by a live stream
//!     of [`MobileResponse::Output`] frames carrying raw PTY bytes.
//!   * [`MobileRequest::Unsubscribe`] stops that stream.
//!   * [`MobileRequest::Input`] writes raw bytes into a pane's PTY, exactly as
//!     if the user typed them locally (control codes, arrow keys, `/clear`).
//!   * [`MobileRequest::Paste`] pastes a composed block of text (bracketed-paste
//!     aware, optionally submitting) — the "compose then send" path.
//!
//! Unlike M1, responses are no longer 1:1 with requests — a single `Subscribe`
//! yields a snapshot plus an open-ended sequence of output frames, and `Input`
//! produces no direct response at all (its effect comes back as `Output` once
//! the PTY echoes). The server pushes frames through a per-connection outbound
//! queue rather than replying inline. Later milestones extend these enums in
//! place:
//!   * M4 — `Authenticate { token }`, plus `PaneState` attention pushes.

use serde::{Deserialize, Serialize};

/// A request sent by the mobile client to the desktop bridge.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MobileRequest {
    /// Ask for the full live tree of windows → tabs → panes.
    ListPanes,
    /// Start mirroring a pane: the bridge replies with a one-shot
    /// [`MobileResponse::Snapshot`] of the current screen, then streams live
    /// [`MobileResponse::Output`] frames until [`MobileRequest::Unsubscribe`]
    /// or the connection closes. `pane_id` is the opaque token the client
    /// received in a `PaneList` (the serialized [`PaneId`]).
    ///
    /// [`PaneId`]: crate::pane_group::pane::PaneId
    Subscribe { pane_id: serde_json::Value },
    /// Stop mirroring a pane previously passed to [`MobileRequest::Subscribe`].
    Unsubscribe { pane_id: serde_json::Value },
    /// Write raw bytes into a pane's PTY, as if typed locally. `pane_id` is the
    /// same opaque token as `Subscribe`; `bytes_b64` is base64-encoded raw input
    /// (UTF-8 text, control codes like `\x03` for Ctrl-C, or escape sequences
    /// like `\x1b[A` for the up arrow). Subscribing first is not required, but
    /// in practice the client only types into the pane it's mirroring.
    Input {
        pane_id: serde_json::Value,
        bytes_b64: String,
    },
    /// Paste a block of text into a pane as if it came from the clipboard, then
    /// (optionally) submit it. Unlike [`MobileRequest::Input`] — which sends raw
    /// bytes verbatim — this mirrors the desktop's `Ctrl+V`: the bridge
    /// normalizes newlines to `\r` and, *only when the focused app has enabled
    /// bracketed-paste mode*, wraps the text in `ESC[200~`/`ESC[201~` so multi-
    /// line content lands in the input buffer as one paste instead of executing
    /// line-by-line. This is what makes the phone's "compose then send" box work
    /// against Claude's prompt and any readline shell. `text` rides as a plain
    /// JSON string (UTF-8 and embedded newlines survive the channel, so no
    /// base64). When `submit` is true a single `\r` is appended *after* the
    /// closing bracket — i.e. "paste finished, then Enter" — for one-tap send.
    Paste {
        pane_id: serde_json::Value,
        text: String,
        #[serde(default)]
        submit: bool,
    },
}

/// A response sent back to the mobile client.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MobileResponse {
    /// The current workspace tree, one entry per open window.
    PaneList { windows: Vec<WindowEntry> },
    /// The initial screen state for a freshly subscribed pane: a base64-encoded
    /// ANSI redraw string. Fed to a blank xterm.js, it reproduces the screen
    /// (clear + paint + cursor). Always precedes the pane's `Output` frames.
    /// `cols`/`rows` are the desktop pane's grid size at subscribe time; the
    /// client resizes its terminal to match so the desktop's absolute cursor
    /// moves and alt-screen TUIs (Claude's prompts) land in the right cells,
    /// then scales/zooms that grid to fit the phone (v1 doesn't resize the PTY).
    Snapshot {
        pane_id: serde_json::Value,
        ansi_b64: String,
        cols: usize,
        rows: usize,
    },
    /// A live chunk of raw PTY output for a subscribed pane, base64-encoded.
    /// Written verbatim to xterm.js, which interprets the ANSI exactly as the
    /// desktop terminal does.
    Output {
        pane_id: serde_json::Value,
        bytes_b64: String,
    },
    /// A request could not be served. Carries a human-readable reason.
    Error { message: String },
}

/// One open Cortex window.
#[derive(Debug, Clone, Serialize)]
pub struct WindowEntry {
    /// Opaque, client-stable window identifier (debug form of the warpui
    /// `WindowId`). The client groups tabs under it; it never parses it.
    pub window_id: String,
    pub tabs: Vec<TabEntry>,
}

/// One tab within a window.
#[derive(Debug, Clone, Serialize)]
pub struct TabEntry {
    /// Opaque tab identifier (debug form of the tab's pane-group `EntityId`).
    pub tab_id: String,
    /// Display title of the tab — its focused pane's title (custom title if set).
    pub title: String,
    /// Whether this is the active tab in its window.
    pub active: bool,
    /// Cortex: the tab's project accent as `#rrggbb` — the same color the
    /// desktop's rounded pane-border / header-tint features resolve from the
    /// tab's saved project (or its manual/directory color). `None` when the tab
    /// maps to no project color. The mobile client tints the sidebar's project
    /// rail + swatch with it so each project and its panes are colour-coded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Cortex: the tab's mapped saved-project NAME (e.g. "cortex"), resolved from
    /// the tab's primary terminal working directory against the saved projects
    /// list. `None` when the tab maps to no project. The mobile client shows it
    /// in the pane header as "ProjectName: N" (N = the pane's number in the tab).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    pub panes: Vec<PaneEntry>,
}

/// One pane within a tab.
#[derive(Debug, Clone, Serialize)]
pub struct PaneEntry {
    /// Opaque, round-trippable pane token: the serde-serialized [`PaneId`].
    /// The client treats it as a black box and echoes it back in
    /// `Subscribe` / `Unsubscribe` requests, where the bridge deserializes it
    /// to the live `PaneId`. Embedding the real id (rather than a debug string)
    /// is what makes that round-trip work without any server-side id table.
    ///
    /// [`PaneId`]: crate::pane_group::pane::PaneId
    pub pane_id: serde_json::Value,
    /// Human-readable pane kind ("Terminal", "Code", "Cortex Settings", …),
    /// from the pane's `IPaneType` display name.
    pub kind: String,
    /// The CLI agent running in this pane, if one is tracked
    /// (e.g. "Claude Code"). `None` for plain shells and non-terminal panes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// The CLI agent's status, present iff `agent` is:
    /// `"in_progress"` | `"success"` | `"blocked"`. `"blocked"` is the
    /// attention signal the phone surfaces as a badge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Cortex: the desktop pane's current terminal grid size (columns, rows) —
    /// "what the pty and grid model thinks the terminal is". The phone keeps its
    /// xterm grid resized to match so a TUI's wrap/cursor math (Claude/Ink) lines
    /// up; carried on every poll so a size change re-syncs the mirror. `None` for
    /// non-terminal panes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cols: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<usize>,
}
