use std::time::Duration;

use instant::Instant;
use warpui::{Entity, SingletonEntity};

/// Process-wide monotonic phase source for tab-row animations.
///
/// All tab-row animations read phase from this single clock so they stay
/// phase-locked across rows: e.g., the comet pair on every tab running the
/// Running animation rotates in lockstep. New tabs that join mid-animation
/// pick up whatever phase the clock currently has — they don't restart at
/// zero.
#[derive(Debug)]
pub struct AnimationClock {
    origin: Instant,
}

impl AnimationClock {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }

    /// Returns a phase in `[0.0, 1.0)` for an animation of the given period.
    /// A zero or negative period returns 0.0.
    pub fn phase(&self, period: Duration) -> f32 {
        let p = period.as_secs_f32();
        if p <= 0.0 {
            return 0.0;
        }
        let elapsed = self.origin.elapsed().as_secs_f32();
        (elapsed % p) / p
    }
}

impl Default for AnimationClock {
    fn default() -> Self {
        Self::new()
    }
}

pub enum AnimationClockEvent {}

impl SingletonEntity for AnimationClock {}

impl Entity for AnimationClock {
    type Event = AnimationClockEvent;
}
