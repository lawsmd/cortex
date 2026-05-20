//! Cortex-specific settings UI, parallel to (and intentionally independent of)
//! Warp's `settings_view` module.
//!
//! Why this is separate: keeping all Cortex-fork code outside of
//! `app/src/settings_view/` shrinks the surface that conflicts on every
//! upstream merge from `warpdotdev/warp`. Cortex options also live under
//! their own user-facing menu rather than mixing with Warp's.
//!
//! Architecture mirrors the *NetworkLog* pane pattern, not the Warp
//! `SettingsPane` pattern — Cortex Settings is a simple sidebar-and-content
//! pane with no search, no umbrellas, and no need to round-trip through the
//! app-state persistence layer (closing and reopening Cortex Settings between
//! launches is cheap and stateless). See
//! `app/src/server/network_log_pane_manager.rs` and
//! `app/src/pane_group/pane/network_log_pane.rs` for the reference pattern.
//!
//! Note: the [`PaneContent`](crate::pane_group::pane::PaneContent) adapter
//! itself (`CortexSettingsPane`) lives in
//! `app/src/pane_group/pane/cortex_settings_pane.rs` because it needs access
//! to `pub(super)` items in the pane-group module hierarchy.
pub mod action;
pub mod ai_page;
pub mod brand;
pub mod pane_manager;
pub mod tabs_page;
pub mod view;
pub mod working_panes_page;

pub use action::{CortexSettingsAction, CortexSettingsSection};
pub use pane_manager::CortexSettingsPaneManager;
pub use view::{CortexSettingsView, CortexSettingsViewEvent};
