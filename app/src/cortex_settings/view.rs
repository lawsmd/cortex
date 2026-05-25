//! The view rendered inside a Cortex Settings pane: a left-side category list
//! and a right-side content area showing the currently selected page.
//!
//! Modeled on `app/src/server/network_log_view.rs` for the pane plumbing
//! (Entity / BackingView / pane header chrome). The layout is purpose-built
//! and intentionally minimal — sidebar of clickable category labels + content
//! area — rather than the heavier `SettingsView` framework Warp uses for its
//! own settings pane. Cortex’s setting count is small for now and a simpler
//! layout keeps this module independent of the `settings_view` crate (the
//! surface we want to keep clean for upstream merges).
use warp_core::ui::theme::Fill;
use warpui::{
    elements::{
        Align, Border, ChildView, Clipped, ConstrainedBox, Container, CrossAxisAlignment, Element,
        Flex, MainAxisAlignment, MainAxisSize, MouseStateHandle, ParentElement, Shrinkable, Text,
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
use crate::cortex_settings::ai_page::{ai_page_search_terms, render_ai_page, AiPageState};
use crate::cortex_settings::brand::{
    BRAND_HEADER_ICON_TO_TITLE_FONT_RATIO, BRAND_HEADER_TITLE_TO_FONT_RATIO,
    BRAND_MENU_ICON_LABEL_GAP_RATIO,
};
use crate::cortex_settings::editor_page::{
    editor_page_search_terms, render_editor_page, EditorPageState,
};
use crate::cortex_settings::tabs_page::{render_tabs_page, tabs_page_search_terms, TabsPageState};
use crate::cortex_settings::toolbar_page::{
    render_toolbar_page, toolbar_page_search_terms, ToolbarPageState,
};
use crate::cortex_settings::top_bar_page::{
    render_top_bar_page, top_bar_page_search_terms, TopBarPageState,
};
use crate::cortex_settings::working_panes_page::{
    render_working_panes_page, working_panes_page_search_terms, WorkingPanesPageState,
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

// Match the Warp Settings sidebar geometry (no settings-file footer variant)
// so flipping between a Cortex Settings tab and a Warp Settings tab lands on
// pixel-identical chrome. Constants mirrored from
// `app/src/settings_view/settings_page.rs` and `settings_view/mod.rs`.
const SIDEBAR_WIDTH: f32 = 200.0;
const SIDEBAR_ITEM_PADDING: f32 = 8.0;
const SIDEBAR_ITEM_LEFT_MARGIN: f32 = 12.0;
const SEARCH_BAR_HORIZONTAL_MARGIN: f32 = 16.0;
const SEARCH_BAR_BOTTOM_MARGIN: f32 = 8.0;
const SEARCH_ICON_SIZE: f32 = 16.0;
const SEARCH_ICON_RIGHT_GAP: f32 = 12.0;
const SIDEBAR_HEADER_PADDING: f32 = 15.0;
const SIDEBAR_BORDER_WIDTH: f32 = 1.0;
const PAGE_PADDING: f32 = 28.0;
const MAX_PAGE_WIDTH: f32 = 800.0;

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
    working_panes_state: WorkingPanesPageState,
    tabs_state: TabsPageState,
    top_bar_state: TopBarPageState,
    toolbar_state: ToolbarPageState,
    editor_state: EditorPageState,
    ai_state: AiPageState,
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
            working_panes_state: WorkingPanesPageState::default(),
            tabs_state: TabsPageState::new(ctx),
            top_bar_state: TopBarPageState::default(),
            toolbar_state: ToolbarPageState::default(),
            editor_state: EditorPageState::default(),
            ai_state: AiPageState::default(),
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
            // surfaces persistence errors via its own banner, and there’s
            // nothing useful to do at this call site beyond re-rendering with
            // the new state on the next frame.
            let _ = settings.hide_pane_separators.toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn toggle_start_with_blank_pane_on_launch(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .start_with_blank_pane_on_launch
                .toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn toggle_recap_matches_terminal_style(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .recap_matches_terminal_style
                .toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn toggle_tabs_panel_matches_terminal_bg(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .tabs_panel_matches_terminal_bg
                .toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn toggle_tabs_hide_icon_backdrop(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.tabs_hide_icon_backdrop.toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn toggle_stack_left_column(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.stack_left_column.toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn set_tab_style(
        &mut self,
        value: crate::settings::TabStyle,
        ctx: &mut ViewContext<Self>,
    ) {
        use crate::settings::CortexSettings;
        use settings::Setting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.tab_style.set_value(value, ctx);
        });
        ctx.notify();
    }

    fn set_tabs_title_alignment(
        &mut self,
        value: crate::settings::TabsTitleAlignment,
        ctx: &mut ViewContext<Self>,
    ) {
        use crate::settings::CortexSettings;
        use settings::Setting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.tabs_title_alignment.set_value(value, ctx);
        });
        ctx.notify();
    }

    fn set_tabs_metadata_alignment(
        &mut self,
        value: crate::settings::TabsMetadataAlignment,
        ctx: &mut ViewContext<Self>,
    ) {
        use crate::settings::CortexSettings;
        use settings::Setting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.tabs_metadata_alignment.set_value(value, ctx);
        });
        ctx.notify();
    }

    fn set_tab_title_font_name(&mut self, value: String, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::Setting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.tabs_title_font_name.set_value(value, ctx);
        });
        ctx.notify();
    }

    fn set_tab_title_font_size(&mut self, value: f32, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::Setting;
        use warpui::SingletonEntity;

        // Clamp at the write boundary as well as the consumption site —
        // belt-and-suspenders against typo’d hand-edits in `user_preferences.toml`.
        let value = value.clamp(8.0, 32.0);
        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.tabs_title_font_size.set_value(value, ctx);
        });
        ctx.notify();
    }

    fn set_tab_title_font_weight(
        &mut self,
        value: warpui::fonts::Weight,
        ctx: &mut ViewContext<Self>,
    ) {
        use crate::settings::CortexSettings;
        use settings::Setting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.tabs_title_font_weight.set_value(value, ctx);
        });
        ctx.notify();
    }

    fn toggle_tab_title_italic(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.tabs_title_italic.toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn toggle_allow_local_claude_codex_child_harnesses(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warp_core::features::FeatureFlag;
        use warpui::SingletonEntity;

        let previous_value = *CortexSettings::as_ref(ctx).allow_local_claude_codex_child_harnesses;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .allow_local_claude_codex_child_harnesses
                .toggle_and_save_value(ctx);
        });

        FeatureFlag::LocalClaudeCodexChildHarnesses.set_user_preference(!previous_value);

        ctx.notify();
    }

    fn toggle_editor_wrap_long_lines(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.editor_wrap_long_lines.toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn toggle_top_bar_matches_terminal_bg(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .top_bar_matches_terminal_bg
                .toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn toggle_top_bar_hide_divider(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.top_bar_hide_divider.toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn set_top_bar_search_bar_style(
        &mut self,
        value: crate::settings::SearchBarStyle,
        ctx: &mut ViewContext<Self>,
    ) {
        use crate::settings::CortexSettings;
        use settings::Setting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.top_bar_search_bar_style.set_value(value, ctx);
        });
        ctx.notify();
    }

    fn toggle_toolbar_show_file_explorer(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .toolbar_show_file_explorer
                .toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn toggle_toolbar_show_global_search(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .toolbar_show_global_search
                .toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn toggle_toolbar_show_warp_drive(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .toolbar_show_warp_drive
                .toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn toggle_toolbar_show_agent_conversations(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .toolbar_show_agent_conversations
                .toggle_and_save_value(ctx);
        });
        ctx.notify();
    }

    fn handle_search_editor_event(
        &mut self,
        _editor: ViewHandle<EditorView>,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
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
                    .with_margin_right(SEARCH_ICON_RIGHT_GAP)
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
        .with_margin_bottom(SEARCH_BAR_BOTTOM_MARGIN)
        .finish()
    }

    fn render_sidebar(&self, appearance: &Appearance, app: &AppContext) -> Box<dyn Element> {
        let query = self.search_editor.as_ref(app).buffer_text(app);
        let query_lower = query.to_lowercase();
        let query_trimmed = query_lower.trim();

        let mut nav_column = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);

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
                        .set_margin(Coords::default().left(SIDEBAR_ITEM_LEFT_MARGIN))
                        .set_padding(Coords::uniform(SIDEBAR_ITEM_PADDING)),
                )
                .build();

            let row = hoverable
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(CortexSettingsAction::SelectSection(section));
                })
                .finish();

            nav_column = nav_column.with_child(row);
        }

        let column = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(self.render_search_bar(appearance))
            .with_child(
                Container::new(nav_column.finish())
                    .with_padding_top(SIDEBAR_HEADER_PADDING)
                    .finish(),
            )
            .finish();

        let theme = appearance.theme();
        ConstrainedBox::new(
            Container::new(column)
                .with_border(
                    Border::right(SIDEBAR_BORDER_WIDTH).with_border_fill(theme.outline()),
                )
                .finish(),
        )
        .with_width(SIDEBAR_WIDTH)
        .finish()
    }

    fn render_content(&self, appearance: &Appearance, app: &AppContext) -> Box<dyn Element> {
        match self.current_section {
            CortexSettingsSection::WorkingPanes => {
                render_working_panes_page(&self.working_panes_state, appearance, app)
            }
            CortexSettingsSection::Tabs => render_tabs_page(&self.tabs_state, appearance, app),
            CortexSettingsSection::TopBar => {
                render_top_bar_page(&self.top_bar_state, appearance, app)
            }
            CortexSettingsSection::Toolbar => {
                render_toolbar_page(&self.toolbar_state, appearance, app)
            }
            CortexSettingsSection::Editor => {
                render_editor_page(&self.editor_state, appearance, app)
            }
            CortexSettingsSection::Ai => render_ai_page(&self.ai_state, appearance, app),
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

        let centered_content = Container::new(
            Align::new(
                ConstrainedBox::new(self.render_content(appearance, app))
                    .with_max_width(MAX_PAGE_WIDTH)
                    .finish(),
            )
            .top_center()
            .finish(),
        )
        .with_uniform_padding(PAGE_PADDING)
        .finish();

        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_main_axis_alignment(MainAxisAlignment::Start)
            .with_main_axis_size(MainAxisSize::Max)
            .with_child(self.render_sidebar(appearance, app))
            .with_child(Shrinkable::new(1., centered_content).finish())
            .finish()
    }
}

impl warpui::TypedActionView for CortexSettingsView {
    type Action = CortexSettingsAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            CortexSettingsAction::SelectSection(section) => self.select_section(*section, ctx),
            CortexSettingsAction::ToggleHidePaneSeparators => self.toggle_hide_pane_separators(ctx),
            CortexSettingsAction::ToggleStartWithBlankPaneOnLaunch => {
                self.toggle_start_with_blank_pane_on_launch(ctx)
            }
            CortexSettingsAction::ToggleRecapMatchesTerminalStyle => {
                self.toggle_recap_matches_terminal_style(ctx)
            }
            CortexSettingsAction::ToggleTabsPanelMatchesTerminalBg => {
                self.toggle_tabs_panel_matches_terminal_bg(ctx)
            }
            CortexSettingsAction::ToggleTabsHideIconBackdrop => {
                self.toggle_tabs_hide_icon_backdrop(ctx)
            }
            CortexSettingsAction::SetTabStyle(value) => self.set_tab_style(*value, ctx),
            CortexSettingsAction::SetTabsTitleAlignment(value) => {
                self.set_tabs_title_alignment(*value, ctx)
            }
            CortexSettingsAction::SetTabsMetadataAlignment(value) => {
                self.set_tabs_metadata_alignment(*value, ctx)
            }
            CortexSettingsAction::ToggleStackLeftColumn => self.toggle_stack_left_column(ctx),
            CortexSettingsAction::SetTabTitleFontName(value) => {
                self.set_tab_title_font_name(value.clone(), ctx)
            }
            CortexSettingsAction::SetTabTitleFontSize(value) => {
                self.set_tab_title_font_size(*value, ctx)
            }
            CortexSettingsAction::SetTabTitleFontWeight(value) => {
                self.set_tab_title_font_weight(*value, ctx)
            }
            CortexSettingsAction::ToggleTabTitleItalic => self.toggle_tab_title_italic(ctx),
            CortexSettingsAction::ToggleAllowLocalClaudeCodexChildHarnesses => {
                self.toggle_allow_local_claude_codex_child_harnesses(ctx)
            }
            CortexSettingsAction::ToggleEditorWrapLongLines => {
                self.toggle_editor_wrap_long_lines(ctx)
            }
            CortexSettingsAction::ToggleTopBarMatchesTerminalBg => {
                self.toggle_top_bar_matches_terminal_bg(ctx)
            }
            CortexSettingsAction::ToggleTopBarHideDivider => {
                self.toggle_top_bar_hide_divider(ctx)
            }
            CortexSettingsAction::SetTopBarSearchBarStyle(value) => {
                self.set_top_bar_search_bar_style(*value, ctx)
            }
            CortexSettingsAction::ToggleToolbarShowFileExplorer => {
                self.toggle_toolbar_show_file_explorer(ctx)
            }
            CortexSettingsAction::ToggleToolbarShowGlobalSearch => {
                self.toggle_toolbar_show_global_search(ctx)
            }
            CortexSettingsAction::ToggleToolbarShowWarpDrive => {
                self.toggle_toolbar_show_warp_drive(ctx)
            }
            CortexSettingsAction::ToggleToolbarShowAgentConversations => {
                self.toggle_toolbar_show_agent_conversations(ctx)
            }
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

        let base_font = appearance.ui_font_size();
        let title_font = base_font * BRAND_HEADER_TITLE_TO_FONT_RATIO;
        let icon_size = title_font * BRAND_HEADER_ICON_TO_TITLE_FONT_RATIO;
        let icon_label_gap = icon_size * BRAND_MENU_ICON_LABEL_GAP_RATIO;
        let accent_color: Fill = theme.accent().into();

        let brain = ConstrainedBox::new(
            Container::new(Icon::Brain.to_warpui_icon(accent_color).finish())
                .with_margin_right(icon_label_gap)
                .finish(),
        )
        .with_width(icon_size)
        .with_height(icon_size)
        .finish();

        let title_text = Text::new_inline(
            CORTEX_SETTINGS_HEADER_TEXT.to_string(),
            appearance.ui_font_family(),
            title_font,
        )
        .with_color(accent_color.into())
        .finish();

        let header_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(brain)
            .with_child(title_text)
            .finish();

        HeaderContent::Standard(StandardHeader {
            title: String::new(),
            title_secondary: None,
            title_style: None,
            title_color: None,
            title_clip_config: ClipConfig::start(),
            title_max_width: None,
            left_of_title: Some(header_row),
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

#[allow(dead_code)]
pub fn cortex_settings_search_terms() -> String {
    format!(
        "cortex settings {} {} {} {} {} {}",
        working_panes_page_search_terms().join(" "),
        tabs_page_search_terms().join(" "),
        top_bar_page_search_terms().join(" "),
        toolbar_page_search_terms().join(" "),
        editor_page_search_terms().join(" "),
        ai_page_search_terms().join(" ")
    )
}
