//! Content rendered on the right side of the Cortex Settings pane when the
//! "Tabs" section is selected.
//!
//! Layout: subsection headers (`Vertical Tab Bar/Panel`, `Selected Tabs`,
//! `Unselected Tabs`, `Tab Icons`), each followed by its toggles/selectors and
//! separated from the next by a 2 px outline-colored bottom border —
//! mirroring Warp's `render_separator` between categories. Subsection
//! headers are plain styled `Text` rows; row layout follows the Warp
//! `build_toggle_element` shape (label hugs the left edge of the centered
//! max-width column, control hugs the right).
use std::rc::Rc;

use settings::Setting;
use warpui::{
    elements::{
        Align, Border, ChildView, ConstrainedBox, Container, CrossAxisAlignment, Element, Empty,
        Flex, MouseStateHandle, Padding, ParentElement, Shrinkable, Text,
    },
    fonts::{Properties, Weight},
    ui_components::{
        components::UiComponent,
        radio_buttons::{RadioButtonItem, RadioButtonLayout, RadioButtonStateHandle},
        switch::SwitchStateHandle,
    },
    AppContext, SingletonEntity, ViewContext, ViewHandle,
};

use crate::appearance::Appearance;
use crate::cortex_settings::action::CortexSettingsAction;
use crate::cortex_settings::view::CortexSettingsView;
use crate::editor::{
    EditorView, PropagateAndNoOpNavigationKeys, SingleLineEditorOptions, TextOptions,
};
use crate::settings::{
    CortexSettings, TabsSelectedMetadataAlignment, TabsSelectedTitleAlignment,
    TabsUnselectedMetadataAlignment, TabsUnselectedTitleAlignment,
};

const ROW_VERTICAL_PADDING: f32 = 6.0;
const CONTROL_RIGHT_PADDING: f32 = 5.0;
// Mirror Warp's `SUBHEADER_FONT_SIZE` / `SUBHEADER_MARGIN_BOTTOM` /
// `HEADER_PADDING` so the section headers across both Settings menus render
// at identical size, weight, and spacing.
const SUBSECTION_HEADER_FONT_SIZE: f32 = 16.0;
const SUBSECTION_HEADER_MARGIN_BOTTOM: f32 = 4.0;
const SUBSECTION_HEADER_PADDING_BOTTOM: f32 = 15.0;
// Section separator — matches Warp's `render_separator` (2 px bottom border
// in the theme outline color, with `HEADER_PADDING = 15` margin below), used
// to space neighboring section groups apart.
const SECTION_SEPARATOR_BORDER_WIDTH: f32 = 2.0;
const SECTION_SEPARATOR_MARGIN_BOTTOM: f32 = 15.0;
// Tab Title text inputs need explicit widths — `EditorElement::layout` panics
// on infinite width (`app/src/editor/view/element.rs:1670`), and a raw
// `ChildView<EditorView>` reports infinite width to the parent flex unless
// constrained. The label-control row helper only shrinks the label side.
const TAB_TITLE_FONT_NAME_INPUT_WIDTH: f32 = 200.0;
const TAB_TITLE_FONT_SIZE_INPUT_WIDTH: f32 = 64.0;

/// Per-toggle/selector UI state that has to outlive a single render frame
/// (mouse-state handles for hover detection, switch animation, radio-button
/// selection state, plus the two `EditorView` handles for the Tab Title font
/// name + size text inputs). Owned by `CortexSettingsView` and threaded into
/// the render fns via `&self`.
///
/// No `Default` impl — the editor handles need a `ViewContext` to construct,
/// so this is built via [`TabsPageState::new`] from `CortexSettingsView::new`.
pub struct TabsPageState {
    panel_bg_switch: SwitchStateHandle,
    inverse_fill_switch: SwitchStateHandle,
    hide_icon_backdrop_switch: SwitchStateHandle,
    stack_left_column_switch: SwitchStateHandle,
    selected_title_radio: RadioButtonStateHandle,
    selected_title_mouse_states: Vec<MouseStateHandle>,
    selected_metadata_radio: RadioButtonStateHandle,
    selected_metadata_mouse_states: Vec<MouseStateHandle>,
    unselected_title_radio: RadioButtonStateHandle,
    unselected_title_mouse_states: Vec<MouseStateHandle>,
    unselected_metadata_radio: RadioButtonStateHandle,
    unselected_metadata_mouse_states: Vec<MouseStateHandle>,

    /// Tab Title subsection — font family (text input; empty = inherit UI font).
    pub(crate) title_font_name_editor: ViewHandle<EditorView>,
    /// Tab Title subsection — font size (numeric text input; clamped to 8..=32).
    pub(crate) title_font_size_editor: ViewHandle<EditorView>,
    /// Tab Title subsection — font weight radio (3 options: Normal / Medium / Bold).
    title_font_weight_radio: RadioButtonStateHandle,
    title_font_weight_mouse_states: Vec<MouseStateHandle>,
    /// Tab Title subsection — italic toggle.
    title_italic_switch: SwitchStateHandle,
}

impl TabsPageState {
    /// Build the page state inside `CortexSettingsView::new`.
    ///
    /// The two `EditorView` handles for Tab Title font name + size are created
    /// here and the parent view subscribes to their `Edited` events so each
    /// keystroke writes through to `CortexSettings`. Pre-populates the editors
    /// with the current setting value.
    pub fn new(ctx: &mut ViewContext<CortexSettingsView>) -> Self {
        let appearance = Appearance::as_ref(ctx);
        let font_family = appearance.ui_font_family();
        let cortex = CortexSettings::as_ref(ctx);
        let initial_name = (*cortex.tabs_title_font_name.value()).clone();
        let initial_size = *cortex.tabs_title_font_size.value();

        let title_font_name_editor = ctx.add_typed_action_view(|ctx| {
            let options = SingleLineEditorOptions {
                text: TextOptions {
                    font_family_override: Some(font_family),
                    ..Default::default()
                },
                propagate_and_no_op_vertical_navigation_keys:
                    PropagateAndNoOpNavigationKeys::Always,
                ..Default::default()
            };
            let mut editor = EditorView::single_line(options, ctx);
            if !initial_name.is_empty() {
                editor.set_buffer_text(&initial_name, ctx);
            }
            editor.set_placeholder_text("(use UI font)", ctx);
            editor
        });
        ctx.subscribe_to_view(
            &title_font_name_editor,
            CortexSettingsView::handle_tab_title_font_name_editor_event,
        );

        let title_font_size_editor = ctx.add_typed_action_view(|ctx| {
            let options = SingleLineEditorOptions {
                text: TextOptions {
                    font_family_override: Some(font_family),
                    ..Default::default()
                },
                propagate_and_no_op_vertical_navigation_keys:
                    PropagateAndNoOpNavigationKeys::Always,
                ..Default::default()
            };
            let mut editor = EditorView::single_line(options, ctx);
            editor.set_buffer_text(&format!("{}", initial_size), ctx);
            editor.set_placeholder_text("12", ctx);
            editor
        });
        ctx.subscribe_to_view(
            &title_font_size_editor,
            CortexSettingsView::handle_tab_title_font_size_editor_event,
        );

        Self {
            panel_bg_switch: SwitchStateHandle::default(),
            inverse_fill_switch: SwitchStateHandle::default(),
            hide_icon_backdrop_switch: SwitchStateHandle::default(),
            stack_left_column_switch: SwitchStateHandle::default(),
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
            title_font_name_editor,
            title_font_size_editor,
            title_font_weight_radio: RadioButtonStateHandle::default(),
            title_font_weight_mouse_states: vec![
                MouseStateHandle::default(),
                MouseStateHandle::default(),
                MouseStateHandle::default(),
            ],
            title_italic_switch: SwitchStateHandle::default(),
        }
    }
}

pub fn tabs_page_search_terms() -> &'static [&'static str] {
    &[
        "tabs",
        "tab",
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
        "icon",
        "icons",
        "backdrop",
        "circle",
        "font",
        "family",
        "size",
        "weight",
        "bold",
        "medium",
        "italic",
    ]
}

pub fn render_tabs_page(
    state: &TabsPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let settings = CortexSettings::as_ref(app);
    let panel_bg = *settings.tabs_panel_matches_terminal_bg.value();
    let inverse_fill = *settings.tabs_inverse_fill_on_selection.value();
    let hide_icon_backdrop = *settings.tabs_hide_icon_backdrop.value();
    let stack_left_column = *settings.stack_left_column.value();
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
        .with_child(render_subsection_header("Vertical Tab Bar/Panel", appearance))
        .with_child(render_toggle_row(
            "Bar/Panel Background Matches Terminal Background",
            state.panel_bg_switch.clone(),
            panel_bg,
            CortexSettingsAction::ToggleTabsPanelMatchesTerminalBg,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Stack Vertical Tab Bar Over Side Panel",
            state.stack_left_column_switch.clone(),
            stack_left_column,
            CortexSettingsAction::ToggleStackLeftColumn,
            appearance,
        ))
        .with_child(render_section_separator(appearance))
        // Selected Tabs
        .with_child(render_subsection_header("Selected Tabs", appearance))
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
        .with_child(render_section_separator(appearance))
        // Unselected Tabs
        .with_child(render_subsection_header("Unselected Tabs", appearance))
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
        .with_child(render_section_separator(appearance))
        // Tab Icons
        .with_child(render_subsection_header("Tab Icons", appearance))
        .with_child(render_toggle_row(
            "Hide Icon Backdrop",
            state.hide_icon_backdrop_switch.clone(),
            hide_icon_backdrop,
            CortexSettingsAction::ToggleTabsHideIconBackdrop,
            appearance,
        ))
        .with_child(render_section_separator(appearance))
        // Tab Title — font customizations (apply to both horizontal tab bar
        // and vertical tab rail; metadata/subtitle lines stay on the UI font).
        .with_child(render_subsection_header("Tab Title", appearance))
        .with_child(render_tab_title_font_family_row(state, appearance))
        .with_child(render_tab_title_font_size_row(state, appearance))
        .with_child(render_tab_title_font_weight_row(state, appearance, app))
        .with_child(render_toggle_row(
            "Italic",
            state.title_italic_switch.clone(),
            *settings.tabs_title_italic.value(),
            CortexSettingsAction::ToggleTabTitleItalic,
            appearance,
        ))
        .finish()
}

fn render_tab_title_font_family_row(
    state: &TabsPageState,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let label = appearance
        .ui_builder()
        .span("Font Family".to_string())
        .build()
        .finish();
    let editor = ConstrainedBox::new(ChildView::new(&state.title_font_name_editor).finish())
        .with_width(TAB_TITLE_FONT_NAME_INPUT_WIDTH)
        .finish();
    label_control_row(label, editor)
}

fn render_tab_title_font_size_row(
    state: &TabsPageState,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let label = appearance
        .ui_builder()
        .span("Font Size".to_string())
        .build()
        .finish();
    let editor = ConstrainedBox::new(ChildView::new(&state.title_font_size_editor).finish())
        .with_width(TAB_TITLE_FONT_SIZE_INPUT_WIDTH)
        .finish();
    label_control_row(label, editor)
}

fn render_tab_title_font_weight_row(
    state: &TabsPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let label = ui_builder.span("Font Weight".to_string()).build().finish();

    let current_weight = *CortexSettings::as_ref(app).tabs_title_font_weight.value();
    let selected_index = weight_to_radio_index(current_weight);

    let radio = ui_builder
        .radio_buttons(
            state.title_font_weight_mouse_states.clone(),
            vec![
                RadioButtonItem::text("Normal"),
                RadioButtonItem::text("Medium"),
                RadioButtonItem::text("Bold"),
            ],
            state.title_font_weight_radio.clone(),
            Some(selected_index),
            appearance.ui_font_size(),
            RadioButtonLayout::Row,
        )
        .on_change(Rc::new(|ctx, _, index| {
            if let Some(index) = index {
                ctx.dispatch_typed_action(CortexSettingsAction::SetTabTitleFontWeight(
                    radio_index_to_weight(index),
                ));
            }
        }))
        .build()
        .finish();

    label_control_row(label, radio)
}

fn weight_to_radio_index(weight: Weight) -> usize {
    match weight {
        Weight::Thin | Weight::ExtraLight | Weight::Light | Weight::Normal => 0,
        Weight::Medium | Weight::Semibold => 1,
        Weight::Bold | Weight::ExtraBold | Weight::Black => 2,
    }
}

fn radio_index_to_weight(index: usize) -> Weight {
    match index {
        0 => Weight::Normal,
        1 => Weight::Medium,
        _ => Weight::Bold,
    }
}

fn render_subsection_header(label: &'static str, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let font_family = appearance.ui_font_family();

    let text = Text::new_inline(label.to_string(), font_family, SUBSECTION_HEADER_FONT_SIZE)
        .with_style(Properties::default().weight(Weight::Bold))
        .with_color(theme.active_ui_text_color().into())
        .finish();

    // `Align::left` reports a finite width for the text. A bare
    // `Text::new_inline` placed directly inside the page's stretched column
    // can report infinite width and crashes scene painting
    // (`!rect.size().x().is_infinite()`).
    let header = Container::new(Align::new(text).left().finish())
        .with_margin_bottom(SUBSECTION_HEADER_MARGIN_BOTTOM)
        .finish();

    Container::new(header)
        .with_padding_bottom(SUBSECTION_HEADER_PADDING_BOTTOM)
        .finish()
}

/// Mirrors Warp's `render_separator` (settings_page.rs:381): a 2 px bottom
/// border in the theme outline color with `HEADER_PADDING` margin below, used
/// to space neighboring section groups apart.
fn render_section_separator(appearance: &Appearance) -> Box<dyn Element> {
    Container::new(Empty::new().finish())
        .with_border(
            Border::bottom(SECTION_SEPARATOR_BORDER_WIDTH)
                .with_border_fill(appearance.theme().outline()),
        )
        .with_margin_bottom(SECTION_SEPARATOR_MARGIN_BOTTOM)
        .finish()
}

/// Lays out a row in the same shape Warp Settings uses
/// (`build_toggle_element` in `settings_view/settings_page.rs`): the label
/// expands to fill the available width and pushes the control to the right
/// edge of the centered max-width column.
fn label_control_row(
    label_element: Box<dyn Element>,
    control: Box<dyn Element>,
) -> Box<dyn Element> {
    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label_element).left().finish()).finish(),
    )
    .finish();

    let control = Container::new(control)
        .with_padding_right(CONTROL_RIGHT_PADDING)
        .finish();

    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(header)
        .with_child(control)
        .finish();

    Container::new(row)
        .with_padding(Padding::uniform(ROW_VERTICAL_PADDING))
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

    let label_element = ui_builder.span(label.to_string()).build().finish();

    let switch = ui_builder
        .switch(switch_state)
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(action.clone());
        })
        .finish();

    label_control_row(label_element, switch)
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

    let label_element = ui_builder.span(label.to_string()).build().finish();

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

    label_control_row(label_element, radio)
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
