//! Background-executor side of the hook-bridge IPC. Pushes incoming
//! envelopes onto the bridge's main-thread channel; the bridge drains
//! that channel and updates the singleton's counters via
//! `spawn_stream_local`. Mirrors the pattern in
//! `app/src/orchestrate/service_impl.rs`.

use async_channel::Sender;
use async_trait::async_trait;
use futures::channel::oneshot;

use super::service::{HookAck, HookEmitService, HookEnvelope};

pub(super) type HookEmitJob = (HookEnvelope, oneshot::Sender<HookAck>);

#[derive(Clone)]
pub(super) struct HookEmitServiceImpl {
    tx: Sender<HookEmitJob>,
}

impl HookEmitServiceImpl {
    pub(super) fn new(tx: Sender<HookEmitJob>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl ipc::ServiceImpl for HookEmitServiceImpl {
    type Service = HookEmitService;

    async fn handle_request(&self, envelope: HookEnvelope) -> HookAck {
        let (reply_tx, reply_rx) = oneshot::channel();
        if let Err(err) = self.tx.send((envelope, reply_tx)).await {
            return HookAck {
                accepted: false,
                error: Some(format!("hook bridge channel closed: {err}")),
            };
        }
        match reply_rx.await {
            Ok(ack) => ack,
            Err(_) => HookAck {
                accepted: false,
                error: Some("hook bridge dropped envelope before reply".to_string()),
            },
        }
    }
}
