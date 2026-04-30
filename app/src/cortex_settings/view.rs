//! The view rendered inside a Cortex Settings pane: a left-side category list
//! and a right-side content area showing the currently selected page.
//!
//! Modeled on `app/src/server/network_log_view.rs` for the pane plumbing
//! (Entity / BackingView / pane header chrome). The layout is purpose-built
//! and intentionally minimal — sidebar of clickable category labels + content
//! area — rather than the heavier `SettingsView` framework Warp uses for its
//! own settings pane. Cortex's setting count is small for now and a simpler
//! layout keeps this module independent of the `settings_view` crate (the
//! surface we want to keep clean for upstream merges).
use warp_core::ui::theme::Fill;
use warpui::{
    elements::{
        ChildView, Clipped, ConstrainedBox, Container, CrossAxisAlignment, Element, Flex,
        MainAxisAlignment, MouseStateHandle, Padding, ParentElement, Rect, Shrinkable,
    },
    text_layout::ClipConfig,
    ui_components::{
        button::ButtonVariant,
        components::{Coords, UiComponent, UiComponentStyles},
    },
    AppContext, Entity, ModelHandle, SingletonEntity, View, ViewContext, ViewHandle,
};

use crate::appearance::Appearance;
use crate::cortex_settings::action::{CortexSettingsAction, CortexSettingsSection};
use crate::cortex_settings::appearance_page::{
    appearance_page_search_terms, render_appearance_page, AppearancePageState,
};
use crate::editor::{
    EditorView, Event as EditorEvent, PropagateAndNoOpNavigationKeys, SingleLineEditorOptions,
    TextOptions,
};
use crate::pane_group::focus_state::PaneFocusHandle;
use crate::pane_group::pane::view::{
    self, HeaderContent, StandardHeader, StandardHeaderOptions,
};
use crate::pane_group::{BackingView, PaneConfiguration, PaneEvent};
use crate::ui_components::icons::Icon;

pub const CORTEX_SETTINGS_HEADER_TEXT: &str = "Cortex Settings";

const SIDEBAR_WIDTH: f32 = 220.0;
const SIDEBAR_ITEM_PADDING: f32 = 8.0;
const SIDEBAR_ITEM_HORIZONTAL_MARGIN: f32 = 6.0;
const SEARCH_BAR_VERTICAL_MARGIN: f32 = 6.0;
const SEARCH_BAR_HORIZONTAL_MARGIN: f32 = 8.0;
const SEARCH_ICON_SIZE: f32 = 14.0;
const HEADER_BRAIN_ICON_SIZE: f32 = 16.0;
const HEADER_BRAIN_ICON_RIGHT_MARGIN: f32 = 6.0;
const CONTENT_AREA_PADDING: f32 = 20.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CortexSettingsViewEvent {
    Pane(PaneEvent),
}

/// Empty overflow-menu action type — Cortex Settings has no overflow menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CortexSettingsViewOverflowAction {}

/// Empty custom-action type — Cortex Settings header has no custom buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CortexSettingsViewCustomAction {}

pub struct CortexSettingsView {
    pane_configuration: ModelHandle<PaneConfiguration>,
    focus_handle: Option<PaneFocusHandle>,
    current_section: CortexSettingsSection,
    sidebar_states: Vec<(CortexSettingsSection, MouseStateHandle)>,
    appearance_state: AppearancePageState,
    search_editor: ViewHandle<EditorView>,
}

impl CortexSettingsView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let pane_configuration =
            ctx.add_model(|_ctx| PaneConfiguration::new(CORTEX_SETTINGS_HEADER_TEXT));

        let sidebar_states = CortexSettingsSection::all()
            .iter()
            .map(|section| (*section, MouseStateHandle::default()))
            .collect();

        let font_family = Appearance::as_ref(ctx).ui_font_family();
        let search_editor = ctx.add_typed_action_view(|ctx| {
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
            editor.set_placeholder_text("Search", ctx);
            editor
        });

        ctx.subscribe_to_view(&search_editor, Self::handle_search_editor_event);

        Self {
            pane_configuration,
            focus_handle: None,
            current_section: CortexSettingsSection::default(),
            sidebar_states,
            appearance_state: AppearancePageState::default(),
            search_editor,
        }
    }

    pub fn pane_configuration(&self) -> ModelHandle<PaneConfiguration> {
        self.pane_configuration.clone()
    }

    pub fn focus(&mut self, _ctx: &mut ViewContext<Self>) {
        // No interactive child to focus — section selection happens via clicks.
    }

    fn select_section(&mut self, section: CortexSettingsSection, ctx: &mut ViewContext<Self>) {
        if self.current_section != section {
            self.current_section = section;
            ctx.notify();
        }
    }

    fn toggle_hide_pane_separators(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            // `toggle_and_save_value` is provided by the `ToggleableSetting`
            // blanket impl for any boolean `Setting`. It flips the value,
            // persists it through the settings system, and returns the new
            // value (or an error). We discard the result — the settings system
            // surfaces persistence errors via its own banner, and there's
            // nothing useful to do at this call site beyond re-rendering with
            // the new state on the next frame.
            let _ = settings.hide_pane_separators.toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn handle_search_editor_event(
        &mut self,
        _editor: ViewHandle<EditorView>,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        // Re-render whenever the search text changes so the sidebar filter
        // reflects the latest query. We don't need to keep a copy of the
        // query — `render_sidebar` reads it on demand from the editor.
        if matches!(event, EditorEvent::Edited(_)) {
            ctx.notify();
        }
    }

    fn render_search_bar(&self, appearance: &Appearance) -> Box<dyn Element> {
        Container::new(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    Container::new(
                        ConstrainedBox::new(
                            Icon::SearchSmall
                                .to_warpui_icon(appearance.theme().active_ui_text_color())
                                .finish(),
                        )
                        .with_width(SEARCH_ICON_SIZE)
                        .with_height(SEARCH_ICON_SIZE)
                        .finish(),
                    )
                    .with_uniform_margin(4.0)
                    .with_margin_right(8.0)
                    .finish(),
                )
                .with_child(
                    Shrinkable::new(
                        1.,
                        Clipped::new(ChildView::new(&self.search_editor).finish()).finish(),
                    )
                    .finish(),
                )
                .finish(),
        )
        .with_margin_left(SEARCH_BAR_HORIZONTAL_MARGIN)
        .with_margin_right(SEARCH_BAR_HORIZONTAL_MARGIN)
        .with_margin_top(SEARCH_BAR_VERTICAL_MARGIN)
        .with_margin_bottom(SEARCH_BAR_VERTICAL_MARGIN)
        .finish()
    }

    fn render_sidebar(&self, appearance: &Appearance, app: &AppContext) -> Box<dyn Element> {
        let query = self.search_editor.as_ref(app).buffer_text(app);
        let query_lower = query.to_lowercase();
        let query_trimmed = query_lower.trim();

        let mut column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        column = column.with_child(self.render_search_bar(appearance));

        for (section, mouse_state) in &self.sidebar_states {
            let label = section.label();
            if !query_trimmed.is_empty() && !label.to_lowercase().contains(query_trimmed) {
                continue;
            }
            let section = *section;
            let is_active = section == self.current_section;
            let hoverable = appearance
                .ui_builder()
                .button(
                    if is_active {
                        ButtonVariant::Accent
                    } else {
                        ButtonVariant::Text
                    },
                    mouse_state.clone(),
                )
                .with_text_label(label.to_string())
                .with_style(
                    UiComponentStyles::default()
                        .set_border_width(0.)
                        .set_margin(
                            Coords::default()
                                .left(SIDEBAR_ITEM_HORIZONTAL_MARGIN)
                                .right(SIDEBAR_ITEM_HORIZONTAL_MARGIN),
                        )
                        .set_padding(Coords::uniform(SIDEBAR_ITEM_PADDING)),
                )
                .build();

            let row = hoverable
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(CortexSettingsAction::SelectSection(section));
                })
                .finish();

            column = column.with_child(row);
        }

        ConstrainedBox::new(column.finish())
            .with_width(SIDEBAR_WIDTH)
            .finish()
    }

    fn render_divider(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        ConstrainedBox::new(
            Rect::new()
                .with_background_color(theme.outline().into_solid())
                .finish(),
        )
        .with_width(1.0)
        .finish()
    }

    fn render_content(&self, appearance: &Appearance, app: &AppContext) -> Box<dyn Element> {
        match self.current_section {
            CortexSettingsSection::Appearance => {
                render_appearance_page(&self.appearance_state, appearance, app)
            }
        }
    }
}

impl Entity for CortexSettingsView {
    type Event = CortexSettingsViewEvent;
}

impl View for CortexSettingsView {
    fn ui_name() -> &'static str {
        "CortexSettingsView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_alignment(MainAxisAlignment::Start)
            .with_child(self.render_sidebar(appearance, app))
            .with_child(self.render_divider(appearance))
            .with_child(
                Container::new(self.render_content(appearance, app))
                    .with_padding(Padding::uniform(CONTENT_AREA_PADDING))
                    .finish(),
            )
            .finish()
    }
}

impl warpui::TypedActionView for CortexSettingsView {
    type Action = CortexSettingsAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            CortexSettingsAction::SelectSection(section) => self.select_section(*section, ctx),
            CortexSettingsAction::ToggleHidePaneSeparators => self.toggle_hide_pane_separators(ctx),
        }
    }
}

impl BackingView for CortexSettingsView {
    type PaneHeaderOverflowMenuAction = CortexSettingsViewOverflowAction;
    type CustomAction = CortexSettingsViewCustomAction;
    type AssociatedData = ();

    fn handle_pane_header_overflow_menu_action(
        &mut self,
        _action: &Self::PaneHeaderOverflowMenuAction,
        _ctx: &mut ViewContext<Self>,
    ) {
        // Uninhabited — Cortex Settings has no overflow menu items.
    }

    fn close(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.emit(CortexSettingsViewEvent::Pane(PaneEvent::Close));
    }

    fn focus_contents(&mut self, ctx: &mut ViewContext<Self>) {
        self.focus(ctx);
    }

    fn render_header_content(
        &self,
        _ctx: &view::HeaderRenderContext<'_>,
        app: &AppContext,
    ) -> HeaderContent {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();

        // Brain glyph tinted to `theme.foreground()`. The icon shader's
        // red-channel-as-alpha rule means the SVG itself just needs non-zero
        // red pixels (white-stroked); the call-site color wins. See
        // docs/branding.md. Never the U+1F9E0 emoji.
        let brain_color: Fill = theme.foreground().into();
        let brain = ConstrainedBox::new(
            Container::new(Icon::Brain.to_warpui_icon(brain_color).finish())
                .with_margin_right(HEADER_BRAIN_ICON_RIGHT_MARGIN)
                .finish(),
        )
        .with_width(HEADER_BRAIN_ICON_SIZE)
        .with_height(HEADER_BRAIN_ICON_SIZE)
        .finish();

        // Render the title in the theme's accent color so the Cortex Settings
        // header visually distinguishes itself from regular pane headers
        // (which use the default sub-text color).
        let title_color: Fill = theme.accent().into();

        HeaderContent::Standard(StandardHeader {
            title: CORTEX_SETTINGS_HEADER_TEXT.to_string(),
            title_secondary: None,
            title_style: None,
            title_color: Some(title_color),
            title_clip_config: ClipConfig::start(),
            title_max_width: None,
            left_of_title: Some(brain),
            right_of_title: None,
            left_of_overflow: None,
            options: StandardHeaderOptions {
                always_show_icons: true,
                ..StandardHeaderOptions::default()
            },
        })
    }

    fn set_focus_handle(&mut self, focus_handle: PaneFocusHandle, _ctx: &mut ViewContext<Self>) {
        self.focus_handle = Some(focus_handle);
    }
}

/// Search terms for surfacing the Cortex Settings pane in command palettes —
/// not consumed yet, but kept here so a future palette integration has a
/// single point to draw from.
#[allow(dead_code)]
pub fn cortex_settings_search_terms() -> String {
    format!(
        "cortex settings {}",
        appearance_page_search_terms().join(" ")
    )
}
