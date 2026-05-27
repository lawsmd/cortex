//! Cortex-only: receive-side watchdog for the external-status hook bridge.
//!
//! Background. The CLI-agent status pipeline (Tier 1) depends on `cortex-hook.{sh,ps1}`
//! emitting OSC 777 / IPC events as Claude Code fires its hook lifecycle.
//! Both transports today are coupled to subprocess details outside our
//! control (controlling-tty inheritance on Unix, AttachConsole/CONOUT$ on
//! Windows). On 2026-05-19 a Claude Code release silently broke the Unix
//! path; the regression ran undetected for 8 days because the fallback
//! tiers (title heuristic, block state) kept the UI looking "mostly fine."
//!
//! This module fixes the *detection* gap: it doesn't matter how the bytes
//! reach Cortex (OSC, FIFO, IPC, carrier pigeon) — the watchdog reads the
//! pulse on the *receive* side. Each `PromptSubmit` event arms a timestamp
//! on [`super::CLIAgentSessionsModel::prompt_submit_at`]; the matching
//! `Stop` disarms it. A periodic sweep checks for entries older than the
//! "missed Stop" threshold and bumps the [`BridgeHealthMonitor`]'s counters
//! accordingly. When the consecutive-miss count crosses the [`Down`] line,
//! a `BRIDGE_STALE` WARN is emitted to `warp-oss.log` and the Cortex
//! Settings → Diagnostics page surfaces the state in the UI.
//!
//! Architecture and rationale: `docs/ai/external-status-injection.md`
//! (the section will become "Layer A2" when Phase 2 of the big-move plan
//! ships the IPC-transport replacement; this module is Phase 1).
//!
//! [`Down`]: BridgeState::Down

use std::time::{Duration, Instant};

use futures::stream::AbortHandle;
use warpui::r#async::Timer;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

use super::CLIAgentSessionsModel;

/// How often the watchdog sweeps the armed-PromptSubmit map. Short enough
/// that the Settings → Diagnostics page feels live, long enough that the
/// cost is negligible.
const SWEEP_INTERVAL: Duration = Duration::from_secs(5);

/// How long an armed `PromptSubmit` can sit without a matching `Stop`
/// before the watchdog counts it as a miss. Conservative — long Claude
/// turns (refactors, plan-mode exploration) can run several minutes, and
/// we don't want false positives. The 8-day Layer A regression was visible
/// at any threshold under "hours," so 5 minutes is plenty fast.
const MISSED_STOP_THRESHOLD: Duration = Duration::from_secs(5 * 60);

/// Bridge health tiers surfaced in the Settings → Diagnostics page.
///
/// State transitions (recovery is symmetric — a single fresh `Stop` resets
/// `consecutive_misses` and walks the state back down):
///
/// | Consecutive misses | State        |
/// |--------------------|--------------|
/// | 0                  | `Healthy`    |
/// | 1–2                | `Degraded`   |
/// | ≥3                 | `Down`       |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeState {
    Healthy,
    Degraded,
    Down,
}

impl BridgeState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Healthy => "Healthy",
            Self::Degraded => "Degraded",
            Self::Down => "Down",
        }
    }
}

/// Singleton that mirrors the model's `prompt_submit_at` map on a timer
/// and translates "stale armed entry" into a per-session miss.
///
/// Per-session deduplication: the same `(view_id, prompt_submit_at)` pair
/// must only count once, no matter how many sweep ticks observe it. We
/// track that via the `missed` set, which is reset by any subsequent
/// successful Stop on the same view.
pub struct BridgeHealthMonitor {
    last_event_at: Option<Instant>,
    events_received_this_session: u64,
    missed_stops: u64,
    consecutive_misses: u32,
    bridge_state: BridgeState,
    /// (view_id, prompt_submit_instant) pairs that have already been counted
    /// as misses on a prior sweep. Entries are pruned when the corresponding
    /// armed entry disappears from `CLIAgentSessionsModel` (i.e. when Stop
    /// finally arrives, or the view is destroyed) so a future stale entry on
    /// the same view counts again.
    counted_misses: std::collections::HashSet<(warpui::EntityId, Instant)>,
    sweep_abort_handle: Option<AbortHandle>,
}

impl BridgeHealthMonitor {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let mut me = Self {
            last_event_at: None,
            events_received_this_session: 0,
            missed_stops: 0,
            consecutive_misses: 0,
            bridge_state: BridgeState::Healthy,
            counted_misses: std::collections::HashSet::new(),
            sweep_abort_handle: None,
        };
        me.schedule_sweep(ctx);
        me
    }

    pub fn bridge_state(&self) -> BridgeState {
        self.bridge_state
    }

    pub fn last_event_at(&self) -> Option<Instant> {
        self.last_event_at
    }

    pub fn events_received_this_session(&self) -> u64 {
        self.events_received_this_session
    }

    pub fn missed_stops(&self) -> u64 {
        self.missed_stops
    }

    pub fn consecutive_misses(&self) -> u32 {
        self.consecutive_misses
    }

    /// Called from `CLIAgentSessionsModel::update_from_event` whenever any
    /// CLI-agent event lands. Bumps the receive counter and resets the
    /// "Down → Degraded → Healthy" walk on a confirmed Stop.
    pub fn record_event(
        &mut self,
        is_stop: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        self.last_event_at = Some(Instant::now());
        self.events_received_this_session = self.events_received_this_session.saturating_add(1);
        if is_stop {
            self.consecutive_misses = 0;
            self.update_state(ctx);
        }
        ctx.notify();
    }

    /// Manual sweep — wired to the "Test bridge" button on Settings →
    /// Diagnostics so the user can force a state refresh without waiting
    /// for the next periodic tick.
    pub fn sweep_now(&mut self, ctx: &mut ModelContext<Self>) {
        self.sweep(ctx);
    }

    fn schedule_sweep(&mut self, ctx: &mut ModelContext<Self>) {
        if let Some(prior) = self.sweep_abort_handle.take() {
            prior.abort();
        }
        let handle = ctx.spawn(
            async move { Timer::after(SWEEP_INTERVAL).await },
            |me, _, ctx| {
                me.sweep(ctx);
                me.schedule_sweep(ctx);
            },
        );
        self.sweep_abort_handle = Some(handle.abort_handle());
    }

    fn sweep(&mut self, ctx: &mut ModelContext<Self>) {
        let now = Instant::now();
        let armed = collect_armed(ctx);

        // Prune `counted_misses` of entries whose armed-state has cleared
        // (the matching Stop arrived, or the view was destroyed). Without
        // this, a long-running session that repeatedly emits PromptSubmit
        // → stale → Stop would never tally a second miss because the prior
        // (view_id, instant) tuple persisted.
        self.counted_misses
            .retain(|(view_id, at)| armed.iter().any(|(v, a)| v == view_id && a == at));

        let mut new_misses = 0u64;
        for (view_id, at) in &armed {
            if now.duration_since(*at) < MISSED_STOP_THRESHOLD {
                continue;
            }
            if self.counted_misses.insert((*view_id, *at)) {
                new_misses += 1;
            }
        }

        if new_misses > 0 {
            self.missed_stops = self.missed_stops.saturating_add(new_misses);
            self.consecutive_misses = self.consecutive_misses.saturating_add(new_misses as u32);
        }

        self.update_state(ctx);
        ctx.notify();
    }

    fn update_state(&mut self, ctx: &mut ModelContext<Self>) {
        let new_state = match self.consecutive_misses {
            0 => BridgeState::Healthy,
            1..=2 => BridgeState::Degraded,
            _ => BridgeState::Down,
        };
        if new_state != self.bridge_state {
            let prev = self.bridge_state;
            self.bridge_state = new_state;
            if matches!(new_state, BridgeState::Down) {
                log::warn!(
                    target: "bridge_health",
                    "BRIDGE_STALE: {} missed stops, {} consecutive — Tier 1 bridge appears to have stopped delivering events. See docs/ai/external-status-injection.md and ~/.claude/cortex-hook.log",
                    self.missed_stops,
                    self.consecutive_misses,
                );
            } else if matches!(prev, BridgeState::Down) {
                log::info!(
                    target: "bridge_health",
                    "BRIDGE_RECOVERED: state {} → {}",
                    prev.label(),
                    new_state.label(),
                );
            }
            ctx.emit(BridgeHealthEvent::StateChanged {
                previous: prev,
                current: new_state,
            });
        }
    }
}

/// Helper that snapshots the model's armed map without holding a borrow
/// across the `update` boundary.
fn collect_armed(ctx: &mut ModelContext<BridgeHealthMonitor>) -> Vec<(warpui::EntityId, Instant)> {
    CLIAgentSessionsModel::handle(ctx)
        .as_ref(ctx)
        .armed_prompt_submits()
        .collect()
}

/// Event published by [`BridgeHealthMonitor`]. Currently only emitted on
/// state transitions; future subscribers (toast banner, telemetry) can
/// consume `previous`/`current` to react to transitions specifically.
pub enum BridgeHealthEvent {
    StateChanged {
        #[allow(dead_code)]
        previous: BridgeState,
        #[allow(dead_code)]
        current: BridgeState,
    },
}

impl Entity for BridgeHealthMonitor {
    type Event = BridgeHealthEvent;
}

impl SingletonEntity for BridgeHealthMonitor {}

/// Convenience accessor for the diagnostics UI.
pub fn snapshot(app: &AppContext) -> BridgeHealthSnapshot {
    let monitor = BridgeHealthMonitor::as_ref(app);
    BridgeHealthSnapshot {
        bridge_state: monitor.bridge_state(),
        last_event_at: monitor.last_event_at(),
        events_received_this_session: monitor.events_received_this_session(),
        missed_stops: monitor.missed_stops(),
        consecutive_misses: monitor.consecutive_misses(),
    }
}

/// Plain-data view of the monitor's state for rendering. Copied out so the
/// settings page can render without holding a model borrow.
#[derive(Debug, Clone)]
pub struct BridgeHealthSnapshot {
    pub bridge_state: BridgeState,
    pub last_event_at: Option<Instant>,
    pub events_received_this_session: u64,
    pub missed_stops: u64,
    pub consecutive_misses: u32,
}
