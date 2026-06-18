use std::sync::Arc;

use crate::terminal::event::Event as TerminalEvent;
use async_channel::Sender;

/// A wrapper struct that emits events which originate from the PTY event loop.
/// Instead of passing individual senders, we can pass through this struct
/// so that users have access to all of the senders in one nicely wrapped struct.
#[derive(Clone)]
pub struct ChannelEventListener {
    /// We have a dedicated channel for "wakeup"s because we throttle the receiver
    /// so that we can coalesce successive wakeup events during situations of high
    /// throughput (e.g. running `yes`).
    wakeups_tx: Sender<()>,
    terminal_events_tx: Sender<TerminalEvent>,
    pty_reads_tx: async_broadcast::Sender<Arc<Vec<u8>>>,
}

impl ChannelEventListener {
    pub fn new(
        wakeups_tx: Sender<()>,
        terminal_events_tx: Sender<TerminalEvent>,
        pty_reads_tx: async_broadcast::Sender<Arc<Vec<u8>>>,
    ) -> Self {
        ChannelEventListener {
            wakeups_tx,
            terminal_events_tx,
            pty_reads_tx,
        }
    }

    #[cfg(any(test, feature = "integration_tests"))]
    pub fn are_any_events_pending(&self) -> bool {
        !self.wakeups_tx.is_empty()
            || !self.terminal_events_tx.is_empty()
            || !self.pty_reads_tx.is_empty()
    }

    pub fn send_wakeup_event(&self) {
        if let Err(e) = self.wakeups_tx.try_send(()) {
            log::warn!("Failed to send Wakeup event: {e:?}");
        }
    }

    pub fn send_terminal_event(&self, event: TerminalEvent) {
        if let Err(e) = self.terminal_events_tx.try_send(event) {
            let try_send_error_dbg = format!("{e:?}");
            log::warn!(
                "Failed to send Terminal event {:?}: {:?}",
                e.into_inner(),
                try_send_error_dbg
            );
        }
    }

    pub fn send_handler_event(&self, event: HandlerEvent) {
        if let Err(e) = self
            .terminal_events_tx
            .try_send(TerminalEvent::Handler(event))
        {
            log::warn!("Failed to send Terminal Handler event {e:?}");
        }
    }

    pub fn send_pty_read_event(&self, bytes: &[u8]) {
        // Don't bother sending the event if there aren't any
        // active receivers. This avoids an unnecessary allocation of the bytes vector.
        // Note that we don't simply close the sending side since receivers
        // might come alive at some point in the future.
        if self.pty_reads_tx.receiver_count() > 0 {
            if let Err(e) = self.pty_reads_tx.try_broadcast(Arc::new(bytes.to_vec())) {
                log::warn!("Failed to send pty read event: {e:?}");
            }
        }
    }

    // CORTEX-BEGIN: mobile-bridge-pty-subscribe
    /// Cortex: mint a fresh, active receiver on the PTY-reads broadcast.
    ///
    /// Used by the mobile companion bridge (`app/src/mobile_bridge/`) to mirror
    /// a pane's live output to a phone. The new receiver only observes chunks
    /// broadcast *after* this call, so it pairs cleanly with a screen snapshot
    /// taken under the same model lock — no gap, no overlap. Creating it also
    /// bumps `receiver_count()`, which is the gate `send_pty_read_event` checks
    /// before broadcasting, so output flows only while a pane is being mirrored.
    pub fn cortex_new_pty_reads_receiver(&self) -> async_broadcast::Receiver<Arc<Vec<u8>>> {
        self.pty_reads_tx.new_receiver()
    }
    // CORTEX-END: mobile-bridge-pty-subscribe
}

#[cfg(test)]
mod testing;

use crate::terminal::model::terminal_model::HandlerEvent;

#[cfg(test)]
pub use testing::*;
