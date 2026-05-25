//! Content rendered on the right side of the Cortex Settings pane when the
//! "Tabs" section is selected.
//!
//! Layout (top to bottom): Tab Style selector, live animated preview,
//! Tab Title font controls, unified Title/Metadata Alignment radios,
//! Tab Icons toggle, Vertical Tab Bar/Panel toggles. Sections are
//! separated by 2 px outline-colored bottom borders.
use std::rc::Rc;

use settings::Setting;
use warpui::{
    elements::{
        Align, Border, ChildView, ConstrainedBox, Container, CrossAxisAlignment, Element, Empty,
        Flex, MouseStateHandle, Padding, ParentElement, Shrinkable, Text,
    },
    fonts::{Properties, Weight},
    ui_components::{
        components::{Coords, UiComponent, UiComponentStyles},
        radio_buttons::{RadioButtonItem, RadioButtonLayout, RadioButtonStateHandle},
        slider::SliderStateHandle,
        switch::SwitchStateHandle,
    },
    AppContext, SingletonEntity, ViewContext, ViewHandle,
};

use crate::appearance::Appearance;
use crate::cortex_settings::action::CortexSettingsAction;
use crate::cortex_settings::tabs_preview;
use crate::cortex_settings::view::CortexSettingsView;
use crate::settings::{
    CortexSettings, TabStyle, TabsMetadataAlignment, TabsTitleAlignment,
};
use crate::view_components::{Dropdown, DropdownItem};

const ROW_VERTICAL_PADDING: f32 = 6.0;
const CONTROL_RIGHT_PADDING: f32 = 5.0;
const SUBSECTION_HEADER_FONT_SIZE: f32 = 16.0;
const SUBSECTION_HEADER_MARGIN_BOTTOM: f32 = 4.0;
const SUBSECTION_HEADER_PADDING_BOTTOM: f32 = 15.0;
const SECTION_SEPARATOR_BORDER_WIDTH: f32 = 2.0;
const SECTION_SEPARATOR_MARGIN_BOTTOM: f32 = 15.0;
const TAB_TITLE_FONT_NAME_DROPDOWN_WIDTH: f32 = 200.0;
const TAB_TITLE_FONT_SIZE_SLIDER_WIDTH: f32 = 160.0;
const TAB_TITLE_FONT_SIZE_MIN: f32 = 10.0;
const TAB_TITLE_FONT_SIZE_MAX: f32 = 20.0;

pub struct TabsPageState {
    // Tab Style
    tab_style_radio: RadioButtonStateHandle,
    tab_style_mouse_states: Vec<MouseStateHandle>,
    // Alignment (unified)
    title_alignment_radio: RadioButtonStateHandle,
    title_alignment_mouse_states: Vec<MouseStateHandle>,
    metadata_alignment_radio: RadioButtonStateHandle,
    metadata_alignment_mouse_states: Vec<MouseStateHandle>,
    // Tab Icons
    hide_icon_backdrop_switch: SwitchStateHandle,
    // Vertical Tab Bar/Panel
    panel_bg_switch: SwitchStateHandle,
    stack_left_column_switch: SwitchStateHandle,
    // Tab Title (font)
    pub(crate) title_font_name_dropdown: ViewHandle<Dropdown<CortexSettingsAction>>,
    title_font_size_slider: SliderStateHandle,
    title_font_weight_radio: RadioButtonStateHandle,
    title_font_weight_mouse_states: Vec<MouseStateHandle>,
    title_italic_switch: SwitchStateHandle,
}

impl TabsPageState {
    pub fn new(ctx: &mut ViewContext<CortexSettingsView>) -> Self {
        let title_font_name_dropdown = ctx.add_typed_action_view(|ctx| {
            let mut dropdown = Dropdown::new(ctx);
            dropdown.set_top_bar_max_width(TAB_TITLE_FONT_NAME_DROPDOWN_WIDTH);
            dropdown.set_menu_width(TAB_TITLE_FONT_NAME_DROPDOWN_WIDTH, ctx);
            let items = font_family_dropdown_items();
            dropdown.add_items(items, ctx);
            let initial_name = (*CortexSettings::as_ref(ctx).tabs_title_font_name.value()).clone();
            dropdown.set_selected_by_name(font_family_label_for_value(&initial_name), ctx);
            dropdown
        });

        Self {
            tab_style_radio: RadioButtonStateHandle::default(),
            tab_style_mouse_states: vec![MouseStateHandle::default()],
            title_alignment_radio: RadioButtonStateHandle::default(),
            title_alignment_mouse_states: vec![
                MouseStateHandle::default(),
                MouseStateHandle::default(),
            ],
            metadata_alignment_radio: RadioButtonStateHandle::default(),
            metadata_alignment_mouse_states: vec![
                MouseStateHandle::default(),
                MouseStateHandle::default(),
            ],
            hide_icon_backdrop_switch: SwitchStateHandle::default(),
            panel_bg_switch: SwitchStateHandle::default(),
            stack_left_column_switch: SwitchStateHandle::default(),
            title_font_name_dropdown,
            title_font_size_slider: SliderStateHandle::default(),
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

const TAB_TITLE_FONT_FAMILY_OPTIONS: &[(&str, &str)] = &[
    ("(use UI font)", ""),
    ("Hack", "Hack"),
    ("Roboto", "Roboto"),
];

fn font_family_dropdown_items() -> Vec<DropdownItem<CortexSettingsAction>> {
    TAB_TITLE_FONT_FAMILY_OPTIONS
        .iter()
        .map(|(label, value)| {
            DropdownItem::new(
                *label,
                CortexSettingsAction::SetTabTitleFontName((*value).to_string()),
            )
        })
        .collect()
}

fn font_family_label_for_value(value: &str) -> &'static str {
    TAB_TITLE_FONT_FAMILY_OPTIONS
        .iter()
        .find(|(_, v)| *v == value)
        .map(|(label, _)| *label)
        .unwrap_or(TAB_TITLE_FONT_FAMILY_OPTIONS[0].0)
}

pub fn tabs_page_search_terms() -> &'static [&'static str] {
    &[
        "tabs",
        "tab",
        "vertical",
        "panel",
        "background",
        "style",
        "modern",
        "preview",
        "animation",
        "text",
        "title",
        "metadata",
        "alignment",
        "centered",
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
    let tab_style_idx = tab_style_idx(*settings.tab_style.value());
    let title_alignment_idx = alignment_idx_title(*settings.tabs_title_alignment.value());
    let metadata_alignment_idx = alignment_idx_metadata(*settings.tabs_metadata_alignment.value());
    let hide_icon_backdrop = *settings.tabs_hide_icon_backdrop.value();
    let panel_bg = *settings.tabs_panel_matches_terminal_bg.value();
    let stack_left_column = *settings.stack_left_column.value();

    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        // 1. Tab Style
        .with_child(render_subsection_header("Tab Style", appearance))
        .with_child(render_alignment_row(
            "Style",
            "Cortex Modern",
            "",
            state.tab_style_radio.clone(),
            state.tab_style_mouse_states.clone(),
            tab_style_idx,
            |index| {
                let value = match index {
                    _ => TabStyle::CortexModern,
                };
                CortexSettingsAction::SetTabStyle(value)
            },
            appearance,
        ))
        .with_child(render_section_separator(appearance))
        // 2. Preview
        .with_child(render_subsection_header("Preview", appearance))
        .with_child(tabs_preview::render_preview_section(appearance, app))
        .with_child(render_section_separator(appearance))
        // 3. Tab Text (font + alignment, merged)
        .with_child(render_subsection_header("Tab Text", appearance))
        .with_child(render_tab_title_font_family_row(state, appearance))
        .with_child(render_tab_title_font_size_row(state, appearance, app))
        .with_child(render_tab_title_font_weight_row(state, appearance, app))
        .with_child(render_toggle_row(
            "Italic",
            state.title_italic_switch.clone(),
            *settings.tabs_title_italic.value(),
            CortexSettingsAction::ToggleTabTitleItalic,
            appearance,
        ))
        .with_child(render_alignment_row(
            "Title Alignment",
            "Centered",
            "Warp Default",
            state.title_alignment_radio.clone(),
            state.title_alignment_mouse_states.clone(),
            title_alignment_idx,
            |index| {
                let value = match index {
                    0 => TabsTitleAlignment::Centered,
                    _ => TabsTitleAlignment::WarpDefault,
                };
                CortexSettingsAction::SetTabsTitleAlignment(value)
            },
            appearance,
        ))
        .with_child(render_alignment_row(
            "Metadata Alignment",
            "Centered Under Tab Title",
            "Warp Default (Left Aligned with Title)",
            state.metadata_alignment_radio.clone(),
            state.metadata_alignment_mouse_states.clone(),
            metadata_alignment_idx,
            |index| {
                let value = match index {
                    0 => TabsMetadataAlignment::Centered,
                    _ => TabsMetadataAlignment::WarpDefault,
                };
                CortexSettingsAction::SetTabsMetadataAlignment(value)
            },
            appearance,
        ))
        .with_child(render_section_separator(appearance))
        // 6. Tab Icons
        .with_child(render_subsection_header("Tab Icons", appearance))
        .with_child(render_toggle_row(
            "Hide Icon Backdrop",
            state.hide_icon_backdrop_switch.clone(),
            hide_icon_backdrop,
            CortexSettingsAction::ToggleTabsHideIconBackdrop,
            appearance,
        ))
        .with_child(render_section_separator(appearance))
        // 7. Vertical Tab Bar/Panel (moved to bottom)
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
    let dropdown = ConstrainedBox::new(ChildView::new(&state.title_font_name_dropdown).finish())
        .with_width(TAB_TITLE_FONT_NAME_DROPDOWN_WIDTH)
        .finish();
    label_control_row(label, dropdown)
}

fn render_tab_title_font_size_row(
    state: &TabsPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let current = (*CortexSettings::as_ref(app).tabs_title_font_size.value())
        .clamp(TAB_TITLE_FONT_SIZE_MIN, TAB_TITLE_FONT_SIZE_MAX);
    let label = appearance
        .ui_builder()
        .span(format!("Font Size: {}", current.round() as u32))
        .build()
        .finish();
    let slider = appearance
        .ui_builder()
        .slider(state.title_font_size_slider.clone())
        .with_range(TAB_TITLE_FONT_SIZE_MIN..TAB_TITLE_FONT_SIZE_MAX)
        .with_default_value(current)
        .with_style(UiComponentStyles {
            width: Some(TAB_TITLE_FONT_SIZE_SLIDER_WIDTH),
            margin: Some(Coords::default().top(3.).bottom(3.)),
            ..Default::default()
        })
        .on_drag(|ctx, _, val| {
            ctx.dispatch_typed_action(CortexSettingsAction::SetTabTitleFontSize(val.round()));
        })
        .on_change(|ctx, _, val| {
            ctx.dispatch_typed_action(CortexSettingsAction::SetTabTitleFontSize(val.round()));
        })
        .build()
        .finish();
    label_control_row(label, slider)
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
        Weight::Thin
        | Weight::ExtraLight
        | Weight::Light
        | Weight::Normal
        | Weight::Medium
        | Weight::Semibold => 0,
        Weight::Bold | Weight::ExtraBold | Weight::Black => 1,
    }
}

fn radio_index_to_weight(index: usize) -> Weight {
    match index {
        0 => Weight::Normal,
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

    let header = Container::new(Align::new(text).left().finish())
        .with_margin_bottom(SUBSECTION_HEADER_MARGIN_BOTTOM)
        .finish();

    Container::new(header)
        .with_padding_bottom(SUBSECTION_HEADER_PADDING_BOTTOM)
        .finish()
}

fn render_section_separator(appearance: &Appearance) -> Box<dyn Element> {
    Container::new(Empty::new().finish())
        .with_border(
            Border::bottom(SECTION_SEPARATOR_BORDER_WIDTH)
                .with_border_fill(appearance.theme().outline()),
        )
        .with_margin_bottom(SECTION_SEPARATOR_MARGIN_BOTTOM)
        .finish()
}

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

    let mut items = vec![RadioButtonItem::text(centered_label)];
    if !warp_default_label.is_empty() {
        items.push(RadioButtonItem::text(warp_default_label));
    }

    let radio = ui_builder
        .radio_buttons(
            mouse_states,
            items,
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

fn tab_style_idx(value: TabStyle) -> usize {
    match value {
        TabStyle::CortexModern => 0,
    }
}

fn alignment_idx_title(value: TabsTitleAlignment) -> usize {
    match value {
        TabsTitleAlignment::Centered => 0,
        TabsTitleAlignment::WarpDefault => 1,
    }
}

fn alignment_idx_metadata(value: TabsMetadataAlignment) -> usize {
    match value {
        TabsMetadataAlignment::Centered => 0,
        TabsMetadataAlignment::WarpDefault => 1,
    }
}
