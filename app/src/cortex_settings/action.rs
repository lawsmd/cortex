//! Actions and section identifiers for the Cortex Settings UI.

/// Top-level categories shown down the left side of the Cortex Settings pane.
/// Add new sections here as the toggle set grows.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CortexSettingsSection {
    Appearance,
}

impl CortexSettingsSection {
    pub fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Appearance]
    }
}

impl Default for CortexSettingsSection {
    fn default() -> Self {
        Self::Appearance
    }
}

/// Actions emitted by the Cortex Settings view in response to user input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CortexSettingsAction {
    /// Switch the active sidebar section.
    SelectSection(CortexSettingsSection),
    /// Flip the `hide_pane_separators` toggle on the Appearance page.
    ToggleHidePaneSeparators,
}
