//! Cortex-only: the per-pane "smart clear" button.
//!
//! One button, two behaviors:
//! - A CLI agent with a known clear command (currently Claude Code) is running
//!   in the pane → submit that command (`/clear`) to the agent's PTY, exactly
//!   as if the user typed it.
//! - Otherwise → clear the pane's blocks, same as the "Clear Blocks" action
//!   (Cmd/Ctrl-Shift-K).
//!
//! The button renders in two mutually exclusive placements:
//! - In the pane header, left of the overflow/close buttons, whenever the
//!   header renders (split panes, shared sessions, fullscreen agent view).
//! - As a floating overlay in the pane's top-right corner when the header is
//!   hidden (the default single-pane layout), following the find bar's
//!   `Align::top_right` pattern.

use warp_core::ui::theme::Fill;
use warpui::elements::{Align, ConstrainedBox, Container};
use warpui::ui_components::components::UiComponent;
use warpui::{AppContext, Element, SingletonEntity, ViewContext};

use crate::appearance::Appearance;
use crate::pane_group::pane::view::PaneHeaderAction;
use crate::pane_group::BackingView;
use crate::terminal::cli_agent_sessions::CLIAgentSessionsModel;
use crate::terminal::CLIAgent;
use crate::ui_components::buttons::icon_button_with_color;
use crate::ui_components::icons::Icon;

use super::{TerminalAction, TerminalView};

/// The slash command that clears a CLI agent's session context, per agent.
/// Agents without a mapping fall back to clearing the pane's blocks; future
/// agents (Gemini, Codex, …) are one match arm each.
fn cortex_agent_clear_command(agent: CLIAgent) -> Option<&'static str> {
    match agent {
        CLIAgent::Claude => Some("/clear"),
        _ => None,
    }
}

impl TerminalView {
    /// The CLI agent currently running in this pane, if its foreground block
    /// is still active. The session entry can outlive the agent's block (same
    /// caveat as the dropped-image paste path in `use_agent_footer`), so
    /// require both before treating the pane as agent-controlled.
    fn cortex_active_cli_agent(&self, app: &AppContext) -> Option<CLIAgent> {
        let agent = CLIAgentSessionsModel::as_ref(app)
            .session(self.view_id)?
            .agent;
        let long_running = self
            .model
            .lock()
            .block_list()
            .active_block()
            .is_active_and_long_running();
        long_running.then_some(agent)
    }

    /// Clears the pane: submits the agent's clear command when a CLI agent
    /// with a known one is running, otherwise clears the pane's blocks.
    pub(crate) fn cortex_smart_clear(&mut self, ctx: &mut ViewContext<Self>) {
        #[cfg(feature = "local_tty")]
        if let Some(command) = self
            .cortex_active_cli_agent(ctx)
            .and_then(cortex_agent_clear_command)
        {
            self.submit_text_to_cli_agent_pty(command.to_owned(), ctx);
            return;
        }
        self.clear_buffer(ctx);
    }

    fn cortex_clear_tooltip_text(&self, app: &AppContext) -> String {
        match self
            .cortex_active_cli_agent(app)
            .and_then(cortex_agent_clear_command)
        {
            Some(command) => format!("Clear agent session ({command})"),
            None => "Clear blocks".to_owned(),
        }
    }

    /// The smart-clear button for the pane header, rendered left of the
    /// overflow/close buttons. Routed through `PaneHeaderAction::CustomAction`
    /// like the neighboring cancel/details buttons in `pane_impl`.
    pub(crate) fn render_cortex_clear_header_button(
        &self,
        icon_color: Option<Fill>,
        button_size: Option<f32>,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let color = icon_color.unwrap_or_else(|| theme.sub_text_color(theme.background()));
        let ui_builder = appearance.ui_builder().clone();
        let tooltip_text = self.cortex_clear_tooltip_text(app);

        let button = icon_button_with_color(
            appearance,
            Icon::SlashCircle,
            false, /* active */
            self.mouse_states.cortex_clear_header_button.clone(),
            color,
        )
        .with_tooltip(move || ui_builder.tool_tip(tooltip_text.clone()).build().finish())
        .build()
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action::<PaneHeaderAction<TerminalAction, TerminalAction>>(
                PaneHeaderAction::CustomAction(TerminalAction::CortexClearPane),
            );
        })
        .finish();

        if let Some(size) = button_size {
            ConstrainedBox::new(button)
                .with_width(size)
                .with_height(size)
                .finish()
        } else {
            button
        }
    }

    /// Whether the floating placement should render: only when the pane
    /// header (which hosts the button otherwise) is hidden, and the find bar
    /// isn't occupying the same top-right corner.
    pub(crate) fn cortex_should_show_clear_overlay(&self, app: &AppContext) -> bool {
        !BackingView::should_render_header(self, app)
            && !self.find_model.as_ref(app).is_find_bar_open()
    }

    /// The smart-clear button floating in the pane's top-right corner, for
    /// panes whose header doesn't render (the default single-pane layout).
    /// Dispatches `TerminalAction` directly since it lives in the terminal
    /// view's own element tree rather than the pane header's.
    pub(crate) fn render_cortex_clear_overlay(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let ui_builder = appearance.ui_builder().clone();
        let tooltip_text = self.cortex_clear_tooltip_text(app);

        let button = icon_button_with_color(
            appearance,
            Icon::SlashCircle,
            false, /* active */
            self.mouse_states.cortex_clear_overlay_button.clone(),
            theme.sub_text_color(theme.background()),
        )
        .with_tooltip(move || ui_builder.tool_tip(tooltip_text.clone()).build().finish())
        .build()
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(TerminalAction::CortexClearPane);
        })
        .finish();

        Align::new(
            Container::new(button)
                .with_padding_top(8.)
                .with_padding_right(16.)
                .finish(),
        )
        .top_right()
        .finish()
    }
}
