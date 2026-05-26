//! Client side of the `cortex orchestrate` subcommand.
//!
//! Reads [`CORTEX_IPC_SOCKET_ENV`](super::CORTEX_IPC_SOCKET_ENV) from the
//! environment — populated by the Cortex app on every spawned terminal
//! (Phase 4) — connects to the running [`OrchestrateService`] via
//! `ipc::Client`, and sends an [`OrchestrateRequest`] derived from the
//! CLI args. Prints one pane id per line on success, or returns an
//! error suitable for the CLI dispatch to surface to the user.

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use ipc::{Client, ConnectionAddress};
use warp_cli::orchestrate::OrchestrateArgs;
use warpui::r#async::executor::Background;

use super::{OrchestrateRequest, OrchestrateService, CORTEX_IPC_SOCKET_ENV};

pub fn run_cli(args: &OrchestrateArgs) -> Result<()> {
    if args.panes == 0 {
        return Err(anyhow!("--panes must be >= 1; got 0"));
    }

    let socket_path = std::env::var(CORTEX_IPC_SOCKET_ENV).with_context(|| {
        format!(
            "{CORTEX_IPC_SOCKET_ENV} is not set. `cortex orchestrate` must be invoked \
             from inside a terminal spawned by the Cortex app."
        )
    })?;

    let plan_file = if args.plan_file.is_absolute() {
        args.plan_file.clone()
    } else {
        std::env::current_dir()
            .context("failed to resolve current_dir to absolutize --plan-file")?
            .join(&args.plan_file)
    };

    let request = OrchestrateRequest {
        plan_file,
        panes: args.panes,
        working_dir: args.working_dir.clone(),
    };

    let executor = Arc::new(Background::new(1, |_| {
        "cortex-orchestrate-cli".to_owned()
    }));
    let response = warpui::r#async::block_on(async {
        let client = Client::connect(
            ConnectionAddress::from(socket_path.clone()),
            executor.clone(),
        )
        .await
        .with_context(|| {
            format!("failed to connect to orchestrate IPC socket at {socket_path}")
        })?;
        let caller = ipc::service_caller::<OrchestrateService>(Arc::new(client));
        caller
            .call(request)
            .await
            .context("OrchestrateService call failed")
    })?;

    if let Some(error) = response.error {
        return Err(anyhow!("OrchestrateService returned error: {error}"));
    }

    for id in &response.pane_ids {
        println!("{id}");
    }
    Ok(())
}
