use crate::model::OnboardingStateModel;
use crate::OnboardingEvent;

use super::OnboardingSlide;
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use ui_components::{button, Component as _, Options as _};
use warp_core::cortex::{BRAIN_PINK, CORTEX_ASCII};
use warp_core::send_telemetry_from_ctx;
use warp_core::ui::{appearance::Appearance, color::coloru_with_opacity, theme::Fill, Icon};
use warpui::{
    elements::{
        Align, ChildAnchor, ConstrainedBox, Container, CrossAxisAlignment, Flex,
        FormattedTextElement, MainAxisAlignment, MainAxisSize, OffsetPositioning, ParentAnchor,
        ParentElement, ParentOffsetBounds, Stack,
    },
    keymap::Keystroke,
    text_layout::TextAlignment,
    AppContext, Element, Entity, ModelHandle, SingletonEntity as _, TypedActionView, View,
    ViewContext,
};

// Brain renders via the Cortex `Icon::Brain` SVG (OpenMoji 🧠 derivative,
// app/assets/cortex/brain.svg). The SVG has internal padding (visible
// brain shape fills ~60% of its 72×72 viewBox), so the constraint-box
// size is set larger than the CORTEX text's `8 × font_size` line-box
// height to make the *visible* brain shape match the visible CORTEX
// glyph height. Tune both together if proportions drift.
const CORTEX_FONT_SIZE: f32 = 14.0;
const BRAIN_ICON_SIZE: f32 = 146.0;
const LINE_HEIGHT_RATIO: f32 = 1.0;

/// Cortex fork version, displayed in the top-left of the IntroSlide alongside
/// the Warp version (`ChannelState::app_version()`). See `docs/branding.md` for
/// the bumping policy. Single source of truth — promote to a shared module if a
/// second consumer emerges.
const CORTEX_VERSION: &str = "0.1.1";

/// Wordmark dimensions for the "Powered by Warp" credit. The source SVG
/// (`warp-logo-with-light-title.svg`) is 764×179 (aspect ~4.27:1); 86×20 keeps
/// the proportion intact at a height that pairs with 12pt prefix text.
const WARP_WORDMARK_HEIGHT: f32 = 20.0;
const WARP_WORDMARK_WIDTH: f32 = 86.0;

/// Outline-only pink button that inverts on hover: pink border + pink text by
/// default, fills to pink + bg-color text on hover/press. Used for the two
/// "Create / Log in to Warp Account" buttons on this slide. Co-located here
/// (not added to `ui_components::button::themes`) because it's currently a
/// single-use Cortex visual; lift to a shared module if reused.
struct CortexPinkOutline;

impl button::Theme for CortexPinkOutline {
    fn background(&self, state: button::State, _: &Appearance) -> Option<Fill> {
        match state {
            button::State::Default => None,
            button::State::Hovered | button::State::Pressed => Some(Fill::Solid(BRAIN_PINK)),
        }
    }

    fn text_color(&self, background: Option<Fill>, appearance: &Appearance) -> ColorU {
        match background {
            None => BRAIN_PINK,
            Some(_) => appearance.theme().background().into(),
        }
    }

    fn border(&self, _: &Appearance) -> Option<ColorU> {
        Some(BRAIN_PINK)
    }

    fn keyboard_shortcut_border(&self, text_color: ColorU, _: &Appearance) -> Option<ColorU> {
        Some(coloru_with_opacity(text_color, 60))
    }
}

#[derive(Clone, Debug)]
pub enum IntroSlideEvent {
    LoginRequested,
}

#[derive(Clone, Debug)]
pub enum IntroSlideAction {
    GetStartedClicked,
    LoginClicked,
}

pub struct IntroSlide {
    onboarding_state: ModelHandle<OnboardingStateModel>,
    create_account_button: button::Button,
    login_button: button::Button,
}

impl IntroSlide {
    pub(crate) fn new(onboarding_state: ModelHandle<OnboardingStateModel>) -> Self {
        Self {
            onboarding_state,
            create_account_button: button::Button::default(),
            login_button: button::Button::default(),
        }
    }
}

impl Entity for IntroSlide {
    type Event = IntroSlideEvent;
}

impl View for IntroSlide {
    fn ui_name() -> &'static str {
        "IntroSlide"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let content = self.render_centered_content(appearance);
        let centered = Container::new(Align::new(content).finish()).finish();

        let buttons_row = self.render_buttons_row(appearance);
        let version_corner = self.render_version_corner(appearance);

        let mut stack = Stack::new();
        stack.add_child(centered);
        stack.add_positioned_child(
            version_corner,
            OffsetPositioning::offset_from_parent(
                vec2f(12., 40.),
                ParentOffsetBounds::ParentBySize,
                ParentAnchor::TopLeft,
                ChildAnchor::TopLeft,
            ),
        );
        stack.add_positioned_child(
            buttons_row,
            OffsetPositioning::offset_from_parent(
                vec2f(0., -28.),
                ParentOffsetBounds::ParentBySize,
                ParentAnchor::BottomMiddle,
                ChildAnchor::BottomMiddle,
            ),
        );
        stack.finish()
    }
}

impl IntroSlide {
    fn get_started_clicked(&mut self, ctx: &mut ViewContext<Self>) {
        send_telemetry_from_ctx!(OnboardingEvent::GetStartedClicked, ctx);

        self.onboarding_state.update(ctx, |model, ctx| {
            model.next(ctx);
        });
    }
}

impl OnboardingSlide for IntroSlide {
    fn on_enter(&mut self, ctx: &mut ViewContext<Self>) {
        self.get_started_clicked(ctx);
    }
}

impl IntroSlide {
    fn render_centered_content(&self, appearance: &Appearance) -> Box<dyn Element> {
        let monospace = appearance.monospace_font_family();
        let theme = appearance.theme();
        let muted: ColorU = theme.sub_text_color(theme.background()).into();

        let brain = ConstrainedBox::new(
            Icon::Brain
                .to_warpui_icon(Fill::Solid(BRAIN_PINK))
                .finish(),
        )
        .with_width(BRAIN_ICON_SIZE)
        .with_height(BRAIN_ICON_SIZE)
        .finish();

        let cortex = FormattedTextElement::from_str(CORTEX_ASCII, monospace, CORTEX_FONT_SIZE)
            .with_color(BRAIN_PINK)
            .with_alignment(TextAlignment::Left)
            .with_line_height_ratio(LINE_HEIGHT_RATIO)
            .finish();

        let powered_by = FormattedTextElement::from_str("Powered by ", monospace, 12.0)
            .with_color(muted)
            .finish();

        let warp_mark = ConstrainedBox::new(
            Icon::WarpLogoWithLightTitle
                .to_warpui_icon(Fill::Solid(muted))
                .finish(),
        )
        .with_width(WARP_WORDMARK_WIDTH)
        .with_height(WARP_WORDMARK_HEIGHT)
        .finish();

        let credit = Flex::row()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(powered_by)
            .with_child(warp_mark)
            .finish();

        Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(brain)
            .with_child(Container::new(cortex).with_margin_top(6.).finish())
            .with_child(Container::new(credit).with_margin_top(16.).finish())
            .finish()
    }

    fn render_version_corner(&self, appearance: &Appearance) -> Box<dyn Element> {
        let monospace = appearance.monospace_font_family();
        let theme = appearance.theme();
        let muted: ColorU = theme.sub_text_color(theme.background()).into();

        let cortex_label = format!("Cortex v{}", CORTEX_VERSION);

        FormattedTextElement::from_str(cortex_label, monospace, 11.0)
            .with_color(muted)
            .finish()
    }

    fn render_buttons_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let enter = Keystroke::parse("enter").unwrap_or_default();

        let create_account = self.create_account_button.render(
            appearance,
            button::Params {
                content: button::Content::Label("Create a Warp Account".into()),
                theme: &CortexPinkOutline,
                options: button::Options {
                    keystroke: Some(enter),
                    on_click: Some(Box::new(|ctx, _app, _pos| {
                        ctx.dispatch_typed_action(IntroSlideAction::GetStartedClicked);
                    })),
                    ..button::Options::default(appearance)
                },
            },
        );

        let log_in = self.login_button.render(
            appearance,
            button::Params {
                content: button::Content::Label("Log in to Warp Account".into()),
                theme: &CortexPinkOutline,
                options: button::Options {
                    on_click: Some(Box::new(|ctx, _app, _pos| {
                        ctx.dispatch_typed_action(IntroSlideAction::LoginClicked);
                    })),
                    ..button::Options::default(appearance)
                },
            },
        );

        Flex::row()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(create_account)
            .with_child(Container::new(log_in).with_margin_left(12.).finish())
            .finish()
    }
}

impl TypedActionView for IntroSlide {
    type Action = IntroSlideAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            IntroSlideAction::GetStartedClicked => {
                self.get_started_clicked(ctx);
            }
            IntroSlideAction::LoginClicked => {
                send_telemetry_from_ctx!(OnboardingEvent::WelcomeLoginClicked, ctx);
                ctx.emit(IntroSlideEvent::LoginRequested);
            }
        }
    }
}
