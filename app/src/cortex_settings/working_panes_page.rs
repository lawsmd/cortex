//! Content rendered on the right side of the Cortex Settings pane when the
//! "Panes" section is selected.
//!
//! Houses the pane-chrome and session-recap toggles. New pane-level toggles
//! go here.
use warpui::{
    elements::{
        Align, ChildView, ConstrainedBox, Container, CrossAxisAlignment, Element, Flex, Padding,
        ParentElement, Shrinkable,
    },
    ui_components::{components::UiComponent, switch::SwitchStateHandle},
    AppContext, SingletonEntity, ViewContext, ViewHandle,
};

use crate::appearance::Appearance;
use crate::cortex_settings::action::CortexSettingsAction;
use crate::cortex_settings::font_options::{font_family_dropdown_items, font_family_label_for_value};
use crate::cortex_settings::view::CortexSettingsView;
use crate::settings::CortexSettings;
use crate::view_components::Dropdown;
use settings::Setting;

const ROW_VERTICAL_PADDING: f32 = 6.0;
const CONTROL_RIGHT_PADDING: f32 = 5.0;
const PANE_TITLE_FONT_NAME_DROPDOWN_WIDTH: f32 = 200.0;

/// Per-toggle UI state that has to outlive a single render frame (mouse-state
/// handles for hover detection, switch animation state, etc.). Owned by
/// `CortexSettingsView` and threaded into the render fns via `&self`.
pub struct WorkingPanesPageState {
    hide_pane_separators_switch: SwitchStateHandle,
    rounded_pane_borders_switch: SwitchStateHandle,
    start_with_blank_pane_switch: SwitchStateHandle,
    recap_matches_terminal_style_switch: SwitchStateHandle,
    pub(crate) title_font_name_dropdown: ViewHandle<Dropdown<CortexSettingsAction>>,
}

impl WorkingPanesPageState {
    pub fn new(ctx: &mut ViewContext<CortexSettingsView>) -> Self {
        let title_font_name_dropdown = ctx.add_typed_action_view(|ctx| {
            let mut dropdown = Dropdown::new(ctx);
            dropdown.set_top_bar_max_width(PANE_TITLE_FONT_NAME_DROPDOWN_WIDTH);
            dropdown.set_menu_width(PANE_TITLE_FONT_NAME_DROPDOWN_WIDTH, ctx);
            let items =
                font_family_dropdown_items(CortexSettingsAction::SetPaneTitleFontName);
            dropdown.add_items(items, ctx);
            let initial_name =
                (*CortexSettings::as_ref(ctx).panes_title_font_name.value()).clone();
            dropdown.set_selected_by_name(font_family_label_for_value(&initial_name), ctx);
            dropdown
        });

        Self {
            hide_pane_separators_switch: SwitchStateHandle::default(),
            rounded_pane_borders_switch: SwitchStateHandle::default(),
            start_with_blank_pane_switch: SwitchStateHandle::default(),
            recap_matches_terminal_style_switch: SwitchStateHandle::default(),
            title_font_name_dropdown,
        }
    }
}

pub fn working_panes_page_search_terms() -> &'static [&'static str] {
    &[
        "working", "panes", "pane", "hide", "separator", "lines", "border", "borders", "rounded",
        "tui", "sidebar", "input", "recap", "restored", "session", "launch", "blank", "match",
        "style", "gray", "font", "family", "title", "fira", "hack", "roboto",
    ]
}

pub fn render_working_panes_page(
    state: &WorkingPanesPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(render_pane_title_font_family_row(state, appearance))
        .with_child(render_hide_pane_separators_row(state, appearance, app))
        .with_child(render_rounded_pane_borders_row(state, appearance, app))
        .with_child(render_start_with_blank_pane_row(state, appearance, app))
        .with_child(render_recap_matches_terminal_style_row(
            state, appearance, app,
        ))
        .finish()
}

fn render_pane_title_font_family_row(
    state: &WorkingPanesPageState,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let label = appearance
        .ui_builder()
        .span("Pane Title Font".to_string())
        .build()
        .finish();
    let dropdown = ConstrainedBox::new(
        ChildView::new(&state.title_font_name_dropdown).finish(),
    )
    .with_width(PANE_TITLE_FONT_NAME_DROPDOWN_WIDTH)
    .finish();

    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label).left().finish()).finish(),
    )
    .finish();

    let control = Container::new(dropdown)
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

fn render_hide_pane_separators_row(
    state: &WorkingPanesPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let current_value = *CortexSettings::as_ref(app).hide_pane_separators;

    let label = ui_builder
        .span("Hide Sidebar & Input Borders".to_string())
        .build()
        .finish();

    let switch = ui_builder
        .switch(state.hide_pane_separators_switch.clone())
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(CortexSettingsAction::ToggleHidePaneSeparators);
        })
        .finish();

    // Mirrors the Warp `build_toggle_element` shape (settings_page.rs:778):
    // label hugs the left edge of the centered max-width column, control
    // hugs the right.
    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label).left().finish()).finish(),
    )
    .finish();

    let control = Container::new(switch)
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

fn render_rounded_pane_borders_row(
    state: &WorkingPanesPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let current_value = *CortexSettings::as_ref(app).rounded_pane_borders;

    let label = ui_builder
        .span("Use Rounded Pane Borders".to_string())
        .build()
        .finish();

    let switch = ui_builder
        .switch(state.rounded_pane_borders_switch.clone())
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(CortexSettingsAction::ToggleRoundedPaneBorders);
        })
        .finish();

    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label).left().finish()).finish(),
    )
    .finish();

    let control = Container::new(switch)
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

fn render_start_with_blank_pane_row(
    state: &WorkingPanesPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let current_value = *CortexSettings::as_ref(app).start_with_blank_pane_on_launch;

    let label = ui_builder
        .span("Hide Previous Session Recap on Launch".to_string())
        .build()
        .finish();

    let switch = ui_builder
        .switch(state.start_with_blank_pane_switch.clone())
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(CortexSettingsAction::ToggleStartWithBlankPaneOnLaunch);
        })
        .finish();

    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label).left().finish()).finish(),
    )
    .finish();

    let control = Container::new(switch)
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

fn render_recap_matches_terminal_style_row(
    state: &WorkingPanesPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let current_value = *CortexSettings::as_ref(app).recap_matches_terminal_style;

    let label = ui_builder
        .span("Match Recap Style to Active Terminal".to_string())
        .build()
        .finish();

    let switch = ui_builder
        .switch(state.recap_matches_terminal_style_switch.clone())
        .check(current_value)
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(CortexSettingsAction::ToggleRecapMatchesTerminalStyle);
        })
        .finish();

    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label).left().finish()).finish(),
    )
    .finish();

    let control = Container::new(switch)
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
