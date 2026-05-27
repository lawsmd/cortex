//! Content rendered on the right side of the Cortex Settings pane when the
//! "File Explorer" section is selected.
//!
//! Houses a font selector and toggles controlling the Yazi-inspired TUI file
//! explorer style: tree-drawing characters, Nerd Font icons, and per-file-type
//! icon colors.
use settings::Setting;
use warpui::{
    elements::{
        Align, ChildView, ConstrainedBox, Container, CrossAxisAlignment, Element, Flex, Padding,
        ParentElement, Shrinkable,
    },
    ui_components::{components::UiComponent, switch::SwitchStateHandle},
    AppContext, SingletonEntity, ViewHandle,
};

use crate::appearance::Appearance;
use crate::cortex_settings::action::CortexSettingsAction;
use crate::cortex_settings::view::CortexSettingsView;
use crate::settings::CortexSettings;
use crate::view_components::{Dropdown, DropdownItem};

const ROW_VERTICAL_PADDING: f32 = 6.0;
const CONTROL_RIGHT_PADDING: f32 = 5.0;
const FONT_NAME_DROPDOWN_WIDTH: f32 = 260.0;

const FONT_FAMILY_OPTIONS: &[(&str, &str)] = &[
    ("FiraCode Nerd Font Mono", "FiraCode Nerd Font Mono"),
    ("Fira Code", "Fira Code"),
    ("Hack", "Hack"),
    ("(use UI font)", ""),
];

fn font_family_dropdown_items() -> Vec<DropdownItem<CortexSettingsAction>> {
    FONT_FAMILY_OPTIONS
        .iter()
        .map(|(label, value)| {
            DropdownItem::new(
                *label,
                CortexSettingsAction::SetFileExplorerFontName((*value).to_string()),
            )
        })
        .collect()
}

fn font_family_label_for_value(value: &str) -> &'static str {
    FONT_FAMILY_OPTIONS
        .iter()
        .find(|(_, v)| *v == value)
        .map(|(label, _)| *label)
        .unwrap_or(FONT_FAMILY_OPTIONS[0].0)
}

pub struct FileExplorerPageState {
    pub(crate) font_name_dropdown: ViewHandle<Dropdown<CortexSettingsAction>>,
    tree_lines_switch: SwitchStateHandle,
    nerd_icons_switch: SwitchStateHandle,
    colored_icons_switch: SwitchStateHandle,
}

impl FileExplorerPageState {
    pub fn new(ctx: &mut warpui::ViewContext<CortexSettingsView>) -> Self {
        let font_name_dropdown = ctx.add_typed_action_view(|ctx| {
            let mut dropdown = Dropdown::new(ctx);
            dropdown.set_top_bar_max_width(FONT_NAME_DROPDOWN_WIDTH);
            dropdown.set_menu_width(FONT_NAME_DROPDOWN_WIDTH, ctx);
            let items = font_family_dropdown_items();
            dropdown.add_items(items, ctx);
            let initial_name =
                (*CortexSettings::as_ref(ctx).file_explorer_font_name.value()).clone();
            dropdown.set_selected_by_name(font_family_label_for_value(&initial_name), ctx);
            dropdown
        });

        Self {
            font_name_dropdown,
            tree_lines_switch: SwitchStateHandle::default(),
            nerd_icons_switch: SwitchStateHandle::default(),
            colored_icons_switch: SwitchStateHandle::default(),
        }
    }
}

pub fn file_explorer_page_search_terms() -> &'static [&'static str] {
    &[
        "file",
        "explorer",
        "tree",
        "lines",
        "nerd",
        "font",
        "icons",
        "yazi",
        "tui",
        "colored",
    ]
}

pub fn render_file_explorer_page(
    state: &FileExplorerPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let nerd_enabled = *CortexSettings::as_ref(app).file_explorer_nerd_icons;

    Flex::column()
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(render_font_family_row(state, appearance))
        .with_child(render_tree_lines_row(state, appearance, app))
        .with_child(render_nerd_icons_row(state, appearance, app))
        .with_child(render_colored_icons_row(
            state,
            appearance,
            app,
            nerd_enabled,
        ))
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

fn render_switch_row(
    label_text: &str,
    description: Option<&str>,
    switch_handle: SwitchStateHandle,
    current_value: bool,
    enabled: bool,
    action: CortexSettingsAction,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();

    let mut label_col = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Start);

    label_col.add_child(ui_builder.span(label_text.to_string()).build().finish());

    if let Some(desc) = description {
        let theme = appearance.theme();
        let desc_color =
            crate::ui_components::blended_colors::text_sub(theme, theme.background());
        label_col.add_child(
            Container::new(
                warpui::elements::Text::new_inline(
                    desc.to_string(),
                    appearance.ui_font_family(),
                    appearance.ui_font_size() * 0.85,
                )
                .with_color(desc_color)
                .finish(),
            )
            .with_padding_top(2.0)
            .finish(),
        );
    }

    let mut switch_builder = ui_builder.switch(switch_handle).check(current_value);
    if !enabled {
        switch_builder = switch_builder.disable();
    }
    let switch = switch_builder
        .build()
        .on_click(move |ctx: &mut warpui::EventContext, _, _| {
            if enabled {
                ctx.dispatch_typed_action(action.clone());
            }
        })
        .finish();

    label_control_row(label_col.finish(), switch)
}

fn render_font_family_row(
    state: &FileExplorerPageState,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let label = appearance
        .ui_builder()
        .span("Font Family".to_string())
        .build()
        .finish();
    let dropdown = ConstrainedBox::new(ChildView::new(&state.font_name_dropdown).finish())
        .with_width(FONT_NAME_DROPDOWN_WIDTH)
        .finish();
    label_control_row(label, dropdown)
}

fn render_tree_lines_row(
    state: &FileExplorerPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let current_value = *CortexSettings::as_ref(app).file_explorer_tree_lines;
    render_switch_row(
        "Tree Lines",
        Some("Draw box-drawing characters connecting sibling entries."),
        state.tree_lines_switch.clone(),
        current_value,
        true,
        CortexSettingsAction::ToggleFileExplorerTreeLines,
        appearance,
    )
}

fn render_nerd_icons_row(
    state: &FileExplorerPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    let current_value = *CortexSettings::as_ref(app).file_explorer_nerd_icons;
    render_switch_row(
        "Nerd Font Icons",
        Some("Replace SVG file-type icons with Nerd Font glyphs."),
        state.nerd_icons_switch.clone(),
        current_value,
        true,
        CortexSettingsAction::ToggleFileExplorerNerdIcons,
        appearance,
    )
}

fn render_colored_icons_row(
    state: &FileExplorerPageState,
    appearance: &Appearance,
    app: &AppContext,
    enabled: bool,
) -> Box<dyn Element> {
    let current_value = *CortexSettings::as_ref(app).file_explorer_colored_icons;
    render_switch_row(
        "Colored Icons",
        Some("Color icons per file type (Rust=orange, Python=blue, etc.)."),
        state.colored_icons_switch.clone(),
        current_value,
        enabled,
        CortexSettingsAction::ToggleFileExplorerColoredIcons,
        appearance,
    )
}
