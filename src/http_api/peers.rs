use axum::http::StatusCode;
use axum::extract::State;
use axum::Json;
use std::sync::Arc;

use crate::state::AppState;
use crate::http_api::model::{Peer, PeerRequest, PeersResponse};

pub async fn get_peers(state: State<Arc<AppState>>) -> (StatusCode, Json<PeersResponse>) {

    let peers =state.peers.read().await;
    let peers = peers.keys().cloned().collect();

    (StatusCode::OK, Json(PeersResponse { peers }))
}

pub async fn add_peer(
    state: State<Arc<AppState>>,
    Json(req): Json<PeerRequest>,
) -> (StatusCode, Json<()>) {
    state
        .peers
        .write()
        .await
        .insert(req.peer.name.clone(), req.peer.address);
    (StatusCode::CREATED, Json(()))
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_get_peers_empty() {
        let state = Arc::new(AppState {
            peers: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        });
        let response = get_peers(State(Arc::clone(&state))).await;
        slogger::info!("Response: {response:#?}");
        assert_eq!(response.0, StatusCode::OK);
        assert_eq!(response.1.0.peers, vec![] as Vec<String>);
    }
    
    #[tokio::test]
    async fn test_get_peers_with_peers() {
        let state = Arc::new(AppState {
            peers: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        });
        state.peers.write().await.insert("peer1".to_string(), "peer1".to_string());
        state.peers.write().await.insert("peer2".to_string(), "peer2".to_string());
        let response = get_peers(State(Arc::clone(&state))).await;
        slogger::info!("Response: {response:#?}");
        assert_eq!(response.0, StatusCode::OK);
        assert_eq!(response.1.0.peers.len(), 2);
        assert!(response.1.0.peers.contains(&"peer1".to_string()));
        assert!(response.1.0.peers.contains(&"peer2".to_string()));
    }

    #[tokio::test]
    async fn test_add_peer() {
        let state = Arc::new(AppState {
            peers: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        });
        let response = add_peer(State(Arc::clone(&state)), axum::Json(PeerRequest { peer: Peer { name: "peer1".to_string(), address: "peer1".to_string() } })).await;
        slogger::info!("Response: {response:#?}");
        assert_eq!(response.0, StatusCode::CREATED);
        assert_eq!(state.peers.read().await.len(), 1);
        assert!(state.peers.read().await.contains_key(&"peer1".to_string()));
    }

}