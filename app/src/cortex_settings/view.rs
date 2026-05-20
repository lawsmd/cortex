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
use crate::cortex_settings::tabs_page::{render_tabs_page, tabs_page_search_terms, TabsPageState};
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
            // surfaces persistence errors via its own banner, and there's
            // nothing useful to do at this call site beyond re-rendering with
            // the new state on the next frame.
            let _ = settings.hide_pane_separators.toggle_and_save_value(ctx);
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

    fn toggle_tabs_inverse_fill_on_selection(&mut self, ctx: &mut ViewContext<Self>) {
        use crate::settings::CortexSettings;
        use settings::ToggleableSetting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .tabs_inverse_fill_on_selection
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

    fn set_tabs_selected_title_alignment(
        &mut self,
        value: crate::settings::TabsSelectedTitleAlignment,
        ctx: &mut ViewContext<Self>,
    ) {
        use crate::settings::CortexSettings;
        use settings::Setting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings.tabs_selected_title_alignment.set_value(value, ctx);
        });
        ctx.notify();
    }

    fn set_tabs_selected_metadata_alignment(
        &mut self,
        value: crate::settings::TabsSelectedMetadataAlignment,
        ctx: &mut ViewContext<Self>,
    ) {
        use crate::settings::CortexSettings;
        use settings::Setting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .tabs_selected_metadata_alignment
                .set_value(value, ctx);
        });
        ctx.notify();
    }

    fn set_tabs_unselected_title_alignment(
        &mut self,
        value: crate::settings::TabsUnselectedTitleAlignment,
        ctx: &mut ViewContext<Self>,
    ) {
        use crate::settings::CortexSettings;
        use settings::Setting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .tabs_unselected_title_alignment
                .set_value(value, ctx);
        });
        ctx.notify();
    }

    fn set_tabs_unselected_metadata_alignment(
        &mut self,
        value: crate::settings::TabsUnselectedMetadataAlignment,
        ctx: &mut ViewContext<Self>,
    ) {
        use crate::settings::CortexSettings;
        use settings::Setting;
        use warpui::SingletonEntity;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .tabs_unselected_metadata_alignment
                .set_value(value, ctx);
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
        // belt-and-suspenders against typo'd hand-edits in `user_preferences.toml`.
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

        // Read pre-toggle value so we can compute the post-toggle value and push
        // it into the feature flag without re-reading after the closure (which
        // would race with rendering threads that have already snapshotted).
        let previous_value = *CortexSettings::as_ref(ctx).allow_local_claude_codex_child_harnesses;

        CortexSettings::handle(ctx).update(ctx, |settings, ctx| {
            let _ = settings
                .allow_local_claude_codex_child_harnesses
                .toggle_and_save_value(ctx);
        });

        // Mirror the setting into the runtime feature flag so the existing
        // gate sites (`local_child_harnesses.rs`, `orchestration_controls.rs`)
        // pick up the change on the next `is_enabled()` call without needing
        // to be ported to read the setting directly.
        FeatureFlag::LocalClaudeCodexChildHarnesses.set_user_preference(!previous_value);

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

        // Search bar sits flush with the top, then 15 px of breathing room
        // before the first nav item — matches Warp's `HEADER_PADDING`-padded
        // scroll wrapper around the nav list.
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

        // Mirror Warp Settings: page content is centered in a max-800px column
        // with `Align::top_center`, wrapped in 28 px page padding. Pages
        // themselves continue to use the full inner width via
        // `CrossAxisAlignment::Stretch`.
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
            CortexSettingsAction::ToggleTabsPanelMatchesTerminalBg => {
                self.toggle_tabs_panel_matches_terminal_bg(ctx)
            }
            CortexSettingsAction::ToggleTabsInverseFillOnSelection => {
                self.toggle_tabs_inverse_fill_on_selection(ctx)
            }
            CortexSettingsAction::ToggleTabsHideIconBackdrop => {
                self.toggle_tabs_hide_icon_backdrop(ctx)
            }
            CortexSettingsAction::SetTabsSelectedTitleAlignment(value) => {
                self.set_tabs_selected_title_alignment(*value, ctx)
            }
            CortexSettingsAction::SetTabsSelectedMetadataAlignment(value) => {
                self.set_tabs_selected_metadata_alignment(*value, ctx)
            }
            CortexSettingsAction::SetTabsUnselectedTitleAlignment(value) => {
                self.set_tabs_unselected_title_alignment(*value, ctx)
            }
            CortexSettingsAction::SetTabsUnselectedMetadataAlignment(value) => {
                self.set_tabs_unselected_metadata_alignment(*value, ctx)
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

        // The Cortex Settings header reads as a section heading, so the title
        // is bumped above the standard pane-title font size and the brain
        // glyph above the avatar-menu's icon-to-text ratio — both via the
        // header-specific brand ratios in `brand.rs`. Glyph and title share
        // `theme.accent()` so the brand mark renders as one tinted unit. The
        // icon shader's red-channel-as-alpha rule means the SVG itself just
        // needs non-zero red pixels (white-stroked); the call-site color
        // wins. See docs/branding.md. Never the U+1F9E0 emoji.
        //
        // The framework's `StandardHeader` hard-codes the title text size to
        // `ui_font_size()` (see `app/src/pane_group/pane/view/header/mod.rs`)
        // and `Properties` carries weight/style only — no font-size lever.
        // Rather than fork that upstream file, we render glyph + title as a
        // single flex row passed via `left_of_title` and leave the framework's
        // `title` slot empty.
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

/// Search terms for surfacing the Cortex Settings pane in command palettes —
/// not consumed yet, but kept here so a future palette integration has a
/// single point to draw from.
#[allow(dead_code)]
pub fn cortex_settings_search_terms() -> String {
    format!(
        "cortex settings {} {} {}",
        working_panes_page_search_terms().join(" "),
        tabs_page_search_terms().join(" "),
        ai_page_search_terms().join(" ")
    )
}
