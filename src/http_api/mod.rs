use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use axum::{
    routing::get,
    Router,
};
use tracing::info;

use crate::state::AppState;
pub mod model;
pub mod health;
pub mod peers;

pub async fn run_http(state: Arc<AppState>, http_listen: SocketAddr) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/health", get(health::health))
        .route("/peers", get(peers::get_peers).post(peers::add_peer))
        .with_state(Arc::clone(&state));

    let listener = tokio::net::TcpListener::bind(http_listen)
        .await
        .with_context(|| format!("bind http {http_listen}"))?;

    info!(%http_listen, "http listening");
    axum::serve(listener, app)
        .await
        .context("http server failed")?;
    Ok(())
}

