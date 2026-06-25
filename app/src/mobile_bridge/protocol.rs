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
//!     of further [`MobileResponse::Snapshot`] frames, one per coalesced output
//!     burst (server-side reflow — see [`MobileResponse::Snapshot`]). Raw
//!     [`MobileResponse::Output`] frames are no longer used by the mirror.
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
    /// Start mirroring a pane: the bridge replies with an initial
    /// [`MobileResponse::Snapshot`] of the current screen, then streams further
    /// throttled [`MobileResponse::Snapshot`] frames (server-side reflow) until
    /// [`MobileRequest::Unsubscribe`] or the connection closes. `pane_id` is the
    /// opaque token the client received in a `PaneList` (the serialized [`PaneId`]).
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
    /// Cortex: open a fresh blank terminal tab (the new-tab picker's "New
    /// terminal" item). Lands in the first window; the effect shows up in the
    /// next `PaneList` (the bridge also pushes one immediately).
    NewTab,
    /// Cortex: open a saved project as a new tab in its directory, tinted with
    /// its project color. `name` matches a [`ProjectEntry::name`] the client
    /// received in a [`MobileResponse::PaneList`].
    OpenProject { name: String },
    /// Cortex: add a terminal pane (split right) to the tab identified by
    /// `tab_id` — the opaque [`TabEntry::tab_id`] the client received in a
    /// `PaneList`. Backs the per-tab "+" square in the mobile sidebar.
    NewPane { tab_id: String },
}

/// A response sent back to the mobile client.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MobileResponse {
    /// The current workspace tree, one entry per open window. Cortex also
    /// attaches the user's saved projects so the phone's new-tab picker can list
    /// them — re-sent every poll, but free: `list_panes` already loads them for
    /// the per-tab project-name lookup.
    PaneList {
        windows: Vec<WindowEntry>,
        projects: Vec<ProjectEntry>,
    },
    /// A screen state for a subscribed pane: a base64-encoded ANSI redraw string
    /// that, written to xterm.js, reproduces the screen (clear + paint + cursor).
    /// Sent once on subscribe and then repeatedly — once per coalesced output
    /// burst — as the live mirror (server-side reflow): each frame is a clean,
    /// authoritative repaint, so the phone, which reflows the redraw to its *own*
    /// width, never ghosts a desktop-width cursor move the way a raw-byte stream
    /// would. `cols`/`rows` are the desktop pane's grid size; the client ignores
    /// them for sizing (it owns its width) and lets xterm reflow the redraw.
    Snapshot {
        pane_id: serde_json::Value,
        ansi_b64: String,
        cols: usize,
        rows: usize,
        /// Cortex: how the client should land this frame.
        /// * `true` — a *live* frame from an active output burst. The client
        ///   paints it into xterm's **alternate** buffer for a flicker-free,
        ///   scroll-stable in-place repaint (the alt buffer has no scrollback, so
        ///   row positions don't churn frame-to-frame and xterm diffs cells
        ///   instead of re-rendering the whole grid).
        /// * `false` — the initial on-attach snapshot, or the idle **settle**
        ///   frame the streamer emits once a burst goes quiet. The client paints
        ///   it into the **primary** buffer, whose scrollback then holds
        ///   browsable history. Defaulted so older/synthetic frames read as
        ///   non-streaming (primary).
        #[serde(default)]
        streaming: bool,
    },
    /// A live chunk of raw PTY output for a subscribed pane, base64-encoded.
    /// Written verbatim to xterm.js, which interprets the ANSI exactly as the
    /// desktop terminal does. Retained for protocol completeness, but the mirror
    /// now streams reflowed [`MobileResponse::Snapshot`] frames instead (see
    /// `Subscribe`), so the bridge no longer emits these.
    #[allow(dead_code)] // wire-protocol variant kept for back-compat; mirror streams Snapshot now
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

/// Cortex: one of the user's saved projects, surfaced to the mobile new-tab
/// picker. Tapping a row sends [`MobileRequest::OpenProject`] with `name`.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectEntry {
    /// Project display name; also the key the client echoes back in
    /// [`MobileRequest::OpenProject`].
    pub name: String,
    /// `#rrggbb` accent for the picker row's color swatch.
    pub color: String,
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
