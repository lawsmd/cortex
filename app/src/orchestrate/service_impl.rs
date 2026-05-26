//! Background-executor side of the `OrchestrateService` IPC.
//!
//! `handle_request` runs on `ipc::Server`'s worker pool. It cannot touch
//! UI state directly, so it hands the request (plus a one-shot reply
//! channel) to an `async_channel` that the bridge's main-thread stream
//! drains.

use async_channel::Sender;
use async_trait::async_trait;
use futures::channel::oneshot;

use super::service::{OrchestrateRequest, OrchestrateResponse, OrchestrateService};

/// One unit of work pushed onto the bridge's main-thread channel: the
/// incoming request and the one-shot reply slot the IPC handler is
/// awaiting on.
pub(super) type OrchestrateJob = (
    OrchestrateRequest,
    oneshot::Sender<OrchestrateResponse>,
);

#[derive(Clone)]
pub(super) struct OrchestrateServiceImpl {
    tx: Sender<OrchestrateJob>,
}

impl OrchestrateServiceImpl {
    pub(super) fn new(tx: Sender<OrchestrateJob>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl ipc::ServiceImpl for OrchestrateServiceImpl {
    type Service = OrchestrateService;

    async fn handle_request(&self, request: OrchestrateRequest) -> OrchestrateResponse {
        log::info!(
            "OrchestrateService received request: plan_file={:?}, panes={}",
            request.plan_file,
            request.panes
        );
        let (reply_tx, reply_rx) = oneshot::channel();
        if let Err(err) = self.tx.send((request, reply_tx)).await {
            return OrchestrateResponse {
                pane_ids: Vec::new(),
                error: Some(format!(
                    "Cortex orchestrate bridge is not running: {err}"
                )),
            };
        }
        match reply_rx.await {
            Ok(response) => response,
            Err(_) => OrchestrateResponse {
                pane_ids: Vec::new(),
                error: Some(
                    "Cortex orchestrate bridge dropped the request before replying.".to_string(),
                ),
            },
        }
    }
}
