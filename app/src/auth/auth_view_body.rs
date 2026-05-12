use crate::{
    appearance::Appearance,
    auth::auth_view_shared_helpers::render_offline_contents,
    editor::{EditorView, InteractionState, SingleLineEditorOptions, TextColors, TextOptions},
    experiments::{AuthFlowInstructions, Experiment},
    modal::MODAL_CORNER_RADIUS,
    network::NetworkStatus,
    report_error, send_telemetry_from_ctx, send_telemetry_sync_from_ctx,
    server::telemetry::{AnonymousUserSignupEntrypoint, LoginEventSource, TelemetryEvent},
    settings::{AISettings, PrivacySettings},
    themes::theme::Fill as ThemeFill,
    util::color::{darken, lighten},
};

use anyhow::anyhow;
use lazy_static::lazy_static;
use warp_core::{
    cortex::{BRAIN_PINK, CORTEX_ASCII},
    features::FeatureFlag,
    ui::{
        appearance::DEFAULT_COMMAND_PALETTE_FONT_SIZE, builder::UiBuilder, theme::Fill as CoreFill,
        Icon,
    },
};
use warpui::{
    accessibility::{AccessibilityContent, WarpA11yRole},
    clipboard::ClipboardContent,
    color::ColorU,
    elements::{
        Align, Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Dismiss, Fill,
        Flex, FormattedTextElement, MainAxisAlignment, MainAxisSize, MouseStateHandle,
        ParentElement, Radius, Stack,
    },
    fonts::Weight,
    keymap::FixedBinding,
    text_layout::TextAlignment,
    ui_components::components::{Coords, UiComponent, UiComponentStyles},
    AppContext, Element, Entity, FocusContext, SingletonEntity, TypedActionView, UpdateModel, View,
    ViewContext, ViewHandle,
};

use super::{
    auth_manager::AuthManager,
    auth_view_modal::AuthViewVariant,
    auth_view_shared_helpers::{
        action_button_color_and_variant, render_offline_info_overlay_body, render_overlay,
        render_privacy_settings_overlay_body, PrivacySettingsActions, PrivacySettingsHandles,
    },
    AuthStateProvider,
};

const TOS_URL: &str = "https://www.warp.dev/terms-of-service";

const COMMON_BODY_UI_FONT_SIZE: f32 = 12.;
const AUTH_MODAL_GAP: f32 = 16.;

const AUTH_TOKEN_INPUT_PLACEHOLDER_TEXT: &str = "Auth Token";
const AUTH_TOKEN_INPUT_PLACEHOLDER_TEXT_EXPERIMENTAL: &str = "Browser auth token";

const AUTH_TOKEN_INPUT_BORDER_RADIUS: Radius = Radius::Pixels(4.);

lazy_static! {
    static ref BODY_TEXT_COLOR: ColorU = ColorU::new(157, 157, 157, 255);
    static ref HOVERED_BODY_TEXT_COLOR: ColorU = lighten(*BODY_TEXT_COLOR);
    static ref AUTH_TOKEN_INPUT_BACKGROUND: Fill = ColorU::white().into();
    static ref AUTH_TOKEN_INPUT_TEXT_COLOR: ThemeFill = ThemeFill::Solid(ColorU::black());
    static ref AUTH_TOKEN_INPUT_TEXT_DISABLED: ThemeFill =
        AUTH_TOKEN_INPUT_TEXT_COLOR.with_opacity(20);
    static ref AUTH_TOKEN_INPUT_TEXT_HINT: ThemeFill = AUTH_TOKEN_INPUT_TEXT_COLOR.with_opacity(40);
}

pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    app.register_fixed_bindings([FixedBinding::new(
        "enter",
        AuthViewBodyAction::Signup,
        id!("AuthViewBody"),
    )]);
    app.register_fixed_bindings([FixedBinding::new(
        "escape",
        AuthViewBodyAction::Close,
        id!("AuthViewBody"),
    )]);
}

#[derive(Default)]
struct MouseStateHandles {
    login_link_mouse_state_handle: MouseStateHandle,
    enter_login_later_mouse_state_handle: MouseStateHandle,
    confirm_login_later_mouse_state_handle: MouseStateHandle,
    show_auth_token_input_mouse_state_handle: MouseStateHandle,
    copy_browser_url_mouse_state_handle: MouseStateHandle,
    tos_mouse_state_handle: MouseStateHandle,
    sign_up_mouse_state_handle: MouseStateHandle,
    learn_more_mouse_state_handle: MouseStateHandle,
    privacy_settings_mouse_state_handle: MouseStateHandle,
    close_button_mouse_state_handle: MouseStateHandle,
}

#[derive(Copy, Clone, Debug)]
pub enum AuthViewOverlay {
    PrivacySettings,
    OfflineInfo,
}

pub struct AuthViewBody {
    variant: AuthViewVariant,
    mouse_state_handles: MouseStateHandles,
    privacy_settings_handles: PrivacySettingsHandles,
    active_overlay: Option<AuthViewOverlay>,
    auth_token_input: ViewHandle<EditorView>,
    show_auth_token_input: bool,
    auth_step: AuthStep,
    loginless_step: LoginlessStep,
    copy_url_click_count: u8,
    allow_loginless: bool,
}

/// State for two-step loginless flow for anonymous users
enum LoginlessStep {
    /// Initial state: user has not yet clicked "sign up later" entrypoint
    Start,
    /// Confirmation state: user has clicked "sign up later" and is now in confirmation view
    Initiated,
}

pub enum AuthStep {
    SelectAuthPathway,
    BrowserOpen,
}

#[derive(Clone, Copy, Debug)]
pub enum AuthViewBodyAction {
    Login,
    InitiateLoginLater,
    LoginLater,
    EnterToken,
    CopyLoginUrl,
    Signup,
    SignupAnonymousUser,
    ShowOverlay(AuthViewOverlay),
    HideOverlay,
    ToggleTelemetry,
    ToggleCrashReporting,
    ToggleCloudConversationStorage,
    Close,
}

impl AuthViewBody {
    pub fn new(variant: AuthViewVariant, ctx: &mut ViewContext<Self>) -> Self {
        let experiment_group = AuthFlowInstructions::get_group(ctx);
        let auth_token_input = ctx.add_typed_action_view(|ctx| {
            let appearance = Appearance::as_ref(ctx);
            let mut editor = EditorView::single_line(
                SingleLineEditorOptions {
                    text: TextOptions {
                        font_size_override: Some(COMMON_BODY_UI_FONT_SIZE),
                        font_family_override: Some(appearance.ui_font_family()),
                        text_colors_override: Some(TextColors {
                            default_color: *AUTH_TOKEN_INPUT_TEXT_COLOR,
                            disabled_color: *AUTH_TOKEN_INPUT_TEXT_DISABLED,
                            hint_color: *AUTH_TOKEN_INPUT_TEXT_HINT,
                        }),
                        ..Default::default()
                    },
                    soft_wrap: false,
                    ..Default::default()
                },
                ctx,
            );

            let placeholder_text =
                if matches!(experiment_group, Some(AuthFlowInstructions::Experiment)) {
                    AUTH_TOKEN_INPUT_PLACEHOLDER_TEXT_EXPERIMENTAL
                } else {
                    AUTH_TOKEN_INPUT_PLACEHOLDER_TEXT
                };

            editor.set_placeholder_text(placeholder_text, ctx);
            editor
        });

        ctx.subscribe_to_view(&auth_token_input, |me, _, event, ctx| {
            use crate::editor::Event::{AltEnter, CmdEnter, Enter, Paste, ShiftEnter};
            match event {
                AltEnter | CmdEnter | Enter | Paste | ShiftEnter => me.emit_token_entered(ctx),
                _ => {}
            };
            ctx.notify();
        });

        let allow_loginless = !FeatureFlag::ForceLogin.is_enabled();

        let network_status = NetworkStatus::handle(ctx);
        ctx.subscribe_to_model(&network_status, |_, _, _, ctx| {
            ctx.notify();
        });

        AuthViewBody {
            variant,
            mouse_state_handles: Default::default(),
            privacy_settings_handles: Default::default(),
            active_overlay: None,
            auth_token_input,
            show_auth_token_input: false,
            auth_step: AuthStep::SelectAuthPathway,
            loginless_step: LoginlessStep::Start,
            copy_url_click_count: 0,
            allow_loginless,
        }
    }

    pub fn handle_paste(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_auth_token_input = true;
        self.auth_token_input
            .update(ctx, |editor, ctx| editor.paste(ctx));
    }

    pub fn reset_login_screen(&mut self, ctx: &mut ViewContext<Self>) {
        self.reset_auth_token_input(ctx);
        self.auth_step = AuthStep::SelectAuthPathway;
        self.loginless_step = LoginlessStep::Start;
        self.copy_url_click_count = 0;
    }

    fn reset_auth_token_input(&mut self, ctx: &mut ViewContext<Self>) {
        self.set_input_editable(true, ctx);
        self.auth_token_input
            .update(ctx, |editor, ctx| editor.clear_buffer(ctx));
        self.show_auth_token_input = false;
    }

    pub fn set_input_editable(&mut self, is_editable: bool, ctx: &mut ViewContext<Self>) {
        let interaction_state = match is_editable {
            false => InteractionState::Disabled,
            true => InteractionState::Editable,
        };
        self.auth_token_input.update(ctx, |editor, ctx| {
            editor.set_interaction_state(interaction_state, ctx)
        });
    }

    pub fn set_variant(&mut self, variant: AuthViewVariant) {
        self.variant = variant;
    }

    fn emit_token_entered(&self, ctx: &mut ViewContext<Self>) {
        let text = self.auth_token_input.as_ref(ctx).buffer_text(ctx);
        ctx.emit(AuthViewBodyEvent::AuthTokenEntered(text));
    }

    fn privacy_settings_actions(&self) -> PrivacySettingsActions<AuthViewBodyAction> {
        PrivacySettingsActions {
            toggle_telemetry: AuthViewBodyAction::ToggleTelemetry,
            toggle_crash_reporting: AuthViewBodyAction::ToggleCrashReporting,
            toggle_cloud_conversation_storage: AuthViewBodyAction::ToggleCloudConversationStorage,
            hide_overlay: AuthViewBodyAction::HideOverlay,
        }
    }

    fn render_auth_token_suggest(&self, ui_builder: &UiBuilder) -> Box<dyn Element> {
        Flex::row()
            .with_child(
                ui_builder
                    .link(
                        "Click here to paste your token from the browser".into(),
                        None,
                        Some(Box::new(|ctx| {
                            ctx.dispatch_typed_action(AuthViewBodyAction::EnterToken);
                        })),
                        self.mouse_state_handles
                            .show_auth_token_input_mouse_state_handle
                            .clone(),
                    )
                    .soft_wrap(false)
                    .build()
                    .finish(),
            )
            .finish()
    }

    fn render_auth_token_input(&self, appearance: &Appearance) -> Option<Box<dyn Element>> {
        if !self.show_auth_token_input {
            return None;
        }

        Some(
            appearance
                .ui_builder()
                .text_input(self.auth_token_input.clone())
                .with_style(UiComponentStyles {
                    background: Some(*AUTH_TOKEN_INPUT_BACKGROUND),
                    border_width: Some(0.),
                    border_radius: Some(CornerRadius::with_all(AUTH_TOKEN_INPUT_BORDER_RADIUS)),
                    padding: Some(Coords {
                        top: 12.,
                        bottom: 12.,
                        left: 16.,
                        right: 16.,
                    }),
                    margin: Some(Coords {
                        top: 8.,
                        bottom: 0.,
                        left: 0.,
                        right: 0.,
                    }),
                    ..Default::default()
                })
                .build()
                .finish(),
        )
    }

    fn render_privacy_information(
        &self,
        appearance: &Appearance,
        ui_builder: &UiBuilder,
    ) -> Vec<Box<dyn Element>> {
        let disclaimer_color = appearance
            .theme()
            .sub_text_color(appearance.theme().background())
            .into();

        let disclaimer_styles = UiComponentStyles {
            font_color: Some(disclaimer_color),
            ..Default::default()
        };

        let link_styles = UiComponentStyles {
            font_color: Some(disclaimer_color),
            border_color: Some(Fill::Solid(disclaimer_color)),
            ..Default::default()
        };

        let disclaimer_line_1 = Container::new(
            Flex::row()
                .with_child(
                    ui_builder
                        .span("By continuing, you agree to Warp's ")
                        .with_style(disclaimer_styles)
                        .build()
                        .finish(),
                )
                .with_child(
                    ui_builder
                        .link(
                            "Terms of Service".into(),
                            Some(TOS_URL.into()),
                            None,
                            self.mouse_state_handles.tos_mouse_state_handle.clone(),
                        )
                        .soft_wrap(false)
                        .with_style(link_styles)
                        .build()
                        .finish(),
                )
                .finish(),
        )
        .with_margin_top(AUTH_MODAL_GAP)
        .with_margin_bottom(8.)
        .finish();

        let disclaimer_line_2 = if FeatureFlag::GlobalAIAnalyticsBanner.is_enabled() {
            Align::new(
                ui_builder
                    .link(
                        "Privacy Settings".into(),
                        None,
                        Some(Box::new(|ctx| {
                            ctx.dispatch_typed_action(AuthViewBodyAction::ShowOverlay(
                                AuthViewOverlay::PrivacySettings,
                            ));
                        })),
                        self.mouse_state_handles
                            .privacy_settings_mouse_state_handle
                            .clone(),
                    )
                    .soft_wrap(false)
                    .build()
                    .finish(),
            )
            .left()
            .finish()
        } else {
            Flex::column()
                .with_child(
                    ui_builder
                        .paragraph("If you'd like to opt out of analytics and AI features,")
                        .with_style(disclaimer_styles)
                        .build()
                        .finish(),
                )
                .with_child(
                    Flex::row()
                        .with_child(
                            ui_builder
                                .paragraph("you can adjust your ")
                                .with_style(disclaimer_styles)
                                .build()
                                .finish(),
                        )
                        .with_child(
                            ui_builder
                                .link(
                                    "Privacy Settings".into(),
                                    None,
                                    Some(Box::new(|ctx| {
                                        ctx.dispatch_typed_action(AuthViewBodyAction::ShowOverlay(
                                            AuthViewOverlay::PrivacySettings,
                                        ));
                                    })),
                                    self.mouse_state_handles
                                        .privacy_settings_mouse_state_handle
                                        .clone(),
                                )
                                .soft_wrap(false)
                                .with_style(link_styles)
                                .build()
                                .finish(),
                        )
                        .finish(),
                )
                .finish()
        };

        vec![disclaimer_line_1, disclaimer_line_2]
    }

    fn render_sign_up_button(
        &self,
        is_anonymous: bool,
        appearance: &Appearance,
        ui_builder: &UiBuilder,
    ) -> Box<dyn Element> {
        let on_click_action = if is_anonymous
            && matches!(
                self.variant,
                AuthViewVariant::RequireLoginCloseable
                    | AuthViewVariant::HitDriveObjectLimitCloseable
                    | AuthViewVariant::ShareRequirementCloseable
            ) {
            AuthViewBodyAction::SignupAnonymousUser
        } else {
            AuthViewBodyAction::Signup
        };

        self.render_pink_outline_button(
            appearance,
            ui_builder,
            "Sign up for a Warp account",
            self.mouse_state_handles.sign_up_mouse_state_handle.clone(),
            on_click_action,
        )
    }

    /// Cortex outlined-pink button shared across the AuthView Initial-variant
    /// CTAs (Sign up / Sign in / Skip for now). Default: transparent
    /// background, pink border, pink text. Hover: pink fill, background-color
    /// text. Press: darker pink fill. Mirrors the IntroSlide
    /// `CortexPinkOutline` button theme but expressed via `UiBuilder`'s
    /// custom-styles API since `auth_view_body` doesn't use the
    /// `ui_components::button::Button` system. Centered text within the
    /// button bounds via `with_centered_text_label`.
    fn render_pink_outline_button(
        &self,
        appearance: &Appearance,
        ui_builder: &UiBuilder,
        label: &str,
        mouse_state: MouseStateHandle,
        on_click_action: AuthViewBodyAction,
    ) -> Box<dyn Element> {
        let (_, button_variant) = action_button_color_and_variant(appearance);
        let bg_color: ColorU = appearance.theme().background().into();

        let base_styles = UiComponentStyles {
            font_size: Some(14.),
            font_family_id: Some(appearance.ui_font_family()),
            font_weight: Some(Weight::Bold),
            font_color: Some(BRAIN_PINK),
            background: Some(Fill::Solid(ColorU::transparent_black())),
            border_width: Some(1.),
            border_color: Some(Fill::Solid(BRAIN_PINK)),
            border_radius: Some(CornerRadius::with_all(Radius::Pixels(4.))),
            padding: Some(Coords {
                top: 0.,
                bottom: 0.,
                left: 12.,
                right: 12.,
            }),
            height: Some(40.),
            ..Default::default()
        };

        let hover_styles = UiComponentStyles {
            font_color: Some(bg_color),
            background: Some(Fill::Solid(BRAIN_PINK)),
            ..base_styles
        };

        let click_styles = UiComponentStyles {
            background: Some(Fill::Solid(darken(BRAIN_PINK))),
            ..hover_styles
        };

        let label_owned: String = label.to_string();

        ui_builder
            .button_with_custom_styles(
                button_variant,
                mouse_state,
                base_styles,
                Some(hover_styles),
                Some(click_styles),
                None,
            )
            .with_centered_text_label(label_owned.into())
            .build()
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(on_click_action);
            })
            .finish()
    }

    #[allow(dead_code)]
    fn render_sign_in_row(&self, ui_builder: &UiBuilder) -> Box<dyn Element> {
        Flex::row()
            .with_child(
                ui_builder
                    .span("Already have an account? ")
                    .build()
                    .finish(),
            )
            .with_child(
                ui_builder
                    .link(
                        "Sign in".into(),
                        None,
                        Some(Box::new(|ctx| {
                            ctx.dispatch_typed_action(AuthViewBodyAction::Login);
                        })),
                        self.mouse_state_handles
                            .login_link_mouse_state_handle
                            .clone(),
                    )
                    .soft_wrap(false)
                    .build()
                    .finish(),
            )
            .finish()
    }

    #[allow(dead_code)]
    fn render_sign_up_later_row(&self, ui_builder: &UiBuilder) -> Box<dyn Element> {
        Container::new(
            Flex::row()
                .with_child(
                    ui_builder
                        .span("Don't want to sign in right now? ")
                        .build()
                        .finish(),
                )
                .with_child(
                    ui_builder
                        .link(
                            "Skip for now".into(),
                            None,
                            Some(Box::new(|ctx| {
                                ctx.dispatch_typed_action(AuthViewBodyAction::InitiateLoginLater);
                            })),
                            self.mouse_state_handles
                                .enter_login_later_mouse_state_handle
                                .clone(),
                        )
                        .soft_wrap(false)
                        .build()
                        .finish(),
                )
                .finish(),
        )
        .with_margin_top(8.)
        .finish()
    }

    fn render_sign_in_later_confirm_row(&self, ui_builder: &UiBuilder) -> Box<dyn Element> {
        Container::new(
            Flex::column()
                .with_child(
                    ui_builder
                        .paragraph("Are you sure you want to skip login?")
                        .build()
                        .finish(),
                )
                .with_child(
                    ui_builder
                        .paragraph("You can sign up later, but some features, such as AI,")
                        .build()
                        .finish(),
                )
                .with_child(
                    Flex::row()
                        .with_child(
                            ui_builder
                                .span("are only available to logged-in users. ")
                                .build()
                                .finish(),
                        )
                        .with_child(
                            ui_builder
                                .link(
                                    "Yes, skip login".into(),
                                    None,
                                    Some(Box::new(|ctx| {
                                        ctx.dispatch_typed_action(AuthViewBodyAction::LoginLater);
                                    })),
                                    self.mouse_state_handles
                                        .confirm_login_later_mouse_state_handle
                                        .clone(),
                                )
                                .soft_wrap(false)
                                .build()
                                .finish(),
                        )
                        .finish(),
                )
                .finish(),
        )
        .with_margin_top(8.)
        .finish()
    }

    fn render_force_login_disclaimer(
        &self,
        appearance: &Appearance,
        ui_builder: &UiBuilder,
    ) -> Box<dyn Element> {
        let disclaimer_color = appearance
            .theme()
            .sub_text_color(appearance.theme().background())
            .into();

        let disclaimer_styles = UiComponentStyles {
            font_color: Some(disclaimer_color),
            ..Default::default()
        };

        let text = match self.variant {
            AuthViewVariant::RequireLoginCloseable  => {
                "In order to use Warp’s AI features or collaborate with others, please create an account."
            }
            AuthViewVariant::HitDriveObjectLimitCloseable => {
                "In order to create more objects in Warp Drive, please create an account."
            }
            AuthViewVariant::ShareRequirementCloseable => {
                "In order to share, please create an account."
            }
            _ => "",
        };

        Container::new(
            ui_builder
                .paragraph(text)
                .with_style(disclaimer_styles)
                .build()
                .finish(),
        )
        .with_margin_bottom(AUTH_MODAL_GAP)
        .finish()
    }

    fn render_header(&self, appearance: &Appearance, ui_builder: &UiBuilder) -> Box<dyn Element> {
        let header_styles = UiComponentStyles {
            font_family_id: Some(appearance.header_font_family()),
            font_color: Some(appearance.theme().active_ui_text_color().into()),
            font_size: Some(20.),
            font_weight: Some(Weight::Semibold),
            ..Default::default()
        };

        let text = match self.variant {
            AuthViewVariant::Initial => "Welcome to Cortex",
            AuthViewVariant::RequireLoginCloseable
            | AuthViewVariant::HitDriveObjectLimitCloseable
            | AuthViewVariant::ShareRequirementCloseable => "Sign up for Warp",
        };

        ui_builder
            .span(text)
            .with_style(header_styles)
            .build()
            .finish()
    }

    fn render_logo_row(&self, appearance: &Appearance, ui_builder: &UiBuilder) -> Box<dyn Element> {
        // Cortex variant of the upstream `render_square_logo` helper:
        // same rounded-square dark wrapper, but with `Icon::Brain` tinted
        // BRAIN_PINK instead of the Warp wordmark SVG. The upstream
        // `render_square_logo` is preserved for the offline overlay /
        // paste-token modal surfaces (auth_view_shared_helpers.rs:186, :365)
        // until those are separately addressed.
        let logo = ConstrainedBox::new(
            Container::new(
                Icon::Brain
                    .to_warpui_icon(CoreFill::Solid(BRAIN_PINK))
                    .finish(),
            )
            .with_background(appearance.theme().surface_2())
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(10.)))
            .with_horizontal_padding(11.)
            .finish(),
        )
        .with_width(64.)
        .with_height(64.)
        .finish();
        let mut row = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
            .with_child(logo);

        if matches!(
            self.variant,
            AuthViewVariant::RequireLoginCloseable
                | AuthViewVariant::HitDriveObjectLimitCloseable
                | AuthViewVariant::ShareRequirementCloseable
        ) {
            let close_button = ui_builder
                .close_button(
                    24.,
                    self.mouse_state_handles
                        .close_button_mouse_state_handle
                        .clone(),
                )
                .build()
                .on_click(|ctx, _, _| ctx.dispatch_typed_action(AuthViewBodyAction::Close))
                .finish();
            row = row.with_child(close_button)
        };

        row.finish()
    }

    /// Cortex variant of the Initial-variant header: horizontal row with the
    /// brain glyph (still in the dark rounded square) on the left, and a
    /// stacked "Welcome to" line + CORTEX ASCII figlet on the right. Used
    /// only in the Initial variant of `render_select_auth_pathway_content`;
    /// non-Initial variants keep `render_logo_row` + `render_header` so the
    /// "Sign up for Warp" Warp-Drive-share-boundary flows stay upstream-shaped.
    fn render_welcome_header(
        &self,
        appearance: &Appearance,
        ui_builder: &UiBuilder,
    ) -> Box<dyn Element> {
        // No dark-square backdrop (was needed for the Warp wordmark's
        // contrast on dark; the pink brain glyph stands on its own).
        let logo = ConstrainedBox::new(
            Icon::Brain
                .to_warpui_icon(CoreFill::Solid(BRAIN_PINK))
                .finish(),
        )
        .with_width(64.)
        .with_height(64.)
        .finish();

        let welcome_to_styles = UiComponentStyles {
            font_family_id: Some(appearance.ui_font_family()),
            font_color: Some(appearance.theme().active_ui_text_color().into()),
            font_size: Some(14.),
            font_weight: Some(Weight::Medium),
            ..Default::default()
        };
        let welcome_to = ui_builder
            .span("Welcome to")
            .with_style(welcome_to_styles)
            .build()
            .finish();

        let monospace = appearance.monospace_font_family();
        let cortex_ascii = FormattedTextElement::from_str(CORTEX_ASCII, monospace, 7.0)
            .with_color(BRAIN_PINK)
            .with_alignment(TextAlignment::Left)
            .with_line_height_ratio(1.0)
            .finish();

        let right_column = Flex::column()
            .with_main_axis_size(MainAxisSize::Min)
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(welcome_to)
            .with_child(Container::new(cortex_ascii).with_margin_top(4.).finish())
            .finish();

        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(logo)
            .with_child(Container::new(right_column).with_margin_left(16.).finish())
            .finish()
    }

    /// "Powered by [Warp wordmark]" credit line — same pattern as the
    /// IntroSlide credit. Light-gray prefix text + the Warp logo-with-title
    /// SVG, sized small. Sits between the welcome header and the three
    /// action buttons on the Initial-variant modal.
    fn render_powered_by_warp(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let muted: ColorU = theme.sub_text_color(theme.background()).into();
        let monospace = appearance.monospace_font_family();

        let powered_by = FormattedTextElement::from_str("Powered by ", monospace, 12.0)
            .with_color(muted)
            .finish();

        let warp_mark = ConstrainedBox::new(
            Icon::WarpLogoWithLightTitle
                .to_warpui_icon(CoreFill::Solid(muted))
                .finish(),
        )
        .with_width(86.)
        .with_height(20.)
        .finish();

        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_main_axis_alignment(MainAxisAlignment::Center)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(powered_by)
            .with_child(warp_mark)
            .finish()
    }

    fn render_select_auth_pathway_content(
        &self,
        is_anonymous: bool,
        appearance: &Appearance,
        ui_builder: &UiBuilder,
        app: &AppContext,
    ) -> Vec<Box<dyn Element>> {
        let logo = Container::new(self.render_logo_row(appearance, ui_builder))
            .with_margin_bottom(AUTH_MODAL_GAP)
            .finish();
        let header = Container::new(self.render_header(appearance, ui_builder))
            .with_margin_bottom(AUTH_MODAL_GAP)
            .finish();
        let sign_up_button = self.render_sign_up_button(is_anonymous, appearance, ui_builder);
        let sign_in_button = Container::new(self.render_pink_outline_button(
            appearance,
            ui_builder,
            "Sign in to your Warp account",
            self.mouse_state_handles
                .login_link_mouse_state_handle
                .clone(),
            AuthViewBodyAction::Login,
        ))
        .with_margin_top(AUTH_MODAL_GAP)
        .finish();
        let force_login_disclaimer = self.render_force_login_disclaimer(appearance, ui_builder);

        match self.variant {
            AuthViewVariant::Initial => {
                if !NetworkStatus::as_ref(app).is_online() {
                    // Offline path keeps the upstream-style logo+header pair —
                    // it's a degraded state, not a welcome, so the Cortex
                    // welcome-header treatment doesn't apply.
                    let offline_contents = render_offline_contents(
                        appearance,
                        ui_builder,
                        self.mouse_state_handles
                            .learn_more_mouse_state_handle
                            .clone(),
                        AuthViewBodyAction::ShowOverlay(AuthViewOverlay::OfflineInfo),
                    );
                    vec![logo, header, offline_contents]
                } else if self.active_overlay.is_none() {
                    // Cortex Initial-variant header: horizontal brain glyph +
                    // "Welcome to / CORTEX" pair, followed by a "Powered by
                    // [Warp logo+wordmark]" credit, then the three pink-outline
                    // action buttons.
                    let welcome_header =
                        Container::new(self.render_welcome_header(appearance, ui_builder))
                            .with_margin_bottom(AUTH_MODAL_GAP)
                            .finish();
                    let powered_by = Container::new(self.render_powered_by_warp(appearance))
                        .with_margin_bottom(AUTH_MODAL_GAP)
                        .finish();

                    let mut contents = if self.allow_loginless {
                        match self.loginless_step {
                            LoginlessStep::Start => {
                                let skip_button = Container::new(self.render_pink_outline_button(
                                    appearance,
                                    ui_builder,
                                    "Skip for now",
                                    self.mouse_state_handles
                                        .enter_login_later_mouse_state_handle
                                        .clone(),
                                    AuthViewBodyAction::InitiateLoginLater,
                                ))
                                .with_margin_top(AUTH_MODAL_GAP)
                                .finish();
                                vec![
                                    welcome_header,
                                    powered_by,
                                    sign_up_button,
                                    sign_in_button,
                                    skip_button,
                                ]
                            }
                            LoginlessStep::Initiated => {
                                let confirm_row = Container::new(
                                    self.render_sign_in_later_confirm_row(ui_builder),
                                )
                                .with_margin_top(AUTH_MODAL_GAP)
                                .finish();
                                vec![
                                    welcome_header,
                                    powered_by,
                                    sign_up_button,
                                    sign_in_button,
                                    confirm_row,
                                ]
                            }
                        }
                    } else {
                        vec![welcome_header, powered_by, sign_up_button, sign_in_button]
                    };

                    contents.append(&mut self.render_privacy_information(appearance, ui_builder));
                    contents
                } else {
                    vec![]
                }
            }
            AuthViewVariant::RequireLoginCloseable
            | AuthViewVariant::HitDriveObjectLimitCloseable
            | AuthViewVariant::ShareRequirementCloseable => {
                vec![logo, header, force_login_disclaimer, sign_up_button]
            }
        }
    }

    fn render_browser_open_content(
        &self,
        appearance: &Appearance,
        ui_builder: &UiBuilder,
    ) -> Vec<Box<dyn Element>> {
        let logo = Container::new(self.render_logo_row(appearance, ui_builder))
            .with_margin_bottom(AUTH_MODAL_GAP)
            .finish();

        let header_styles = UiComponentStyles {
            font_family_id: Some(appearance.header_font_family()),
            font_color: Some(appearance.theme().active_ui_text_color().into()),
            font_size: Some(20.),
            font_weight: Some(Weight::Semibold),
            ..Default::default()
        };

        let header = Container::new(
            ui_builder
                .paragraph("Sign in on your browser \nto continue")
                .with_style(header_styles)
                .build()
                .finish(),
        )
        .with_margin_bottom(AUTH_MODAL_GAP)
        .finish();

        let hint = Container::new(
            Flex::column()
                .with_child(
                    Flex::row()
                        .with_child(
                            ui_builder
                                .span("If your browser hasn't launched, ")
                                .build()
                                .finish(),
                        )
                        .with_child(
                            ui_builder
                                .link(
                                    "copy the URL".into(),
                                    None,
                                    Some(Box::new(|event_ctx| {
                                        event_ctx.dispatch_typed_action(
                                            AuthViewBodyAction::CopyLoginUrl,
                                        );
                                    })),
                                    self.mouse_state_handles
                                        .copy_browser_url_mouse_state_handle
                                        .clone(),
                                )
                                .soft_wrap(false)
                                .build()
                                .finish(),
                        )
                        .finish(),
                )
                .with_child(
                    ui_builder
                        .span("and open the page manually.")
                        .build()
                        .finish(),
                )
                .finish(),
        )
        .finish();

        let mut contents = vec![logo, header, hint];

        let auth_token = Container::new(
            if let Some(auth_token_input) = self.render_auth_token_input(appearance) {
                auth_token_input
            } else {
                self.render_auth_token_suggest(ui_builder)
            },
        )
        .with_margin_top(AUTH_MODAL_GAP)
        .finish();

        contents.push(auth_token);
        contents
    }

    pub fn set_auth_step(&mut self, step: AuthStep) {
        self.auth_step = step;
    }
}

pub enum AuthViewBodyEvent {
    SignUpButtonClicked,
    AuthTokenEntered(String),
    LoginLaterClicked,
    Close,
}

impl Entity for AuthViewBody {
    type Event = AuthViewBodyEvent;
}

impl TypedActionView for AuthViewBody {
    type Action = AuthViewBodyAction;

    fn handle_action(&mut self, action: &AuthViewBodyAction, ctx: &mut ViewContext<Self>) {
        match action {
            AuthViewBodyAction::Login => {
                send_telemetry_from_ctx!(
                    TelemetryEvent::LoginButtonClicked {
                        source: LoginEventSource::AuthModal,
                    },
                    ctx
                );
                self.auth_step = AuthStep::BrowserOpen;

                AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                    let sign_in_url = auth_manager.sign_in_url();
                    ctx.open_url(&sign_in_url);
                });
            }
            AuthViewBodyAction::InitiateLoginLater => {
                send_telemetry_from_ctx!(
                    TelemetryEvent::LoginLaterButtonClicked {
                        source: LoginEventSource::AuthModal,
                    },
                    ctx
                );
                self.loginless_step = LoginlessStep::Initiated;
            }
            AuthViewBodyAction::LoginLater => {
                // Send synchronously since this is an important event in the sign up funnel and we
                // don't want to lose events if the user quits before the event queue is flushed.
                send_telemetry_sync_from_ctx!(
                    TelemetryEvent::LoginLaterConfirmationButtonClicked {
                        source: LoginEventSource::AuthModal,
                    },
                    ctx
                );
                ctx.emit(AuthViewBodyEvent::LoginLaterClicked);
            }
            AuthViewBodyAction::EnterToken => {
                self.auth_token_input
                    .update(ctx, |editor, ctx| editor.paste(ctx));
                self.show_auth_token_input = true;

                ctx.notify();
            }
            AuthViewBodyAction::CopyLoginUrl => {
                self.copy_url_click_count += 1;
                if AuthStateProvider::as_ref(ctx)
                    .get()
                    .is_user_anonymous()
                    .unwrap_or_default()
                {
                    AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                        auth_manager.copy_anonymous_user_linking_url_to_clipboard(ctx);
                    });
                } else {
                    AuthManager::handle(ctx).update(ctx, |auth_manager, inner_ctx| {
                        let sign_in_url = auth_manager.sign_in_url();
                        inner_ctx.clipboard().write(ClipboardContent {
                            plain_text: sign_in_url.clone(),
                            paths: Some(vec![sign_in_url]),
                            ..Default::default()
                        });
                    });
                }
            }
            AuthViewBodyAction::Signup => {
                // Send synchronously since this is an important event in the sign up funnel and we
                // don't want to lose events if the user quits before the event queue is flushed.
                send_telemetry_sync_from_ctx!(TelemetryEvent::SignUpButtonClicked, ctx);
                self.auth_step = AuthStep::BrowserOpen;

                AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                    let sign_up_url = auth_manager.sign_up_url();
                    ctx.open_url(&sign_up_url);
                });
            }
            AuthViewBodyAction::SignupAnonymousUser => {
                let entrypoint = match self.variant {
                    AuthViewVariant::RequireLoginCloseable
                    | AuthViewVariant::ShareRequirementCloseable => {
                        AnonymousUserSignupEntrypoint::LoginGatedFeature
                    }
                    AuthViewVariant::HitDriveObjectLimitCloseable => {
                        AnonymousUserSignupEntrypoint::HitDriveObjectLimit
                    }
                    AuthViewVariant::Initial => {
                        report_error!(anyhow!(
                            "Anonymous user initiated sign-up from unexpected AuthView variant"
                        ));
                        AnonymousUserSignupEntrypoint::Unknown
                    }
                };

                AuthManager::handle(ctx).update(ctx, |auth_manager, ctx| {
                    auth_manager.initiate_anonymous_user_linking(entrypoint, ctx);
                });
                self.auth_step = AuthStep::BrowserOpen;
                ctx.emit(AuthViewBodyEvent::SignUpButtonClicked);
            }
            AuthViewBodyAction::ShowOverlay(overlay) => {
                if let AuthViewOverlay::PrivacySettings = overlay {
                    send_telemetry_sync_from_ctx!(
                        TelemetryEvent::OpenAuthPrivacySettings {
                            source: LoginEventSource::AuthModal,
                        },
                        ctx
                    );
                }
                self.active_overlay = Some(*overlay);
                ctx.notify();
            }
            AuthViewBodyAction::HideOverlay => {
                self.active_overlay = None;
                ctx.notify();
            }
            AuthViewBodyAction::ToggleTelemetry => {
                let privacy_settings_handle = PrivacySettings::handle(ctx);
                ctx.update_model(&privacy_settings_handle, |privacy_settings, ctx| {
                    privacy_settings
                        .set_is_telemetry_enabled(!privacy_settings.is_telemetry_enabled, ctx);
                });
                ctx.notify();
            }
            AuthViewBodyAction::ToggleCrashReporting => {
                let privacy_settings_handle = PrivacySettings::handle(ctx);
                ctx.update_model(&privacy_settings_handle, |privacy_settings, ctx| {
                    privacy_settings.set_is_crash_reporting_enabled(
                        !privacy_settings.is_crash_reporting_enabled,
                        ctx,
                    );
                });
                ctx.notify();
            }
            AuthViewBodyAction::ToggleCloudConversationStorage => {
                let privacy_settings_handle = PrivacySettings::handle(ctx);
                ctx.update_model(&privacy_settings_handle, |privacy_settings, ctx| {
                    privacy_settings.set_is_cloud_conversation_storage_enabled(
                        !privacy_settings.is_cloud_conversation_storage_enabled,
                        ctx,
                    );
                });
                ctx.notify();
            }
            AuthViewBodyAction::Close => {
                ctx.emit(AuthViewBodyEvent::Close);
            }
        }
    }
}

impl View for AuthViewBody {
    fn ui_name() -> &'static str {
        "AuthViewBody"
    }

    fn accessibility_contents(&self, _: &AppContext) -> Option<AccessibilityContent> {
        Some(AccessibilityContent::new(
            "Welcome to Cortex",
            "Press enter to open your browser to Sign Up or Sign In.",
            WarpA11yRole::HelpRole,
        ))
    }

    fn on_focus(&mut self, focus_ctx: &FocusContext, ctx: &mut ViewContext<Self>) {
        if focus_ctx.is_self_focused() {
            ctx.notify();
        }
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let ui_builder = UiBuilder::new(
            appearance.theme().clone(),
            appearance.ui_font_family(),
            COMMON_BODY_UI_FONT_SIZE,
            DEFAULT_COMMAND_PALETTE_FONT_SIZE,
            appearance.line_height_ratio(),
        );

        let is_anonymous = AuthStateProvider::as_ref(app)
            .get()
            .is_user_anonymous()
            .unwrap_or_default();

        let mut content = Flex::column().with_cross_axis_alignment(CrossAxisAlignment::Stretch);
        content = content.with_children(match self.auth_step {
            AuthStep::SelectAuthPathway => {
                self.render_select_auth_pathway_content(is_anonymous, appearance, &ui_builder, app)
            }
            AuthStep::BrowserOpen => self.render_browser_open_content(appearance, &ui_builder),
        });

        let content = content.finish();

        let mut stack = Stack::new();
        stack.add_child(
            Container::new(content)
                .with_background(appearance.theme().surface_1())
                .with_border(Border::all(1.).with_border_fill(appearance.theme().outline()))
                .with_corner_radius(CornerRadius::with_all(MODAL_CORNER_RADIUS))
                .with_uniform_padding(32.)
                .finish(),
        );

        if let Some(overlay) = &self.active_overlay {
            match overlay {
                AuthViewOverlay::PrivacySettings => {
                    // The `is_any_ai_enabled` helper also accounts for login /
                    // remote-session gating, so the cloud-conversation toggle
                    // hides whenever AI isn't effectively available.
                    let is_ai_enabled = AISettings::as_ref(app).is_any_ai_enabled(app);
                    stack.add_child(
                        Dismiss::new(render_overlay(
                            render_privacy_settings_overlay_body(
                                appearance,
                                app,
                                &self.privacy_settings_handles,
                                &self.privacy_settings_actions(),
                                is_ai_enabled,
                            ),
                            appearance,
                        ))
                        .on_dismiss(|ctx, _app| {
                            ctx.dispatch_typed_action(AuthViewBodyAction::HideOverlay)
                        })
                        .finish(),
                    );
                }
                AuthViewOverlay::OfflineInfo => {
                    stack.add_child(
                        Dismiss::new(render_overlay(
                            render_offline_info_overlay_body(
                                appearance,
                                self.privacy_settings_handles.close_button_mouse.clone(),
                                AuthViewBodyAction::HideOverlay,
                            ),
                            appearance,
                        ))
                        .on_dismiss(|ctx, _app| {
                            ctx.dispatch_typed_action(AuthViewBodyAction::HideOverlay)
                        })
                        .finish(),
                    );
                }
            }
        }

        stack.finish()
    }
}
