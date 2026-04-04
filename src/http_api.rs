use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use anyhow::Context;
use axum::{
    http::StatusCode,
    routing::get,
    Router,
};
use tracing::info;

use crate::state::AppState;

async fn health() -> StatusCode {
    StatusCode::OK
}

pub async fn run_http(state: Arc<AppState>, http_listen: SocketAddr) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/health", get(health))
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
