use std::sync::Arc;

use clap::Parser;
use tokio::task::JoinSet;

mod cli;
mod http_api;
mod state;

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    slogger::info!("Starting SCION router prototype");

    let cli = cli::Cli::parse();

    let state = Arc::new(state::AppState {});
    
    let mut set = JoinSet::<anyhow::Result<()>>::new();
    set.spawn(http_api::run_http(Arc::clone(&state), cli.http_listen));
    
    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(())) => {
                slogger::info!("task exited");
                break;
            }
            Ok(Err(e)) => {
                slogger::error!("task failed: {e}");
                return Err(e);
            }
            Err(e) => {
                return Err(anyhow::anyhow!(e).context("join error"));
            }
        }
    }

    Ok(())
}
