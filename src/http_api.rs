use std::{collections::HashMap, net::SocketAddr};

use anyhow::Context;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use tower_http::trace::TraceLayer;
use tracing::{error, info};

use crate::{
    beacon::BeaconPeer,
    state::{upsert_iface, AppState, IfaceEntry, RouteEntry},
};

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn get_routes(State(state): State<AppState>) -> Json<HashMap<String, RouteEntry>> {
    let rt = state.rt.read().await;
    Json(rt.routes.clone())
}

async fn get_ifaces(State(state): State<AppState>) -> Json<HashMap<u16, IfaceEntry>> {
    let ifaces = state.ifaces.read().await;
    Json(ifaces.clone())
}

async fn get_beacon_peers(State(state): State<AppState>) -> Json<Vec<SocketAddr>> {
    let peers = state.beacon_peers.read().await;
    Json(peers.clone())
}

async fn post_beacon_peer(State(state): State<AppState>, Json(peer): Json<BeaconPeer>) -> StatusCode {
    let mut peers = state.beacon_peers.write().await;
    if !peers.contains(&peer.peer) {
        peers.push(peer.peer);
    }
    StatusCode::NO_CONTENT
}

async fn put_route(
    Path(dst): Path<String>,
    State(state): State<AppState>,
    Json(entry): Json<RouteEntry>,
) -> StatusCode {
    let mut rt = state.rt.write().await;
    rt.upsert(dst, entry);
    StatusCode::NO_CONTENT
}

async fn delete_route(Path(dst): Path<String>, State(state): State<AppState>) -> StatusCode {
    let mut rt = state.rt.write().await;
    if rt.remove(&dst) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn put_iface(
    Path(ifid): Path<u16>,
    State(state): State<AppState>,
    Json(entry): Json<IfaceEntry>,
) -> StatusCode {
    match upsert_iface(&state, ifid, entry).await {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(e) => {
            error!(ifid, error=%e, "upsert iface failed");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn delete_iface(Path(ifid): Path<u16>, State(state): State<AppState>) -> StatusCode {
    let removed = {
        let mut ifaces = state.ifaces.write().await;
        ifaces.remove(&ifid)
    };
    if removed.is_some() {
        let mut socks = state.iface_socks.write().await;
        socks.remove(&ifid);
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

pub async fn run_http(state: AppState, http_listen: SocketAddr) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/routes", get(get_routes))
        .route("/routes/:dst", post(put_route).delete(delete_route))
        .route("/ifaces", get(get_ifaces))
        .route("/ifaces/:ifid", post(put_iface).delete(delete_iface))
        .route("/beacon/peers", get(get_beacon_peers).post(post_beacon_peer))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(http_listen)
        .await
        .with_context(|| format!("bind http {http_listen}"))?;

    info!(%http_listen, "http listening");
    axum::serve(listener, app)
        .await
        .context("http server failed")?;
    Ok(())
}
