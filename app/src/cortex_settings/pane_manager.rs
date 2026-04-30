//! Tracks open Cortex Settings panes per window so we show at most one and
//! can focus the existing one when reopened.
//!
//! Mirrors `app/src/server/network_log_pane_manager.rs`.
use std::collections::HashMap;

use warpui::{Entity, SingletonEntity, WindowId};

use crate::workspace::PaneViewLocator;

#[derive(Default)]
pub struct CortexSettingsPaneManager {
    panes: HashMap<WindowId, PaneViewLocator>,
}

impl CortexSettingsPaneManager {
    pub fn find_pane(&self, window_id: WindowId) -> Option<PaneViewLocator> {
        self.panes.get(&window_id).copied()
    }

    pub fn register_pane(&mut self, window_id: WindowId, locator: PaneViewLocator) {
        self.panes.insert(window_id, locator);
    }

    pub fn deregister_pane(&mut self, window_id: &WindowId) {
        self.panes.remove(window_id);
    }
}

impl Entity for CortexSettingsPaneManager {
    type Event = ();
}

impl SingletonEntity for CortexSettingsPaneManager {}
