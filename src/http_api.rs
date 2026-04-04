use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Router,
};
use tracing::info;

use crate::state::AppState;

use serde::Serialize;

#[derive(Serialize)]
struct Response {
    message: String,
}

async fn health(State(_state): State<Arc<AppState>>) -> (StatusCode, axum::Json<Response>) {
    (StatusCode::OK, axum::Json(Response { message: "OK".to_string() }))
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
