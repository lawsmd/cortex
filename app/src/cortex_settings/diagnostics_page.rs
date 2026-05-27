//! Content rendered on the right side of the Cortex Settings pane when the
//! "Diagnostics" section is selected.
//!
//! Surfaces the live state of the external-status hook bridge — the Tier 1
//! pipeline that translates Claude's hook events into Cortex's CLI-agent
//! status (`docs/ai/external-status-injection.md`). Each row reads from
//! [`crate::terminal::cli_agent_sessions::bridge_health::BridgeHealthMonitor`]
//! via a plain-data snapshot taken at render time, so the page stays
//! decoupled from the watchdog's internals.
//!
//! On wasm targets the watchdog isn't compiled (no `cortex-hook.sh` to
//! monitor), so this page renders a static "Unavailable" note instead of
//! the live health rows.

use warpui::{
    elements::{
        Align, Container, CrossAxisAlignment, Element, Flex, MouseStateHandle, Padding,
        ParentElement, Shrinkable,
    },
    ui_components::{
        button::ButtonVariant,
        components::{Coords, UiComponent, UiComponentStyles},
    },
    AppContext,
};

use crate::appearance::Appearance;
use crate::cortex_settings::action::CortexSettingsAction;

const ROW_VERTICAL_PADDING: f32 = 6.0;
const VALUE_RIGHT_PADDING: f32 = 5.0;

#[derive(Default)]
pub struct DiagnosticsPageState {
    test_bridge_button: MouseStateHandle,
}

pub fn diagnostics_page_search_terms() -> &'static [&'static str] {
    &[
        "diagnostics",
        "diagnostic",
        "bridge",
        "health",
        "hook",
        "claude",
        "cli agent",
        "tier",
        "osc",
        "ipc",
        "test bridge",
        "external status",
        "missed",
        "stop",
    ]
}

pub fn render_diagnostics_page(
    state: &DiagnosticsPageState,
    appearance: &Appearance,
    app: &AppContext,
) -> Box<dyn Element> {
    #[cfg(target_family = "wasm")]
    {
        let _ = (state, app);
        return render_unavailable_row(appearance);
    }

    #[cfg(not(target_family = "wasm"))]
    {
        use crate::hook_bridge::snapshot as hook_bridge_snapshot;
        use crate::terminal::cli_agent_sessions::bridge_health::snapshot;

        let snap = snapshot(app);
        let ipc_snap = hook_bridge_snapshot(app);

        Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(render_text_row(
                appearance,
                "External status bridge",
                snap.bridge_state.label(),
            ))
            .with_child(render_text_row(
                appearance,
                "Last event (OSC)",
                &format_last_event(snap.last_event_at),
            ))
            .with_child(render_text_row(
                appearance,
                "OSC events received this session",
                &snap.events_received_this_session.to_string(),
            ))
            .with_child(render_text_row(
                appearance,
                "Missed stops (cumulative)",
                &snap.missed_stops.to_string(),
            ))
            .with_child(render_text_row(
                appearance,
                "Consecutive misses",
                &snap.consecutive_misses.to_string(),
            ))
            .with_child(render_text_row(
                appearance,
                "IPC socket bound",
                if ipc_snap.socket_bound { "yes" } else { "no" },
            ))
            .with_child(render_text_row(
                appearance,
                "IPC events received this session (shadow)",
                &ipc_snap.events_received.to_string(),
            ))
            .with_child(render_text_row(
                appearance,
                "Last envelope (IPC)",
                &format_last_event(ipc_snap.last_envelope_at),
            ))
            .with_child(render_text_row(
                appearance,
                "IPC decode errors",
                &ipc_snap.decode_errors.to_string(),
            ))
            .with_child(render_test_bridge_row(state, appearance))
            .with_child(render_text_row(
                appearance,
                "Forensic log",
                "~/.claude/cortex-hook.log",
            ))
            .finish()
    }
}

#[cfg(target_family = "wasm")]
fn render_unavailable_row(appearance: &Appearance) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();
    let label = ui_builder
        .span("Bridge diagnostics are unavailable on this build.".to_string())
        .build()
        .finish();
    Container::new(label)
        .with_padding(Padding::uniform(ROW_VERTICAL_PADDING))
        .finish()
}

fn render_text_row(
    appearance: &Appearance,
    label_text: &str,
    value_text: &str,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();

    let label = ui_builder
        .span(label_text.to_string())
        .build()
        .finish();

    let value = ui_builder
        .span(value_text.to_string())
        .build()
        .finish();

    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label).left().finish()).finish(),
    )
    .finish();

    let value_container = Container::new(value)
        .with_padding_right(VALUE_RIGHT_PADDING)
        .finish();

    let row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(header)
        .with_child(value_container)
        .finish();

    Container::new(row)
        .with_padding(Padding::uniform(ROW_VERTICAL_PADDING))
        .finish()
}

fn render_test_bridge_row(
    state: &DiagnosticsPageState,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let ui_builder = appearance.ui_builder();

    let label = ui_builder
        .span("Force a watchdog sweep now".to_string())
        .build()
        .finish();

    let button = ui_builder
        .button(ButtonVariant::Accent, state.test_bridge_button.clone())
        .with_text_label("Test bridge".to_string())
        .with_style(
            UiComponentStyles::default()
                .set_padding(Coords::uniform(8.0)),
        )
        .build()
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(CortexSettingsAction::TriggerBridgeHealthSweep);
        })
        .finish();

    let header = Shrinkable::new(
        1.0,
        Container::new(Align::new(label).left().finish()).finish(),
    )
    .finish();

    let control = Container::new(button)
        .with_padding_right(VALUE_RIGHT_PADDING)
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

#[cfg(not(target_family = "wasm"))]
fn format_last_event(at: Option<std::time::Instant>) -> String {
    match at {
        None => "never".to_string(),
        Some(t) => {
            let elapsed = t.elapsed();
            let secs = elapsed.as_secs();
            if secs < 60 {
                format!("{}s ago", secs)
            } else if secs < 3600 {
                format!("{}m ago", secs / 60)
            } else {
                format!("{}h ago", secs / 3600)
            }
        }
    }
}
