//! Content rendered on the right side of the Cortex Settings pane when the
//! "Top Bar" section is selected.
//!
//! Houses settings that affect the title/application bar: whether its
//! background matches the terminal and the visual style of the embedded
//! search bar.
use std::rc::Rc;

use settings::Setting as _;
use warpui::{
    elements::{
        Align, Container, CrossAxisAlignment, Element, Flex, MouseStateHandle, Padding,
        ParentElement, Shrinkable,
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
use crate::settings::{CortexSettings, SearchBarStyle};

const ROW_VERTICAL_PADDING: f32 = 6.0;
const CONTROL_RIGHT_PADDING: f32 = 5.0;

pub struct TopBarPageState {
    matches_terminal_bg_switch: SwitchStateHandle,
    hide_divider_switch: SwitchStateHandle,
    search_bar_style_radio: RadioButtonStateHandle,
    search_bar_style_mouse_states: Vec<MouseStateHandle>,
}

impl Default for TopBarPageState {
    fn default() -> Self {
        Self {
            matches_terminal_bg_switch: SwitchStateHandle::default(),
            hide_divider_switch: SwitchStateHandle::default(),
            search_bar_style_radio: RadioButtonStateHandle::default(),
            search_bar_style_mouse_states: vec![
                MouseStateHandle::default(),
                MouseStateHandle::default(),
            ],
        }
    }
}

pub fn top_bar_page_search_terms() -> &'static [&'static str] {
    &[
        "top",
        "bar",
        "title",
        "search",
        "background",
        "terminal",
        "border",
        "style",
        "transparent",
        "divider",
        "line",
        "separator",
    ]
}

pub fn render_top_bar_page(
    state: &TopBarPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(render_matches_terminal_bg_row(state, appearance, app))
        .with_child(render_hide_divider_row(state, appearance, app))
        .with_child(render_search_bar_style_row(state, appearance, app))
        .finish()
}

fn render_matches_terminal_bg_row(
    state: &TopBarPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let current_value = *CortexSettings::as_ref(app).top_bar_matches_terminal_bg;

    let label = ui_builder
        .span("Top Bar Matches Terminal Background Color".to_string())
        .build()
        .finish();

    let switch = ui_builder
        .switch(state.matches_terminal_bg_switch.clone())
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(CortexSettingsAction::ToggleTopBarMatchesTerminalBg);
        })
        .finish();

    label_control_row(label, switch)
}

fn render_hide_divider_row(
    state: &TopBarPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let current_value = *CortexSettings::as_ref(app).top_bar_hide_divider;

    let label = ui_builder
        .span("Hide Top Bar Divider Line".to_string())
        .build()
        .finish();

    let switch = ui_builder
        .switch(state.hide_divider_switch.clone())
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(CortexSettingsAction::ToggleTopBarHideDivider);
        })
        .finish();

    label_control_row(label, switch)
}

fn render_search_bar_style_row(
    state: &TopBarPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let current_value = *CortexSettings::as_ref(app).top_bar_search_bar_style.value();
    let selected_index = search_bar_style_to_idx(current_value);

    let label = ui_builder
        .span("Search Bar Style".to_string())
        .build()
        .finish();

    let radio = ui_builder
        .radio_buttons(
            state.search_bar_style_mouse_states.clone(),
            vec![
                RadioButtonItem::text("Cortex Default"),
                RadioButtonItem::text("Warp Default"),
            ],
            state.search_bar_style_radio.clone(),
            Some(selected_index),
            appearance.ui_font_size(),
            RadioButtonLayout::Row,
        )
        .on_change(Rc::new(move |ctx, _, index| {
            if let Some(index) = index {
                let value = match index {
                    0 => SearchBarStyle::CortexDefault,
                    _ => SearchBarStyle::WarpDefault,
                };
                ctx.dispatch_typed_action(CortexSettingsAction::SetTopBarSearchBarStyle(value));
            }
        }))
        .build()
        .finish();

    label_control_row(label, radio)
}

fn search_bar_style_to_idx(value: SearchBarStyle) -> usize {
    match value {
        SearchBarStyle::CortexDefault => 0,
        SearchBarStyle::WarpDefault => 1,
    }
}

fn label_control_row(
    label: Box<dyn Element>,
    control: Box<dyn Element>,
) -> Box<dyn Element> {
    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label).left().finish()).finish(),
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
