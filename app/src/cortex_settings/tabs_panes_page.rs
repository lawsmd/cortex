//! Content rendered on the right side of the Cortex Settings pane when the
//! "Tabs/Panes" section is selected.
//!
//! Layout: three subsection headers (`Vertical Tab Bar/Panel`, `Selected Tabs`,
//! `Unselected Tabs`), each followed by its toggles/selectors. Subsection
//! headers are plain styled `Text` rows — there's no framework support for
//! sub-sections inside a page, just rendered children. Patterns for individual
//! rows mirror `appearance_page.rs::render_hide_pane_separators_row` (toggle)
//! and `app/src/settings_view/appearance_page.rs::InputTypeWidget::render`
//! (radio buttons).
use std::rc::Rc;

use settings::Setting;
use warpui::{
    elements::{
        Container, CrossAxisAlignment, Element, Flex, MainAxisAlignment, MouseStateHandle, Padding,
        ParentElement, Text,
    },
    ui_components::{
        components::UiComponent,
        radio_buttons::{RadioButtonItem, RadioButtonLayout, RadioButtonStateHandle},
        switch::SwitchStateHandle,
    },
    AppContext, SingletonEntity,
};

use crate::appearance::Appearance;
use crate::cortex_settings::action::CortexSettingsAction;
use crate::settings::{
    CortexSettings, TabsSelectedMetadataAlignment, TabsSelectedTitleAlignment,
    TabsUnselectedMetadataAlignment, TabsUnselectedTitleAlignment,
};

const ROW_VERTICAL_PADDING: f32 = 6.0;
const LABEL_RIGHT_MARGIN: f32 = 12.0;
const SUBSECTION_HEADER_TOP_MARGIN: f32 = 16.0;
const SUBSECTION_HEADER_BOTTOM_MARGIN: f32 = 6.0;
const SUBSECTION_HEADER_FIRST_TOP_MARGIN: f32 = 0.0;
const SUBSECTION_HEADER_FONT_SIZE_DELTA: f32 = 1.0;

/// Per-toggle/selector UI state that has to outlive a single render frame
/// (mouse-state handles for hover detection, switch animation, radio-button
/// selection state). Owned by `CortexSettingsView` and threaded into the render
/// fns via `&self`.
pub struct TabsPanesPageState {
    panel_bg_switch: SwitchStateHandle,
    inverse_fill_switch: SwitchStateHandle,
    selected_title_radio: RadioButtonStateHandle,
    selected_title_mouse_states: Vec<MouseStateHandle>,
    selected_metadata_radio: RadioButtonStateHandle,
    selected_metadata_mouse_states: Vec<MouseStateHandle>,
    unselected_title_radio: RadioButtonStateHandle,
    unselected_title_mouse_states: Vec<MouseStateHandle>,
    unselected_metadata_radio: RadioButtonStateHandle,
    unselected_metadata_mouse_states: Vec<MouseStateHandle>,
}

impl Default for TabsPanesPageState {
    fn default() -> Self {
        Self {
            panel_bg_switch: SwitchStateHandle::default(),
            inverse_fill_switch: SwitchStateHandle::default(),
            selected_title_radio: RadioButtonStateHandle::default(),
            selected_title_mouse_states: vec![
                MouseStateHandle::default(),
                MouseStateHandle::default(),
            ],
            selected_metadata_radio: RadioButtonStateHandle::default(),
            selected_metadata_mouse_states: vec![
                MouseStateHandle::default(),
                MouseStateHandle::default(),
            ],
            unselected_title_radio: RadioButtonStateHandle::default(),
            unselected_title_mouse_states: vec![
                MouseStateHandle::default(),
                MouseStateHandle::default(),
            ],
            unselected_metadata_radio: RadioButtonStateHandle::default(),
            unselected_metadata_mouse_states: vec![
                MouseStateHandle::default(),
                MouseStateHandle::default(),
            ],
        }
    }
}

pub fn tabs_panes_page_search_terms() -> &'static [&'static str] {
    &[
        "tabs",
        "panes",
        "tab",
        "pane",
        "vertical",
        "panel",
        "background",
        "selected",
        "unselected",
        "title",
        "metadata",
        "alignment",
        "centered",
        "inverse",
        "fill",
    ]
}

pub fn render_tabs_panes_page(
    state: &TabsPanesPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let settings = CortexSettings::as_ref(app);
    let panel_bg = *settings.tabs_panel_matches_terminal_bg.value();
    let inverse_fill = *settings.tabs_inverse_fill_on_selection.value();
    let selected_title_idx =
        alignment_idx_selected_title(*settings.tabs_selected_title_alignment.value());
    let selected_metadata_idx =
        alignment_idx_selected_metadata(*settings.tabs_selected_metadata_alignment.value());
    let unselected_title_idx =
        alignment_idx_unselected_title(*settings.tabs_unselected_title_alignment.value());
    let unselected_metadata_idx =
        alignment_idx_unselected_metadata(*settings.tabs_unselected_metadata_alignment.value());

    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        // Vertical Tab Bar/Panel
        .with_child(render_subsection_header(
            "Vertical Tab Bar/Panel",
            true,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Bar/Panel Background Matches Terminal Background",
            state.panel_bg_switch.clone(),
            panel_bg,
            CortexSettingsAction::ToggleTabsPanelMatchesTerminalBg,
            appearance,
        ))
        // Selected Tabs
        .with_child(render_subsection_header("Selected Tabs", false, appearance))
        .with_child(render_toggle_row(
            "Inverse Fill on Selection",
            state.inverse_fill_switch.clone(),
            inverse_fill,
            CortexSettingsAction::ToggleTabsInverseFillOnSelection,
            appearance,
        ))
        .with_child(render_alignment_row(
            "Title Alignment",
            "Centered",
            "Warp Default",
            state.selected_title_radio.clone(),
            state.selected_title_mouse_states.clone(),
            selected_title_idx,
            |index| {
                let value = match index {
                    0 => TabsSelectedTitleAlignment::Centered,
                    _ => TabsSelectedTitleAlignment::WarpDefault,
                };
                CortexSettingsAction::SetTabsSelectedTitleAlignment(value)
            },
            appearance,
        ))
        .with_child(render_alignment_row(
            "Metadata Alignment",
            "Centered Under Tab Title",
            "Warp Default (Left Aligned with Title)",
            state.selected_metadata_radio.clone(),
            state.selected_metadata_mouse_states.clone(),
            selected_metadata_idx,
            |index| {
                let value = match index {
                    0 => TabsSelectedMetadataAlignment::Centered,
                    _ => TabsSelectedMetadataAlignment::WarpDefault,
                };
                CortexSettingsAction::SetTabsSelectedMetadataAlignment(value)
            },
            appearance,
        ))
        // Unselected Tabs
        .with_child(render_subsection_header(
            "Unselected Tabs",
            false,
            appearance,
        ))
        .with_child(render_alignment_row(
            "Title Alignment",
            "Centered",
            "Warp Default",
            state.unselected_title_radio.clone(),
            state.unselected_title_mouse_states.clone(),
            unselected_title_idx,
            |index| {
                let value = match index {
                    0 => TabsUnselectedTitleAlignment::Centered,
                    _ => TabsUnselectedTitleAlignment::WarpDefault,
                };
                CortexSettingsAction::SetTabsUnselectedTitleAlignment(value)
            },
            appearance,
        ))
        .with_child(render_alignment_row(
            "Metadata Alignment",
            "Centered Under Tab Title",
            "Warp Default (Left Aligned with Title)",
            state.unselected_metadata_radio.clone(),
            state.unselected_metadata_mouse_states.clone(),
            unselected_metadata_idx,
            |index| {
                let value = match index {
                    0 => TabsUnselectedMetadataAlignment::Centered,
                    _ => TabsUnselectedMetadataAlignment::WarpDefault,
                };
                CortexSettingsAction::SetTabsUnselectedMetadataAlignment(value)
            },
            appearance,
        ))
        .finish()
}

fn render_subsection_header(
    label: &'static str,
    is_first: bool,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let font_family = appearance.ui_font_family();
    let font_size = appearance.ui_font_size() + SUBSECTION_HEADER_FONT_SIZE_DELTA;

    let text = Text::new_inline(label.to_string(), font_family, font_size)
        .with_color(theme.foreground().into())
        .finish();

    // Wrap in a row with `Start` alignment so the text reports its natural
    // (finite) width. A bare `Text::new_inline` placed directly inside the
    // page's stretched column can report infinite width and crashes scene
    // painting (`!rect.size().x().is_infinite()`). Same pattern the toggle
    // rows below use.
    let row = Flex::row()
        .with_main_axis_alignment(MainAxisAlignment::Start)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(text)
        .finish();

    Container::new(row)
        .with_margin_top(if is_first {
            SUBSECTION_HEADER_FIRST_TOP_MARGIN
        } else {
            SUBSECTION_HEADER_TOP_MARGIN
        })
        .with_margin_bottom(SUBSECTION_HEADER_BOTTOM_MARGIN)
        .finish()
}

fn render_toggle_row(
    label: &'static str,
    switch_state: SwitchStateHandle,
    current_value: bool,
    action: CortexSettingsAction,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();

    let label_element = Container::new(ui_builder.span(label.to_string()).build().finish())
        .with_margin_right(LABEL_RIGHT_MARGIN)
        .finish();

    let switch = ui_builder
        .switch(switch_state)
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
        })
        .finish();

    let row = Flex::row()
        .with_main_axis_alignment(MainAxisAlignment::Start)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(label_element)
        .with_child(switch)
        .finish();

    Container::new(row)
        .with_padding(Padding::uniform(ROW_VERTICAL_PADDING))
        .finish()
}

#[allow(clippy::too_many_arguments)]
fn render_alignment_row<F>(
    label: &'static str,
    centered_label: &'static str,
    warp_default_label: &'static str,
    radio_state: RadioButtonStateHandle,
    mouse_states: Vec<MouseStateHandle>,
    selected_index: usize,
    action_for_index: F,
    appearance: &Appearance,
) -> Box<dyn Element>
where
    F: Fn(usize) -> CortexSettingsAction + 'static,
{
    let ui_builder = appearance.ui_builder();

    let label_element = Container::new(ui_builder.span(label.to_string()).build().finish())
        .with_margin_right(LABEL_RIGHT_MARGIN)
        .finish();

    let action_for_index = Rc::new(action_for_index);

    let radio = ui_builder
        .radio_buttons(
            mouse_states,
            vec![
                RadioButtonItem::text(centered_label),
                RadioButtonItem::text(warp_default_label),
            ],
            radio_state,
            Some(selected_index),
            appearance.ui_font_size(),
            RadioButtonLayout::Row,
        )
        .on_change(Rc::new(move |ctx, _, index| {
            if let Some(index) = index {
                ctx.dispatch_typed_action(action_for_index(index));
            }
        }))
        .build()
        .finish();

    let row = Flex::row()
        .with_main_axis_alignment(MainAxisAlignment::Start)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(label_element)
        .with_child(radio)
        .finish();

    Container::new(row)
        .with_padding(Padding::uniform(ROW_VERTICAL_PADDING))
        .finish()
}

fn alignment_idx_selected_title(value: TabsSelectedTitleAlignment) -> usize {
    match value {
        TabsSelectedTitleAlignment::Centered => 0,
        TabsSelectedTitleAlignment::WarpDefault => 1,
    }
}

fn alignment_idx_selected_metadata(value: TabsSelectedMetadataAlignment) -> usize {
    match value {
        TabsSelectedMetadataAlignment::Centered => 0,
        TabsSelectedMetadataAlignment::WarpDefault => 1,
    }
}

fn alignment_idx_unselected_title(value: TabsUnselectedTitleAlignment) -> usize {
    match value {
        TabsUnselectedTitleAlignment::Centered => 0,
        TabsUnselectedTitleAlignment::WarpDefault => 1,
    }
}

fn alignment_idx_unselected_metadata(value: TabsUnselectedMetadataAlignment) -> usize {
    match value {
        TabsUnselectedMetadataAlignment::Centered => 0,
        TabsUnselectedMetadataAlignment::WarpDefault => 1,
    }
}
