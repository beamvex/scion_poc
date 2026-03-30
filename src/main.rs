use std::{collections::HashMap, sync::Arc};

use clap::Parser;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tracing::{error, info};

mod beacon;
mod cli;
mod dataplane;
mod http_api;
mod scion;
mod state;

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli = cli::Cli::parse();

    let state = state::AppState {
        rt: Arc::new(RwLock::new(state::RoutingTable::default())),
        ifaces: Arc::new(RwLock::new(HashMap::new())),
        iface_socks: Arc::new(RwLock::new(HashMap::new())),
        beacon_peers: Arc::new(RwLock::new(Vec::new())),
        metrics: Arc::new(state::Metrics::default()),
    };

    let mut set = JoinSet::<anyhow::Result<()>>::new();
    set.spawn(http_api::run_http(state.clone(), cli.http_listen));
    set.spawn(dataplane::run_dataplane(state.clone(), cli.data_listen));
    set.spawn(beacon::run_beacon_rx(state.clone(), cli.beacon_listen));
    set.spawn(beacon::run_beacon_tx(state.clone(), cli.beacon_interval_secs));

    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(())) => {
                info!("task exited");
                break;
            }
            Ok(Err(e)) => {
                error!(error=%e, "task failed");
                return Err(e);
            }
            Err(e) => {
                return Err(anyhow::anyhow!(e).context("join error"));
            }
        }
    }

    Ok(())
}
