use std::collections::HashSet;

use fuzzy_match::match_indices_case_insensitive;
use pathfinder_color::ColorU;
use settings::Setting as _;
use warp_editor::editor::NavigationKey;
use warpui::{
    accessibility::{AccessibilityContent, WarpA11yRole},
    elements::{
        Align, ChildAnchor, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
        DispatchEventResult, Element, Empty, EventHandler, Fill, Flex, Hoverable, Icon,
        MainAxisAlignment, MainAxisSize, MouseStateHandle, OffsetPositioning, ParentAnchor,
        ParentElement, ParentOffsetBounds, Radius, Rect, SavePosition, ScrollStateHandle,
        Scrollable, ScrollableElement, ScrollbarWidth, Shrinkable, Stack, Text, UniformList,
        UniformListState,
    },
    fonts::{FamilyId, Weight},
    geometry::vector::vec2f,
    keymap::FixedBinding,
    platform::{Cursor, SystemTheme},
    ui_components::components::{UiComponent, UiComponentStyles},
    windowing::{StateEvent, WindowManager},
    AppContext, Entity, FocusContext, ModelHandle, SingletonEntity, Tracked, TypedActionView,
    UpdateModel, View, ViewContext, ViewHandle,
};

use crate::resource_center::{mark_feature_used_and_write_to_user_defaults, Tip, TipAction};
use crate::themes::theme::{RespectSystemTheme, ThemeKind, WarpTheme};
use crate::util::traffic_lights::traffic_light_data;
use crate::workspace::PANEL_HEADER_HEIGHT;
use crate::{
    appearance::Appearance,
    editor::{
        Event as EditorEvent, PropagateAndNoOpNavigationKeys, SingleLineEditorOptions, TextOptions,
    },
    referral_theme_status::ReferralThemeStatus,
    report_if_error,
    settings::{respect_system_theme, ThemeSettings},
    themes::theme::SelectedSystemThemes,
    user_config::{load_theme_configs, themes_dir, WarpConfig, WarpConfigUpdateEvent},
    util::traffic_lights::{TrafficLightData, TrafficLightSide},
    window_settings::WindowSettings,
};
use crate::{appearance::AppearanceManager, send_telemetry_from_ctx};
use crate::{editor::EditorView, resource_center::TipsCompleted};
use crate::{
    server::telemetry::TelemetryEvent, ui_components::window_focus_dimming::WindowFocusDimming,
};
use crate::{
    themes::theme::WarpThemeConfig,
    ui_components::buttons::{close_button, icon_button},
    ui_components::icons,
};


// All units in px
const THEME_CHOOSER_TITLE: &str = "Themes";
const CLOSE_BUTTON_MARGIN_RIGHT: f32 = 6.;
const TITLE_FONT_SIZE: f32 = 16.;
const TITLE_MARGIN: f32 = 12.;
const SCROLLBAR_WIDTH: ScrollbarWidth = ScrollbarWidth::Auto;
const THEME_NAME_FONT_SIZE: f32 = 13.;
const THEME_NAME_MARGIN_LEFT: f32 = 12.;
const DELETE_BUTTON_LINE_WIDTH: f32 = 10.;
const DELETE_BUTTON_LINE_HEIGHT: f32 = 1.33;
const DELETE_BUTTON_SIZE: f32 = 16.;
const DELETE_BUTTON_MARGIN_RIGHT: f32 = 16.;
const THEME_CHOOSER_ITEM_PADDING: f32 = 8.;

// Swatch strip — small palette preview rendered to the left of each theme
// name. Cheap to paint (16 colored rects, no text, no font atlas warm-up)
// so the picker can open instantly even on a cold GPU.
const SWATCH_WIDTH: f32 = 8.;
const SWATCH_HEIGHT: f32 = 14.;
const SWATCH_STRIP_MARGIN_LEFT: f32 = 14.;

// Section-header / hint rows in the layered default view.
const SECTION_HEADER_FONT_SIZE: f32 = 11.;
const SECTION_HEADER_MARGIN_TOP: f32 = 8.;
const SECTION_HEADER_MARGIN_BOTTOM: f32 = 2.;
const HINT_ROW_FONT_SIZE: f32 = 12.;
const HINT_ROW_PADDING: f32 = 4.;
// How many recently-selected themes the "Recents" section surfaces.
const MAX_RECENT_THEMES: usize = 5;

// Star toggle (Favorites). Rendered on every row except those that live
// inside the Favorites section itself (where the section header already
// communicates the favorited status — a per-row star there is just noise,
// especially when the same theme also shows up starred in Recents).
// Anchored to the right edge of the panel just inside the scrollbar.
// Hollow when not favorited (dim by default, brighter on hover) and
// filled when favorited.
const FAVORITE_STAR_SIZE: f32 = 14.;
const FAVORITE_STAR_MARGIN_RIGHT: f32 = 12.;
const FAVORITE_STAR_DIM_OPACITY: f32 = 0.35;
const FAVORITE_STAR_HOVER_OPACITY: f32 = 0.85;
const FAVORITE_STAR_OUTLINE_SVG: &str = "bundled/svg/star-outline.svg";
const FAVORITE_STAR_FILLED_SVG: &str = "bundled/svg/star-filled.svg";
// Star glyph rendered next to the "Favorites" section header so the
// section title carries its own visual identity.
const FAVORITES_HEADER_LABEL: &str = "Favorites";
const FAVORITES_HEADER_STAR_SIZE: f32 = 11.;
const FAVORITES_HEADER_STAR_GAP: f32 = 5.;

#[derive(Default)]
struct MouseStateHandles {
    create_theme_button_hover_state: MouseStateHandle,
    close_button_mouse_state: MouseStateHandle,
}

pub enum ThemeChooserEvent {
    Click,
    Close(ThemeChooserMode),
    OpenThemeCreatorModal,
    OpenThemeDeletionModal(ThemeKind),
}

#[derive(Clone, Copy, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ThemeChooserMode {
    /// Select a single fixed theme, independent of whether the system is using
    /// a light or dark theme.
    SystemAgnostic,
    /// Select a theme to use when the system is using a light theme.
    SystemLight,
    /// Select a theme to use when the system is using a dark theme.
    SystemDark,
}

impl ThemeChooserMode {
    /// Returns the mode the theme chooser should use if the aim is to change
    /// the active theme, as opposed to changing a specific theme option.
    pub fn for_active_theme(app: &AppContext) -> Self {
        match respect_system_theme(ThemeSettings::as_ref(app)) {
            RespectSystemTheme::On(_) => match app.system_theme() {
                SystemTheme::Dark => ThemeChooserMode::SystemDark,
                SystemTheme::Light => ThemeChooserMode::SystemLight,
            },
            RespectSystemTheme::Off => ThemeChooserMode::SystemAgnostic,
        }
    }

    pub fn into_theme_kind(self, ctx: &AppContext) -> ThemeKind {
        let theme_settings = ThemeSettings::as_ref(ctx);
        let theme_kind = theme_settings.theme_kind.value();
        match (self, &respect_system_theme(theme_settings)) {
            (ThemeChooserMode::SystemAgnostic, _) => theme_kind.clone(),
            (ThemeChooserMode::SystemLight, RespectSystemTheme::On(system_themes)) => {
                system_themes.light.clone()
            }
            (ThemeChooserMode::SystemDark, RespectSystemTheme::On(system_themes)) => {
                system_themes.dark.clone()
            }
            (_, _) => ThemeKind::default(),
        }
    }

    fn render_hint_text(&self, appearance: &Appearance) -> Box<dyn Element> {
        let hint_text = match self {
            ThemeChooserMode::SystemAgnostic => appearance
                .ui_builder()
                .paragraph("Change your current theme.".to_string()),
            ThemeChooserMode::SystemLight => appearance
                .ui_builder()
                .paragraph("Pick a theme for when your system is in light mode.".to_string()),
            ThemeChooserMode::SystemDark => appearance
                .ui_builder()
                .paragraph("Pick a theme for when your system is in dark mode.".to_string()),
        };
        hint_text
            .build()
            .with_margin_left(TITLE_MARGIN)
            .with_margin_right(TITLE_MARGIN)
            .finish()
    }
}

pub struct ThemeChooser {
    button_mouse_states: MouseStateHandles,
    header_dimming_mouse_state: MouseStateHandle,
    list_state: UniformListState,
    scroll_state: ScrollStateHandle,
    selected_theme: Tracked<Option<ThemeKind>>,
    /// Default-view rows (sectioned). Used when the search box is empty.
    rows: Tracked<Vec<ThemeChooserRow>>,
    /// Search-mode rows (flat, fuzzy-ranked). `Some` only when the user has
    /// typed in the search box; `None` means render `rows` instead.
    filtered_rows: Tracked<Option<Vec<ThemeChooserRow>>>,
    mode: ThemeChooserMode,
    search_editor: ViewHandle<EditorView>,
    referral_theme_status: ModelHandle<ReferralThemeStatus>,
    tips_completed: ModelHandle<TipsCompleted>,
    window_id: warpui::WindowId,
}

#[derive(Debug)]
pub enum ThemeChooserAction {
    Close,
    Enter,
    Click(ThemeKind),
    Up,
    Down,
    OpenThemeCreator,
    OpenThemeDeletionModal(ThemeKind),
    ToggleFavorite(ThemeKind),
    /// Toggle the focused row's favorite state — bound to `f` in the picker.
    /// Resolves the focused row at dispatch time since the keybinding has no
    /// payload to carry a `ThemeKind`.
    ToggleFavoriteFocused,
}

pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    app.register_fixed_bindings(vec![
        FixedBinding::new("up", ThemeChooserAction::Up, id!("ThemeChooser")),
        FixedBinding::new("down", ThemeChooserAction::Down, id!("ThemeChooser")),
        FixedBinding::new("escape", ThemeChooserAction::Close, id!("ThemeChooser")),
        FixedBinding::new("enter", ThemeChooserAction::Enter, id!("ThemeChooser")),
        // ⌘D / Ctrl+D toggles the focused row's favorite. Plain letter
        // hotkeys would collide with typing into the search box, so we
        // gate behind a modifier (mnemonic: "bookmark", matching browser
        // bookmark shortcuts).
        FixedBinding::new(
            "cmdorctrl-d",
            ThemeChooserAction::ToggleFavoriteFocused,
            id!("ThemeChooser"),
        ),
    ]);
}

/// One row in the theme picker list. Header and HintRow are visual-only and
/// non-selectable; navigation skips them.
#[derive(Clone)]
enum ThemeChooserRow {
    Header(&'static str),
    HintRow(String),
    Item(ThemeChooserItem),
}

impl ThemeChooserRow {
    fn item_kind(&self) -> Option<&ThemeKind> {
        match self {
            ThemeChooserRow::Item(it) => Some(&it.kind),
            _ => None,
        }
    }

    fn is_selectable(&self) -> bool {
        matches!(self, ThemeChooserRow::Item(_))
    }
}

/// The 21 unconditional Cortex-curated built-in themes. Order matches the
/// historical sort order of `ThemeKind` discriminants for visual continuity.
/// Referral reward themes are handled separately because they're conditional
/// on `ReferralThemeStatus`; they appear in the "Your themes" section when
/// active.
fn cortex_builtin_kinds() -> Vec<ThemeKind> {
    use ThemeKind::*;
    vec![
        Adeberry,
        Phenomenon,
        Dark,
        Dracula,
        FancyDracula,
        CyberWave,
        SolarFlare,
        SolarizedDark,
        WillowDream,
        Light,
        DarkCity,
        GruvboxDark,
        RedRock,
        JellyFish,
        Leafy,
        Koi,
        SolarizedLight,
        Snowy,
        GruvboxLight,
        PinkCity,
        Marble,
    ]
}

fn item_for(kind: &ThemeKind, theme_config: &WarpThemeConfig) -> ThemeChooserItem {
    ThemeChooserItem::new(kind.clone(), theme_config.theme(kind))
}

/// Build the layered default view (no search): Favorites → Recents →
/// Your themes → Cortex built-ins → "browse the library" hint row.
fn build_default_rows(
    referral: &ReferralThemeStatus,
    theme_config: &WarpThemeConfig,
    recents: &[ThemeKind],
    favorites: &[ThemeKind],
) -> Vec<ThemeChooserRow> {
    let mut rows: Vec<ThemeChooserRow> = Vec::new();

    // Favorites section sits at the top so user-pinned themes are always one
    // glance away. Skip the section entirely when nothing is pinned. Filter
    // out inactive referral kinds the same way Recents does, just in case
    // they got pinned and then deactivated. Items here render with
    // `hide_star` so the section title carries the star instead.
    let favorite_items: Vec<ThemeChooserItem> = favorites
        .iter()
        .filter(|kind| match kind {
            ThemeKind::SentReferralReward => referral.sent_referral_theme_active(),
            ThemeKind::ReceivedReferralReward => referral.received_referral_theme_active(),
            _ => true,
        })
        .filter(|kind| theme_config.contains_theme(kind))
        .map(|kind| item_for(kind, theme_config).with_star_hidden())
        .collect();
    if !favorite_items.is_empty() {
        rows.push(ThemeChooserRow::Header(FAVORITES_HEADER_LABEL));
        rows.extend(favorite_items.into_iter().map(ThemeChooserRow::Item));
    }

    let mut already_in_recents: HashSet<ThemeKind> = HashSet::new();
    let recents_items: Vec<ThemeChooserItem> = recents
        .iter()
        .filter(|kind| {
            // Skip an inactive referral reward kind even if it lingers in the
            // user's MRU list.
            match kind {
                ThemeKind::SentReferralReward => referral.sent_referral_theme_active(),
                ThemeKind::ReceivedReferralReward => referral.received_referral_theme_active(),
                _ => true,
            }
        })
        .filter(|kind| already_in_recents.insert((*kind).clone()))
        .take(MAX_RECENT_THEMES)
        .map(|kind| item_for(kind, theme_config))
        .collect();
    if !recents_items.is_empty() {
        rows.push(ThemeChooserRow::Header("Recents"));
        rows.extend(recents_items.into_iter().map(ThemeChooserRow::Item));
    }

    let mut your_items: Vec<ThemeChooserItem> = theme_config
        .theme_items()
        .filter_map(|(kind, theme)| match kind {
            ThemeKind::Custom(_) | ThemeKind::CustomBase16(_) => {
                Some(ThemeChooserItem::new(kind.clone(), theme.clone()))
            }
            ThemeKind::SentReferralReward if referral.sent_referral_theme_active() => {
                Some(ThemeChooserItem::new(kind.clone(), theme.clone()))
            }
            ThemeKind::ReceivedReferralReward if referral.received_referral_theme_active() => {
                Some(ThemeChooserItem::new(kind.clone(), theme.clone()))
            }
            _ => None,
        })
        .collect();
    your_items.sort_by(|a, b| a.kind.cmp(&b.kind));
    if !your_items.is_empty() {
        rows.push(ThemeChooserRow::Header("Your themes"));
        rows.extend(your_items.into_iter().map(ThemeChooserRow::Item));
    }

    rows.push(ThemeChooserRow::Header("Cortex built-ins"));
    for kind in cortex_builtin_kinds() {
        rows.push(ThemeChooserRow::Item(item_for(&kind, theme_config)));
    }

    let bundled_count = theme_config
        .theme_items()
        .filter(|(kind, _)| matches!(kind, ThemeKind::Wezterm(_)))
        .count();
    if bundled_count > 0 {
        rows.push(ThemeChooserRow::HintRow(format!(
            "Type to search {} more community themes",
            bundled_count
        )));
    }

    rows
}

/// Build the search-mode rows: the Favorites section stays pinned at the
/// top (unfiltered, so newly favorited rows visibly anchor there even when
/// they don't match the query) and the fuzzy-ranked match list below
/// excludes anything already shown in Favorites to avoid duplicates.
fn build_search_rows(
    query: &str,
    referral: &ReferralThemeStatus,
    theme_config: &WarpThemeConfig,
    favorites: &[ThemeKind],
) -> Vec<ThemeChooserRow> {
    let mut rows: Vec<ThemeChooserRow> = Vec::new();

    // Favorites pinned at the top, unfiltered. This is the key UX
    // difference from build_default_rows-only: when the user favorites a
    // row mid-search, they need to see it land somewhere. Filtering by
    // query would hide favorites that don't match the current search,
    // breaking that visual continuity.
    let visible_favorites: Vec<ThemeChooserItem> = favorites
        .iter()
        .filter(|kind| match kind {
            ThemeKind::SentReferralReward => referral.sent_referral_theme_active(),
            ThemeKind::ReceivedReferralReward => referral.received_referral_theme_active(),
            _ => true,
        })
        .filter(|kind| theme_config.contains_theme(kind))
        .map(|kind| item_for(kind, theme_config).with_star_hidden())
        .collect();
    if !visible_favorites.is_empty() {
        rows.push(ThemeChooserRow::Header(FAVORITES_HEADER_LABEL));
        rows.extend(
            visible_favorites
                .iter()
                .cloned()
                .map(ThemeChooserRow::Item),
        );
    }

    let favorite_set: HashSet<ThemeKind> = favorites.iter().cloned().collect();

    let total_count = theme_config
        .theme_items()
        .filter(|(kind, _)| match kind {
            ThemeKind::SentReferralReward => referral.sent_referral_theme_active(),
            ThemeKind::ReceivedReferralReward => referral.received_referral_theme_active(),
            _ => true,
        })
        .count();

    let mut scored: Vec<(i64, ThemeChooserItem)> = theme_config
        .theme_items()
        .filter(|(kind, _)| match kind {
            ThemeKind::SentReferralReward => referral.sent_referral_theme_active(),
            ThemeKind::ReceivedReferralReward => referral.received_referral_theme_active(),
            _ => true,
        })
        .filter(|(kind, _)| !favorite_set.contains(kind))
        .filter_map(|(kind, theme)| {
            let name = kind.to_string();
            match_indices_case_insensitive(&name, query)
                .map(|m| (m.score, ThemeChooserItem::new(kind.clone(), theme.clone())))
        })
        .collect();
    // Higher score first; tie-break alphabetically by the rendered name.
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.kind.to_string().cmp(&b.1.kind.to_string()))
    });

    let match_count = scored.len();
    rows.extend(
        scored
            .into_iter()
            .map(|(_, item)| ThemeChooserRow::Item(item)),
    );

    if match_count > 0 {
        rows.push(ThemeChooserRow::HintRow(format!(
            "Showing {} of {} themes",
            match_count, total_count
        )));
    }

    rows
}

fn position_of_kind(rows: &[ThemeChooserRow], kind: &ThemeKind) -> Option<usize> {
    rows.iter().position(|r| match r {
        ThemeChooserRow::Item(item) => &item.kind == kind,
        _ => false,
    })
}

fn next_selectable_after(rows: &[ThemeChooserRow], from: Option<usize>) -> Option<usize> {
    let start = from.map(|i| i + 1).unwrap_or(0);
    rows.iter()
        .enumerate()
        .skip(start)
        .find(|(_, r)| r.is_selectable())
        .map(|(i, _)| i)
}

fn prev_selectable_before(rows: &[ThemeChooserRow], from: Option<usize>) -> Option<usize> {
    let upper = from.unwrap_or(rows.len());
    rows.iter()
        .enumerate()
        .take(upper)
        .rev()
        .find(|(_, r)| r.is_selectable())
        .map(|(i, _)| i)
}

fn first_selectable(rows: &[ThemeChooserRow]) -> Option<usize> {
    next_selectable_after(rows, None)
}

fn selectable_count(rows: &[ThemeChooserRow]) -> usize {
    rows.iter().filter(|r| r.is_selectable()).count()
}

impl ThemeChooser {
    pub fn new(
        referral_theme_status: ModelHandle<ReferralThemeStatus>,
        ctx: &mut ViewContext<Self>,
        tips_completed: ModelHandle<TipsCompleted>,
    ) -> Self {
        let search_editor = {
            ctx.add_typed_action_view(|ctx| {
                let appearance = Appearance::as_ref(ctx);
                let options = SingleLineEditorOptions {
                    text: TextOptions::ui_font_size(appearance),
                    propagate_and_no_op_vertical_navigation_keys:
                        PropagateAndNoOpNavigationKeys::Always,
                    ..Default::default()
                };
                EditorView::single_line(options, ctx)
            })
        };

        ctx.subscribe_to_view(&search_editor, move |me, _, event, ctx| {
            me.handle_editor_event(event, ctx);
        });

        ctx.subscribe_to_model(&referral_theme_status, |me, _, _, ctx| {
            me.update_themes(ctx);
        });

        let warp_config_handle = WarpConfig::handle(ctx);
        ctx.subscribe_to_model(&warp_config_handle, |me, _, event, ctx| {
            match event {
                WarpConfigUpdateEvent::Themes | WarpConfigUpdateEvent::Favorites => {
                    me.update_themes(ctx);
                    ctx.notify();
                }
                _ => {}
            }
        });

        // Subscribe to window state changes for focus dimming updates
        let state_handle: ModelHandle<WindowManager> = WindowManager::handle(ctx);
        ctx.subscribe_to_model(&state_handle, |_me, _, event, ctx| {
            match &event {
                StateEvent::ValueChanged { current, previous } => {
                    // Re-render if this window's focus state has changed
                    if WindowManager::did_window_change_focus(ctx.window_id(), current, previous) {
                        ctx.notify();
                    }
                }
            }
        });

        let recents = ThemeSettings::as_ref(ctx).recent_themes.value().clone();
        let warp_config_ref = WarpConfig::as_ref(ctx);
        let favorites = warp_config_ref.favorite_themes().to_vec();
        let rows = build_default_rows(
            referral_theme_status.as_ref(ctx),
            warp_config_ref.theme_config(),
            &recents,
            &favorites,
        );

        Self {
            rows: Tracked::new(rows),
            button_mouse_states: Default::default(),
            header_dimming_mouse_state: Default::default(),
            list_state: Default::default(),
            scroll_state: Default::default(),
            selected_theme: Tracked::new(None),
            filtered_rows: Tracked::new(None),
            mode: ThemeChooserMode::for_active_theme(ctx),
            search_editor,
            referral_theme_status,
            tips_completed,
            window_id: ctx.window_id(),
        }
    }

    pub fn handle_theme_change(&mut self, ctx: &mut ViewContext<Self>) {
        // Ensure that we are still showing the right mode and have the correct theme selected.
        // The only time this can get out of sync is if there's a cloud preferences change affecting settings.
        // Note that we intentionally read from the settings model, not appearance here, as
        // the appearance will give us the derived theme, but we are trying to stay in sync
        // with the actual theme settings.
        let theme_settings = ThemeSettings::as_ref(ctx);
        let respect_system_theme = respect_system_theme(theme_settings);
        let system_theme = ctx.system_theme();
        match (respect_system_theme, self.mode, system_theme) {
            (
                RespectSystemTheme::On(selected_system_themes),
                ThemeChooserMode::SystemLight,
                SystemTheme::Light,
            )
            | (
                RespectSystemTheme::On(selected_system_themes),
                ThemeChooserMode::SystemDark,
                SystemTheme::Dark,
            ) => {
                // If we are choosing the theme for the current mode, ensure that we update the chooser state to match the
                // model state.
                let theme = match system_theme {
                    SystemTheme::Light => selected_system_themes.light.clone(),
                    SystemTheme::Dark => selected_system_themes.dark.clone(),
                };
                self.select_theme(theme, ctx);
            }
            (RespectSystemTheme::Off, ThemeChooserMode::SystemAgnostic, _) => {
                // If we are choosing the global theme, ensure that we update the chooser state to match the
                // model state
                let theme = ThemeSettings::as_ref(ctx).theme_kind.value().clone();
                self.select_theme(theme, ctx);
            }
            _ => {
                // Otherwise, we don't need to update anything, as we are in a state where we are
                // choosing a theme for an inactive mode.
            }
        }
    }

    pub fn reload_and_set_custom_theme(&mut self, theme: ThemeKind, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move { load_theme_configs(&themes_dir()) },
            move |theme_chooser, loaded_themes, ctx| {
                ctx.update_model(&WarpConfig::handle(ctx), move |warp_config, ctx| {
                    warp_config.update_theme_config(loaded_themes, ctx);
                });
                theme_chooser.update_themes(ctx);
                theme_chooser.select_and_save_theme(&theme, ctx);
            },
        );
    }

    pub fn reload_and_set_latest_theme(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move { load_theme_configs(&themes_dir()) },
            move |theme_chooser, loaded_themes, ctx| {
                ctx.update_model(&WarpConfig::handle(ctx), move |warp_config, ctx| {
                    warp_config.update_theme_config(loaded_themes, ctx);
                });
                theme_chooser.update_themes(ctx);
                theme_chooser.select_latest_theme(ctx);
            },
        );
    }
    fn handle_editor_event(&mut self, event: &EditorEvent, ctx: &mut ViewContext<Self>) {
        match event {
            EditorEvent::Edited(_) => {
                let search_term = self.search_editor.as_ref(ctx).buffer_text(ctx);
                *self.filtered_rows = if search_term.is_empty() {
                    None
                } else {
                    let warp_config_ref = WarpConfig::as_ref(ctx);
                    let favorites = warp_config_ref.favorite_themes().to_vec();
                    Some(build_search_rows(
                        &search_term,
                        self.referral_theme_status.as_ref(ctx),
                        warp_config_ref.theme_config(),
                        &favorites,
                    ))
                };
                // Finding the position of the selected theme to adjust the scroll position of the
                // list of visible themes.
                let index = self.theme_position(self.selected_theme.clone().unwrap_or_default());
                self.list_state.scroll_to(index.unwrap_or_default());
            }
            EditorEvent::Navigate(NavigationKey::Up) => self.up(ctx),
            EditorEvent::Navigate(NavigationKey::Down) => self.down(ctx),
            EditorEvent::Enter => self.enter(ctx),
            EditorEvent::Escape => self.close(ctx),
            _ => {}
        }
    }

    pub fn record_open_theme(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        send_telemetry_from_ctx!(TelemetryEvent::OpenThemeChooser, ctx);
        true
    }

    pub fn open_theme_creator_modal(&mut self, ctx: &mut ViewContext<Self>) {
        send_telemetry_from_ctx!(TelemetryEvent::OpenThemeCreatorModal, ctx);
        ctx.emit(ThemeChooserEvent::OpenThemeCreatorModal);
    }

    pub fn open_theme_deletion_modal(
        &mut self,
        theme_kind: ThemeKind,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.emit(ThemeChooserEvent::OpenThemeDeletionModal(theme_kind));
    }

    pub fn set_mode(&mut self, mode: ThemeChooserMode) {
        self.mode = mode;
    }

    // this is actually used in our integration test assertions,
    // but rust thinks it's unused when running unit tests in this crate
    #[allow(dead_code)]
    pub fn themes(&self) -> impl Iterator<Item = &ThemeKind> {
        self.visible_rows().iter().filter_map(|r| r.item_kind())
    }

    /// The currently visible row sequence: search results when the user is
    /// searching, otherwise the layered default view.
    fn visible_rows(&self) -> &[ThemeChooserRow] {
        match self.filtered_rows.as_ref() {
            Some(filtered) => filtered.as_slice(),
            None => self.rows.as_slice(),
        }
    }

    fn push_recent_theme(&self, kind: &ThemeKind, ctx: &mut ViewContext<Self>) {
        // Custom themes reference local files that may not exist on other
        // machines, so we keep them out of the synced MRU list — same logic
        // the existing `Theme::current_value_is_syncable` uses.
        if matches!(
            kind,
            ThemeKind::Custom(_) | ThemeKind::CustomBase16(_) | ThemeKind::InMemory(_)
        ) {
            return;
        }

        let theme_settings = ThemeSettings::handle(ctx);
        let mut next = theme_settings.as_ref(ctx).recent_themes.value().clone();
        next.retain(|k| k != kind);
        next.insert(0, kind.clone());
        next.truncate(MAX_RECENT_THEMES);
        theme_settings.update(ctx, |theme_settings, ctx| {
            report_if_error!(theme_settings.recent_themes.set_value(next, ctx));
        });
    }

    fn close(&mut self, ctx: &mut ViewContext<Self>) {
        *self.selected_theme = None;
        *self.filtered_rows = None;
        AppearanceManager::handle(ctx).update(ctx, |appearance_manager, ctx| {
            appearance_manager.clear_transient_theme(ctx);
        });
        self.search_editor.update(ctx, |editor, ctx| {
            editor.clear_buffer_and_reset_undo_stack(ctx);
        });
        ctx.emit(ThemeChooserEvent::Close(self.mode));
    }

    fn enter(&mut self, ctx: &mut ViewContext<Self>) {
        // "Enter" should close the theme picker is there is a theme that's visibly selected
        // If the user has entered a search term, but no theme is selected, we should not close
        if self.is_selected_theme_visible() {
            self.close(ctx);
        } else {
            log::info!("Handled enter key in theme chooser, but no theme is visibly selected.")
        }
    }

    pub fn select_and_save_theme(
        &mut self,
        selected_kind: &ThemeKind,
        ctx: &mut ViewContext<Self>,
    ) {
        self.select_theme(selected_kind.clone(), ctx);
        send_telemetry_from_ctx!(
            TelemetryEvent::ThemeSelection {
                theme: selected_kind.to_string(),
                entrypoint: "theme_chooser".to_string()
            },
            ctx
        );
        self.push_recent_theme(selected_kind, ctx);
        let theme_settings = ThemeSettings::handle(ctx);

        let selected_themes = respect_system_theme(theme_settings.as_ref(ctx))
            .selected_system_themes()
            .cloned()
            .unwrap_or_default();
        match self.mode {
            ThemeChooserMode::SystemAgnostic => {
                theme_settings.update(ctx, |theme_settings, ctx| {
                    report_if_error!(theme_settings
                        .theme_kind
                        .set_value(selected_kind.clone(), ctx,));
                });
            }
            ThemeChooserMode::SystemLight => {
                theme_settings.update(ctx, |theme_settings, ctx| {
                    report_if_error!(theme_settings.selected_system_themes.set_value(
                        SelectedSystemThemes {
                            light: selected_kind.clone(),
                            dark: selected_themes.dark,
                        },
                        ctx,
                    ));
                });
            }
            ThemeChooserMode::SystemDark => {
                theme_settings.update(ctx, |theme_settings, ctx| {
                    report_if_error!(theme_settings.selected_system_themes.set_value(
                        SelectedSystemThemes {
                            light: selected_themes.light,
                            dark: selected_kind.clone(),
                        },
                        ctx,
                    ));
                });
            }
        };
    }

    fn theme_position(&self, kind: ThemeKind) -> Option<usize> {
        position_of_kind(self.visible_rows(), &kind)
    }

    pub fn select_theme(&mut self, kind: ThemeKind, ctx: &mut ViewContext<Self>) {
        let index = self.theme_position(kind.clone()).unwrap_or_default();

        self.list_state.scroll_to(index);

        *self.selected_theme = Some(kind.clone());

        self.tips_completed.update(ctx, |tips_completed, ctx| {
            mark_feature_used_and_write_to_user_defaults(
                Tip::Action(TipAction::ThemePicker),
                tips_completed,
                ctx,
            );
            ctx.notify();
        });

        AppearanceManager::handle(ctx).update(ctx, |appearance_manager, ctx| {
            appearance_manager.set_transient_theme(kind, ctx);
        });
    }

    pub fn select_latest_theme(&mut self, ctx: &mut ViewContext<Self>) {
        // "Latest" means the most recently registered theme — typically a
        // user-created custom theme that just landed in `Your themes`. Pick
        // the last selectable row in the current view; if there are none, no-op.
        let target = {
            let rows = self.visible_rows();
            let Some(index) = prev_selectable_before(rows, None) else {
                return;
            };
            let Some(kind) = rows.get(index).and_then(|r| r.item_kind().cloned()) else {
                return;
            };
            (index, kind)
        };
        let (index, kind) = target;

        self.list_state.scroll_to(index);
        *self.selected_theme = Some(kind.clone());

        self.tips_completed.update(ctx, |tips_completed, ctx| {
            mark_feature_used_and_write_to_user_defaults(
                Tip::Action(TipAction::ThemePicker),
                tips_completed,
                ctx,
            );
            ctx.notify();
        });

        AppearanceManager::handle(ctx).update(ctx, |appearance_manager, ctx| {
            appearance_manager.set_transient_theme(kind, ctx);
        });
    }

    fn update_themes(&mut self, ctx: &mut ViewContext<Self>) {
        let recents = ThemeSettings::as_ref(ctx).recent_themes.value().clone();
        let search_term = self.search_editor.as_ref(ctx).buffer_text(ctx);
        let warp_config_ref = WarpConfig::as_ref(ctx);
        let favorites = warp_config_ref.favorite_themes().to_vec();
        *self.rows = build_default_rows(
            self.referral_theme_status.as_ref(ctx),
            warp_config_ref.theme_config(),
            &recents,
            &favorites,
        );
        // If a search is active, the filtered rows also need to refresh —
        // otherwise toggling a favorite mid-search wouldn't visibly move
        // the row into / out of the pinned Favorites section.
        if !search_term.is_empty() {
            *self.filtered_rows = Some(build_search_rows(
                &search_term,
                self.referral_theme_status.as_ref(ctx),
                warp_config_ref.theme_config(),
                &favorites,
            ));
        }
    }

    fn up(&mut self, ctx: &mut ViewContext<Self>) {
        if self.visible_theme_count() == 0 {
            return;
        }

        let next = {
            let rows = self.visible_rows();
            let from = self
                .selected_theme
                .as_ref()
                .and_then(|kind| position_of_kind(rows, kind));
            let target = prev_selectable_before(rows, from)
                .or_else(|| first_selectable(rows))
                .unwrap_or(0);
            let target_kind = rows.get(target).and_then(|r| r.item_kind().cloned());
            (target, target_kind)
        };
        let (target, target_kind) = next;
        self.list_state.scroll_to(target);
        if let Some(kind) = target_kind {
            self.select_and_save_theme(&kind, ctx);
        }
    }

    fn down(&mut self, ctx: &mut ViewContext<Self>) {
        if self.visible_theme_count() == 0 {
            return;
        }

        let next = {
            let rows = self.visible_rows();
            let from = self
                .selected_theme
                .as_ref()
                .and_then(|kind| position_of_kind(rows, kind));
            let target = next_selectable_after(rows, from)
                .or_else(|| first_selectable(rows))
                .unwrap_or(0);
            let target_kind = rows.get(target).and_then(|r| r.item_kind().cloned());
            (target, target_kind)
        };
        let (target, target_kind) = next;
        self.list_state.scroll_to(target);
        if let Some(kind) = target_kind {
            self.select_and_save_theme(&kind, ctx);
        }
    }

    fn visible_theme_count(&self) -> usize {
        selectable_count(self.visible_rows())
    }

    fn is_selected_theme_visible(&self) -> bool {
        self.selected_theme
            .as_ref()
            .map(|selected_kind| self.theme_position(selected_kind.clone()).is_some())
            .unwrap_or(false)
    }

    fn click(&mut self, kind: ThemeKind, ctx: &mut ViewContext<Self>) {
        self.select_and_save_theme(&kind, ctx);
        ctx.emit(ThemeChooserEvent::Click);
    }

    /// Pin or unpin a theme as a favorite. The picker rebuilds via the
    /// `WarpConfigUpdateEvent::Favorites` subscription, so we don't have to
    /// touch `self.rows` here. No effect on the focused row, the search
    /// state, or the picker visibility — toggling is non-destructive.
    fn toggle_favorite(&mut self, kind: ThemeKind, ctx: &mut ViewContext<Self>) {
        let warp_config = WarpConfig::handle(ctx);
        warp_config.update(ctx, |warp_config, ctx| {
            warp_config.toggle_favorite_theme(kind, ctx);
        });
    }

    fn toggle_favorite_focused(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(kind) = self.selected_theme.as_ref().cloned() {
            self.toggle_favorite(kind, ctx);
        }
    }

    fn render_header(
        &self,
        traffic_light_data: Option<&TrafficLightData>,
        appearance: &Appearance,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let mut margin_left = 16.;

        let zoom_factor = WindowSettings::as_ref(app).zoom_level.as_zoom_factor();
        // Since this panel is always on the left, only account for left-side traffic lights.
        if let Some(width) = traffic_light_data
            .filter(|data| data.side == TrafficLightSide::Left)
            .map(|data| data.width(zoom_factor))
        {
            margin_left += width;
        }

        let close_button = close_button(
            appearance,
            self.button_mouse_states.close_button_mouse_state.clone(),
        )
        .build()
        .on_click(|ctx, _, _| ctx.dispatch_typed_action(ThemeChooserAction::Close))
        .finish();

        let header_element = ConstrainedBox::new(
            Flex::row()
                .with_child(
                    Container::new(Align::new(close_button).finish())
                        .with_margin_left(margin_left)
                        .with_margin_right(CLOSE_BUTTON_MARGIN_RIGHT)
                        .finish(),
                )
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::End)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .finish(),
        )
        .with_height(PANEL_HEADER_HEIGHT)
        .finish();

        // Apply dimming if window is not focused
        WindowFocusDimming::apply_panel_header_dimming(
            header_element,
            self.header_dimming_mouse_state.clone(),
            PANEL_HEADER_HEIGHT,
            appearance.theme().surface_1().into(),
            self.window_id,
            app,
        )
    }

    fn render_title_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let mut title_row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(
                Shrinkable::new(
                    1.0,
                    Align::new(
                        appearance
                            .ui_builder()
                            .span(THEME_CHOOSER_TITLE.to_string())
                            .with_style(UiComponentStyles {
                                font_family_id: Some(appearance.ui_font_family()),
                                font_size: Some(TITLE_FONT_SIZE),
                                font_weight: Some(Weight::Semibold),
                                ..Default::default()
                            })
                            .build()
                            .finish(),
                    )
                    .left()
                    .finish(),
                )
                .finish(),
            );

        // Custom themes are only supported on desktop platforms currently.
        if cfg!(not(target_family = "wasm")) {
            let create_theme_button = SavePosition::new(
                icon_button(
                    appearance,
                    icons::Icon::Plus,
                    false,
                    self.button_mouse_states
                        .create_theme_button_hover_state
                        .clone(),
                )
                .build()
                .on_click(|ctx, _, _| {
                    ctx.dispatch_typed_action(ThemeChooserAction::OpenThemeCreator)
                })
                .finish(),
                "create_theme_button",
            )
            .finish();

            title_row = title_row.with_child(create_theme_button);
        }

        Container::new(title_row.finish())
            .with_margin_bottom(6.)
            .with_margin_left(TITLE_MARGIN)
            .with_margin_right(TITLE_MARGIN)
            .finish()
    }

    fn render_search_bar(&self, appearance: &Appearance) -> Box<dyn Element> {
        Container::new(
            Flex::row()
                .with_child(
                    Container::new(
                        ConstrainedBox::new(
                            Icon::new(
                                "bundled/svg/find.svg",
                                appearance.theme().active_ui_detail(),
                            )
                            .finish(),
                        )
                        .with_height(10.)
                        .with_width(10.)
                        .finish(),
                    )
                    .with_margin_right(3.)
                    .with_padding_top(12.)
                    .finish(),
                )
                .with_child(
                    Shrinkable::new(
                        1.,
                        appearance
                            .ui_builder()
                            .text_input(self.search_editor.clone())
                            .with_style(UiComponentStyles {
                                border_radius: Some(CornerRadius::with_all(Radius::Pixels(0.))),
                                background: Some(Fill::None),
                                border_width: Some(0.),
                                ..Default::default()
                            })
                            .build()
                            .finish(),
                    )
                    .finish(),
                )
                .finish(),
        )
        .with_margin_left(TITLE_MARGIN)
        .finish()
    }

    fn render_list(&self, appearance: &Appearance) -> Box<dyn Element> {
        // Owned copy: the closure passed to UniformList is stored on the
        // element and called across layout passes, so it can't borrow from
        // `self` for the duration of one render.
        let rows: Vec<ThemeChooserRow> = self.visible_rows().to_vec();
        let selected_kind = self.selected_theme.clone();

        let selectable = selectable_count(&rows);
        let element = if selectable == 0 {
            // renders a text & an empty rectangle that expands over the panel
            // without it, the theme picker panel would be shorter than the terminal window
            Flex::column()
                .with_child(
                    appearance
                        .ui_builder()
                        .span("No matching themes!".to_string())
                        .build()
                        .finish(),
                )
                .with_child(Empty::new().finish())
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .finish()
        } else {
            let list_len = rows.len();
            let list = UniformList::new(self.list_state.clone(), list_len, move |range, ctx| {
                let appearance = Appearance::as_ref(ctx);
                let font_family = appearance.ui_font_family();
                let text_color: ColorU = appearance.theme().active_ui_text_color().into();
                let selected_background_color = appearance.theme().surface_2();
                let warp_config = WarpConfig::as_ref(ctx);

                rows.clone()
                    .into_iter()
                    .enumerate()
                    .skip(range.start)
                    .take(range.end - range.start)
                    .map(|(_, row)| match row {
                        ThemeChooserRow::Item(item) => {
                            let selected = match &selected_kind {
                                Some(selected_kind) => selected_kind == &item.kind,
                                None => false,
                            };
                            let is_favorite = warp_config.is_favorite_theme(&item.kind);
                            let element = item.render(
                                selected,
                                is_favorite,
                                font_family,
                                text_color,
                                selected_background_color.into(),
                            );
                            let kind_for_click = item.kind.clone();
                            EventHandler::new(element)
                                .on_left_mouse_down(move |ctx, _, _| {
                                    ctx.dispatch_typed_action(ThemeChooserAction::Click(
                                        kind_for_click.clone(),
                                    ));
                                    DispatchEventResult::StopPropagation
                                })
                                .finish()
                        }
                        ThemeChooserRow::Header(label) => {
                            render_section_header_row(label, font_family, appearance)
                        }
                        ThemeChooserRow::HintRow(text) => {
                            render_hint_row(&text, font_family, appearance)
                        }
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
            });
            let warp_theme = appearance.theme();

            Scrollable::vertical(
                self.scroll_state.clone(),
                list.finish_scrollable(),
                SCROLLBAR_WIDTH,
                warp_theme
                    .disabled_text_color(warp_theme.surface_2())
                    .into(),
                warp_theme.main_text_color(warp_theme.surface_2()).into(),
                Fill::None,
            )
            .finish()
        };

        Shrinkable::new(
            1.,
            ConstrainedBox::new(element).with_height(f32::MAX).finish(),
        )
        .finish()
    }
}

fn render_section_header_row(
    label: &'static str,
    font_family: FamilyId,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let label_span = appearance
        .ui_builder()
        .span(label.to_string())
        .with_style(UiComponentStyles {
            font_family_id: Some(font_family),
            font_size: Some(SECTION_HEADER_FONT_SIZE),
            font_weight: Some(Weight::Semibold),
            ..Default::default()
        })
        .build()
        .finish();

    // The "Favorites" header carries a filled star so the section title is
    // self-describing — and so per-row stars inside the section can be
    // suppressed without the section losing its visual identity.
    let header_content: Box<dyn Element> = if label == FAVORITES_HEADER_LABEL {
        let star = ConstrainedBox::new(
            Icon::new(
                FAVORITE_STAR_FILLED_SVG,
                appearance.theme().active_ui_text_color(),
            )
            .finish(),
        )
        .with_width(FAVORITES_HEADER_STAR_SIZE)
        .with_height(FAVORITES_HEADER_STAR_SIZE)
        .finish();
        Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(star)
            .with_child(
                Container::new(label_span)
                    .with_margin_left(FAVORITES_HEADER_STAR_GAP)
                    .finish(),
            )
            .finish()
    } else {
        label_span
    };

    let header = Container::new(header_content)
        .with_padding_top(SECTION_HEADER_MARGIN_TOP)
        .with_padding_bottom(SECTION_HEADER_MARGIN_BOTTOM)
        .with_padding_left(THEME_NAME_MARGIN_LEFT)
        .finish();

    // Pad to item-row height so UniformList's first-item height measurement
    // produces a row that comfortably fits both items and headers.
    Container::new(
        Flex::column()
            .with_child(header)
            .with_child(Empty::new().finish())
            .finish(),
    )
    .with_padding_top(THEME_CHOOSER_ITEM_PADDING)
    .with_padding_bottom(THEME_CHOOSER_ITEM_PADDING)
    .finish()
}

fn render_hint_row(
    text: &str,
    font_family: FamilyId,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let hint = Container::new(
        appearance
            .ui_builder()
            .span(text.to_string())
            .with_style(UiComponentStyles {
                font_family_id: Some(font_family),
                font_size: Some(HINT_ROW_FONT_SIZE),
                ..Default::default()
            })
            .build()
            .finish(),
    )
    .with_padding_top(HINT_ROW_PADDING)
    .with_padding_bottom(HINT_ROW_PADDING)
    .with_padding_left(THEME_NAME_MARGIN_LEFT)
    .finish();

    Container::new(
        Flex::column()
            .with_child(hint)
            .with_child(Empty::new().finish())
            .finish(),
    )
    .with_padding_top(THEME_CHOOSER_ITEM_PADDING)
    .with_padding_bottom(THEME_CHOOSER_ITEM_PADDING)
    .finish()
}

impl Entity for ThemeChooser {
    type Event = ThemeChooserEvent;
}

impl TypedActionView for ThemeChooser {
    type Action = ThemeChooserAction;

    fn handle_action(&mut self, action: &ThemeChooserAction, ctx: &mut ViewContext<Self>) {
        use ThemeChooserAction::*;

        match action {
            Up => self.up(ctx),
            Down => self.down(ctx),
            Click(kind) => self.click(kind.clone(), ctx),
            Close => self.close(ctx),
            Enter => self.enter(ctx),
            OpenThemeCreator => self.open_theme_creator_modal(ctx),
            OpenThemeDeletionModal(kind) => self.open_theme_deletion_modal(kind.clone(), ctx),
            ToggleFavorite(kind) => self.toggle_favorite(kind.clone(), ctx),
            ToggleFavoriteFocused => self.toggle_favorite_focused(ctx),
        }
    }
}

impl View for ThemeChooser {
    fn ui_name() -> &'static str {
        "ThemeChooser"
    }

    fn accessibility_contents(&self, _: &AppContext) -> Option<AccessibilityContent> {
        Some(AccessibilityContent::new(
                "Theme chooser. Unfortunately, theme chooser window isn't compatible with screen readers yet.",
                "Press escape to close.",
                WarpA11yRole::WindowRole,
        ))
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let traffic_light_data = traffic_light_data(app, self.window_id);

        Container::new(
            Flex::column()
                .with_child(self.render_header(traffic_light_data.as_ref(), appearance, app))
                .with_child(self.render_title_row(appearance))
                .with_child(self.mode.render_hint_text(appearance))
                .with_child(self.render_search_bar(appearance))
                .with_child(self.render_list(appearance))
                .finish(),
        )
        .finish()
    }

    fn on_focus(&mut self, focus_ctx: &FocusContext, ctx: &mut ViewContext<Self>) {
        if focus_ctx.is_self_focused() {
            ctx.focus(&self.search_editor);
        }
    }
}

#[derive(Clone)]
struct ThemeChooserItem {
    pub kind: ThemeKind,
    warp_theme: WarpTheme,
    mouse_state: MouseStateHandle,
    /// When true the right-side star is suppressed for this row. Set on
    /// items that live inside the Favorites section, where the section
    /// header already communicates favorited status.
    hide_star: bool,
}

impl ThemeChooserItem {
    pub fn new(kind: ThemeKind, warp_theme: WarpTheme) -> Self {
        Self {
            kind,
            warp_theme,
            mouse_state: MouseStateHandle::default(),
            hide_star: false,
        }
    }

    pub fn with_star_hidden(mut self) -> Self {
        self.hide_star = true;
        self
    }

    /// Compact palette preview: 16 small colored rects, one per ANSI color.
    /// Replaces the old mini-terminal thumbnail (`theme::render_preview`),
    /// which was visually richer but cost so much on cold-GPU first paint that
    /// the picker took 5-8s to appear. Swatches are essentially free.
    fn render_swatch_strip(&self) -> Box<dyn Element> {
        let normal = self.warp_theme.terminal_colors().normal;
        let bright = self.warp_theme.terminal_colors().bright;
        let palette: [_; 16] = [
            normal.black,
            normal.red,
            normal.green,
            normal.yellow,
            normal.blue,
            normal.magenta,
            normal.cyan,
            normal.white,
            bright.black,
            bright.red,
            bright.green,
            bright.yellow,
            bright.blue,
            bright.magenta,
            bright.cyan,
            bright.white,
        ];
        let mut row = Flex::row();
        for color in palette {
            row = row.with_child(
                ConstrainedBox::new(Rect::new().with_background_color(color.into()).finish())
                    .with_width(SWATCH_WIDTH)
                    .with_height(SWATCH_HEIGHT)
                    .finish(),
            );
        }
        Container::new(row.finish())
            .with_margin_left(SWATCH_STRIP_MARGIN_LEFT)
            .finish()
    }

    pub fn render(
        &self,
        is_selected: bool,
        is_favorite: bool,
        font_family: FamilyId,
        text_color: ColorU,
        selected_background_color: ColorU,
    ) -> Box<dyn Element> {
        Hoverable::new(self.mouse_state.clone(), |state| {
            let swatches = self.render_swatch_strip();
            let row_hovered = state.is_hovered();

            let name_text = Shrinkable::new(
                1.,
                Container::new(
                    Text::new_inline(self.kind.to_string(), font_family, THEME_NAME_FONT_SIZE)
                        .with_color(text_color)
                        .finish(),
                )
                .with_margin_left(THEME_NAME_MARGIN_LEFT)
                .finish(),
            )
            .finish();

            // Left group: swatches + name. Wrapped in a row so the outer
            // SpaceBetween row can push the star anchor to the far right.
            // Made flexible so the outer row passes a bounded main-axis
            // constraint — `name_text` is itself a Shrinkable, and a flex
            // with flexible children panics under an infinite constraint.
            let left_group = Shrinkable::new(
                1.,
                Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_child(swatches)
                    .with_child(name_text)
                    .finish(),
            )
            .finish();

            // Right group: optional delete (custom themes on hover) and the
            // ever-present star toggle. Building as a row keeps both controls
            // anchored together at the right edge.
            let mut right_group = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);

            // Custom themes get a delete circle on hover, immediately left
            // of the star.
            if matches!(self.kind, ThemeKind::Custom(_)) && row_hovered {
                let horizontal_line = ConstrainedBox::new(
                    Rect::new()
                        .with_background_color(ColorU::from_u32(0x000000ff))
                        .finish(),
                )
                .with_width(DELETE_BUTTON_LINE_WIDTH)
                .with_height(DELETE_BUTTON_LINE_HEIGHT)
                .finish();

                let delete_theme_button_circle = ConstrainedBox::new(
                    Rect::new()
                        .with_background(ColorU::from_u32(0xFF8272FF))
                        .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
                        .finish(),
                )
                .with_height(DELETE_BUTTON_SIZE)
                .with_width(DELETE_BUTTON_SIZE)
                .finish();

                let mut stack = Stack::new().with_child(delete_theme_button_circle);
                stack.add_positioned_child(
                    horizontal_line,
                    OffsetPositioning::offset_from_parent(
                        vec2f(0., 0.),
                        ParentOffsetBounds::WindowByPosition,
                        ParentAnchor::Center,
                        ChildAnchor::Center,
                    ),
                );

                let theme_kind = self.kind.clone();
                right_group = right_group.with_child(
                    EventHandler::new(
                        Container::new(stack.finish())
                            .with_margin_right(DELETE_BUTTON_MARGIN_RIGHT)
                            .finish(),
                    )
                    .on_left_mouse_down(move |ctx, _, _| {
                        ctx.dispatch_typed_action(ThemeChooserAction::OpenThemeDeletionModal(
                            theme_kind.clone(),
                        ));
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
                );
            }

            // Star is rendered on every row except those inside the
            // Favorites section (where the header already conveys it).
            // States:
            // - favorited → filled (full opacity)
            // - not favorited, row not hovered → hollow (dim)
            // - not favorited, row hovered → hollow (brighter)
            if !self.hide_star {
                let star_svg = if is_favorite {
                    FAVORITE_STAR_FILLED_SVG
                } else {
                    FAVORITE_STAR_OUTLINE_SVG
                };
                let star_opacity = if is_favorite {
                    1.0
                } else if row_hovered {
                    FAVORITE_STAR_HOVER_OPACITY
                } else {
                    FAVORITE_STAR_DIM_OPACITY
                };
                let kind_for_star = self.kind.clone();
                let star = Container::new(
                    ConstrainedBox::new(
                        Icon::new(star_svg, text_color)
                            .with_opacity(star_opacity)
                            .finish(),
                    )
                    .with_width(FAVORITE_STAR_SIZE)
                    .with_height(FAVORITE_STAR_SIZE)
                    .finish(),
                )
                .with_margin_right(FAVORITE_STAR_MARGIN_RIGHT)
                .finish();
                right_group = right_group.with_child(
                    EventHandler::new(star)
                        .on_left_mouse_down(move |ctx, _, _| {
                            ctx.dispatch_typed_action(ThemeChooserAction::ToggleFavorite(
                                kind_for_star.clone(),
                            ));
                            DispatchEventResult::StopPropagation
                        })
                        .finish(),
                );
            }

            let row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_child(left_group)
                .with_child(right_group.finish());

            let mut container = Container::new(row.finish())
                .with_padding_top(THEME_CHOOSER_ITEM_PADDING)
                .with_padding_bottom(THEME_CHOOSER_ITEM_PADDING);

            if is_selected {
                container = container.with_background_color(selected_background_color);
            }

            container.finish()
        })
        .with_cursor(Cursor::PointingHand)
        .finish()
    }
}
