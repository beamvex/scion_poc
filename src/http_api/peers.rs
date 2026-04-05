use axum::http::StatusCode;
use axum::extract::State;
use axum::Json;
use std::sync::Arc;

use crate::state::AppState;
use crate::http_api::model::{PeerResponse};

pub async fn get_peers(_state: State<Arc<AppState>>) -> (StatusCode, Json<PeerResponse>) {

    let peers =_state.peers.read().await;
    let peers = peers.keys().cloned().collect();

    (StatusCode::OK, Json(PeerResponse { peers }))
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
}