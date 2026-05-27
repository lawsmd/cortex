use warpui::elements::{DraggableState, MouseStateHandle};
use warpui::fonts::FamilyId;

use super::FileTreeItem;
use crate::code::icon_from_file_path;
use crate::ui_components::item_highlight::ImageOrIcon;
use crate::{appearance::Appearance, ui_components::icons::Icon};

// CORTEX-BEGIN: yazi-file-explorer
/// Settings snapshot for a single render frame, populated in `render_item()`
/// where `&AppContext` is available, and consumed inside the `Hoverable` closure
/// where only owned/borrowed data is accessible.
#[derive(Clone)]
pub(super) struct TuiStyleSettings {
    pub tree_lines: bool,
    pub nerd_icons: bool,
    pub colored_icons: bool,
    pub explorer_font_size: f32,
    pub explorer_font_family: Option<FamilyId>,
    pub nerd_font_family: Option<FamilyId>,
}

impl Default for TuiStyleSettings {
    fn default() -> Self {
        Self {
            tree_lines: false,
            nerd_icons: false,
            colored_icons: false,
            explorer_font_size: 14.0,
            explorer_font_family: None,
            nerd_font_family: None,
        }
    }
}
// CORTEX-END: yazi-file-explorer

impl FileTreeItem {
    pub(super) fn to_render_state(
        &self,
        is_expanded: Option<bool>,
        appearance: &Appearance,
    ) -> RenderState {
        match self {
            FileTreeItem::File {
                metadata,
                mouse_state_handle,
                depth,
                draggable_state,
                // CORTEX-BEGIN: yazi-file-explorer
                is_last_at_depth,
                // CORTEX-END: yazi-file-explorer
            } => {
                let display_name = metadata
                    .path
                    .file_name()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| String::from("File"));

                let icon_from_file_path =
                    icon_from_file_path(metadata.path.as_str(), appearance).map(ImageOrIcon::Image);

                RenderState {
                    display_name,
                    icon: icon_from_file_path.unwrap_or(ImageOrIcon::Icon(Icon::File)),
                    is_expanded,
                    depth: *depth,
                    mouse_state: mouse_state_handle.clone(),
                    draggable_state: draggable_state.clone(),
                    is_ignored: metadata.ignored,
                    is_last_at_depth: is_last_at_depth.clone(),
                    tui: TuiStyleSettings::default(),
                }
            }
            FileTreeItem::DirectoryHeader {
                directory,
                mouse_state_handle,
                depth,
                draggable_state,
                // CORTEX-BEGIN: yazi-file-explorer
                is_last_at_depth,
                // CORTEX-END: yazi-file-explorer
            } => {
                let display_name = directory
                    .path
                    .file_name()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| String::from("Folder"));
                RenderState {
                    display_name,
                    icon: ImageOrIcon::Icon(Icon::Folder),
                    is_expanded,
                    depth: *depth,
                    mouse_state: mouse_state_handle.clone(),
                    draggable_state: draggable_state.clone(),
                    is_ignored: directory.ignored,
                    is_last_at_depth: is_last_at_depth.clone(),
                    tui: TuiStyleSettings::default(),
                }
            }
        }
    }
}

pub(super) struct RenderState {
    pub display_name: String,
    pub icon: ImageOrIcon,
    pub is_expanded: Option<bool>,
    pub depth: usize,
    pub mouse_state: MouseStateHandle,
    pub draggable_state: DraggableState,
    pub is_ignored: bool,
    // CORTEX-BEGIN: yazi-file-explorer
    pub is_last_at_depth: Vec<bool>,
    pub tui: TuiStyleSettings,
    // CORTEX-END: yazi-file-explorer
}
