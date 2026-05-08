use crate::model::OnboardingStateModel;
use crate::OnboardingEvent;

use super::OnboardingSlide;
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use ui_components::{button, Component as _, Options as _};
use warp_core::send_telemetry_from_ctx;
use warp_core::ui::{appearance::Appearance, theme::Fill, Icon};
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

const CORTEX_ASCII: &str = include_str!("../../../../app/assets/cortex/ascii/CortexAscii5.txt");

const BRAIN_PINK: ColorU = ColorU {
    r: 244,
    g: 182,
    b: 194,
    a: 255,
};

// Brain renders via the Cortex `Icon::Brain` SVG (OpenMoji 🧠 derivative,
// app/assets/cortex/brain.svg). The SVG has internal padding (visible
// brain shape fills ~60% of its 72×72 viewBox), so the constraint-box
// size is set larger than the CORTEX text's `8 × font_size` line-box
// height to make the *visible* brain shape match the visible CORTEX
// glyph height. Tune both together if proportions drift.
const CORTEX_FONT_SIZE: f32 = 14.0;
const BRAIN_ICON_SIZE: f32 = 224.0;
const LINE_HEIGHT_RATIO: f32 = 1.0;

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

        let mut stack = Stack::new();
        stack.add_child(centered);
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

        Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(cortex)
            .with_child(Container::new(brain).with_margin_top(24.).finish())
            .finish()
    }

    fn render_buttons_row(&self, appearance: &Appearance) -> Box<dyn Element> {
        let enter = Keystroke::parse("enter").unwrap_or_default();

        let create_account = self.create_account_button.render(
            appearance,
            button::Params {
                content: button::Content::Label("Create a Warp Account".into()),
                theme: &button::themes::Primary,
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
                theme: &button::themes::Primary,
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
