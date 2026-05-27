//! Content rendered on the right side of the Cortex Settings pane when the
//! "Top Bar" section is selected.
//!
//! Houses settings that affect the title/application bar: background-matching,
//! divider hiding, search-bar opacity / compactness, individual top-bar button
//! visibility toggles, top-bar font family, and the generic-profile-icon
//! override.
use settings::Setting as _;
use warpui::{
    elements::{
        Align, ChildView, ConstrainedBox, Container, CrossAxisAlignment, Element, Flex, Padding,
        ParentElement, Shrinkable,
    },
    ui_components::{
        components::{Coords, UiComponent, UiComponentStyles},
        slider::SliderStateHandle,
        switch::SwitchStateHandle,
    },
    AppContext, SingletonEntity, ViewContext, ViewHandle,
};

const SLIDER_VALUE_GAP: f32 = 8.0;

use crate::appearance::Appearance;
use crate::cortex_settings::action::CortexSettingsAction;
use crate::cortex_settings::font_options::{
    font_family_dropdown_items, font_family_label_for_value,
};
use crate::cortex_settings::view::CortexSettingsView;
use crate::settings::CortexSettings;
use crate::view_components::Dropdown;

const ROW_VERTICAL_PADDING: f32 = 6.0;
const CONTROL_RIGHT_PADDING: f32 = 5.0;
const SEARCH_BAR_OPACITY_SLIDER_WIDTH: f32 = 160.0;
const SEARCH_BAR_OPACITY_MIN: f32 = 10.0;
const SEARCH_BAR_OPACITY_MAX: f32 = 100.0;
const TOP_BAR_FONT_NAME_DROPDOWN_WIDTH: f32 = 200.0;

pub struct TopBarPageState {
    matches_terminal_bg_switch: SwitchStateHandle,
    hide_divider_switch: SwitchStateHandle,
    search_bar_opacity_slider: SliderStateHandle,
    search_bar_compact_switch: SwitchStateHandle,
    hide_tabs_panel_collapse_button_switch: SwitchStateHandle,
    hide_agent_management_button_switch: SwitchStateHandle,
    hide_notifications_button_switch: SwitchStateHandle,
    pub(crate) font_name_dropdown: ViewHandle<Dropdown<CortexSettingsAction>>,
    generic_profile_icon_switch: SwitchStateHandle,
}

impl TopBarPageState {
    pub fn new(ctx: &mut ViewContext<CortexSettingsView>) -> Self {
        let font_name_dropdown = ctx.add_typed_action_view(|ctx| {
            let mut dropdown = Dropdown::new(ctx);
            dropdown.set_top_bar_max_width(TOP_BAR_FONT_NAME_DROPDOWN_WIDTH);
            dropdown.set_menu_width(TOP_BAR_FONT_NAME_DROPDOWN_WIDTH, ctx);
            let items = font_family_dropdown_items(CortexSettingsAction::SetTopBarFontName);
            dropdown.add_items(items, ctx);
            let initial_name = (*CortexSettings::as_ref(ctx).top_bar_font_name.value()).clone();
            dropdown.set_selected_by_name(font_family_label_for_value(&initial_name), ctx);
            dropdown
        });

        Self {
            matches_terminal_bg_switch: SwitchStateHandle::default(),
            hide_divider_switch: SwitchStateHandle::default(),
            search_bar_opacity_slider: SliderStateHandle::default(),
            search_bar_compact_switch: SwitchStateHandle::default(),
            hide_tabs_panel_collapse_button_switch: SwitchStateHandle::default(),
            hide_agent_management_button_switch: SwitchStateHandle::default(),
            hide_notifications_button_switch: SwitchStateHandle::default(),
            font_name_dropdown,
            generic_profile_icon_switch: SwitchStateHandle::default(),
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
        "transparent",
        "divider",
        "line",
        "separator",
        "opacity",
        "compact",
        "placeholder",
        "hide",
        "tabs",
        "panel",
        "collapse",
        "agent",
        "management",
        "notifications",
        "font",
        "family",
        "profile",
        "avatar",
        "icon",
        "generic",
    ]
}

pub fn render_top_bar_page(
    state: &TopBarPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let settings = CortexSettings::as_ref(app);

    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(render_toggle_row(
            "Top Bar Matches Terminal Background Color",
            state.matches_terminal_bg_switch.clone(),
            *settings.top_bar_matches_terminal_bg,
            CortexSettingsAction::ToggleTopBarMatchesTerminalBg,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Hide Top Bar Divider Line",
            state.hide_divider_switch.clone(),
            *settings.top_bar_hide_divider,
            CortexSettingsAction::ToggleTopBarHideDivider,
            appearance,
        ))
        .with_child(render_search_bar_opacity_row(state, appearance, app))
        .with_child(render_toggle_row(
            "Compact Search Bar",
            state.search_bar_compact_switch.clone(),
            *settings.top_bar_search_bar_compact.value(),
            CortexSettingsAction::ToggleTopBarSearchBarCompact,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Hide Tabs Panel Collapse Button",
            state.hide_tabs_panel_collapse_button_switch.clone(),
            *settings.top_bar_hide_tabs_panel_collapse_button.value(),
            CortexSettingsAction::ToggleTopBarHideTabsPanelCollapseButton,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Hide Agent Management Panel Button",
            state.hide_agent_management_button_switch.clone(),
            *settings.top_bar_hide_agent_management_button.value(),
            CortexSettingsAction::ToggleTopBarHideAgentManagementButton,
            appearance,
        ))
        .with_child(render_toggle_row(
            "Hide Notifications Button",
            state.hide_notifications_button_switch.clone(),
            *settings.top_bar_hide_notifications_button.value(),
            CortexSettingsAction::ToggleTopBarHideNotificationsButton,
            appearance,
        ))
        .with_child(render_top_bar_font_row(state, appearance))
        .with_child(render_toggle_row(
            "Replace Profile Button with Generic Icon",
            state.generic_profile_icon_switch.clone(),
            *settings.top_bar_generic_profile_icon.value(),
            CortexSettingsAction::ToggleTopBarGenericProfileIcon,
            appearance,
        ))
        .finish()
}

fn render_search_bar_opacity_row(
    state: &TopBarPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let current = (*CortexSettings::as_ref(app)
        .top_bar_search_bar_opacity
        .value())
    .clamp(SEARCH_BAR_OPACITY_MIN as u8, SEARCH_BAR_OPACITY_MAX as u8);
    let ui_builder = appearance.ui_builder();
    let label = ui_builder
        .span("Search Bar Opacity".to_string())
        .build()
        .finish();
    let slider = ui_builder
        .slider(state.search_bar_opacity_slider.clone())
        .with_range(SEARCH_BAR_OPACITY_MIN..SEARCH_BAR_OPACITY_MAX)
        .with_default_value(current as f32)
        .with_style(UiComponentStyles {
            width: Some(SEARCH_BAR_OPACITY_SLIDER_WIDTH),
            margin: Some(Coords::default().top(3.).bottom(3.)),
            ..Default::default()
        })
        .on_drag(|ctx, _, val| {
            ctx.dispatch_typed_action(CortexSettingsAction::SetTopBarSearchBarOpacity(
                val.round() as u8,
            ));
        })
        .on_change(|ctx, _, val| {
            ctx.dispatch_typed_action(CortexSettingsAction::SetTopBarSearchBarOpacity(
                val.round() as u8,
            ));
        })
        .build()
        .finish();
    let value_text = ui_builder
        .span(format!("{}%", current))
        .build()
        .finish();
    let slider_with_value = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_spacing(SLIDER_VALUE_GAP)
        .with_child(slider)
        .with_child(value_text)
        .finish();
    label_control_row(label, slider_with_value)
}

fn render_top_bar_font_row(
    state: &TopBarPageState,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let label = appearance
        .ui_builder()
        .span("Top Bar Font".to_string())
        .build()
        .finish();
    let dropdown = ConstrainedBox::new(ChildView::new(&state.font_name_dropdown).finish())
        .with_width(TOP_BAR_FONT_NAME_DROPDOWN_WIDTH)
        .finish();
    label_control_row(label, dropdown)
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
