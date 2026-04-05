use axum::http::StatusCode;
use axum::extract::State;
use axum::Json;
use std::sync::Arc;

use crate::state::AppState;
use crate::http_api::model::{PeerResponse};


pub async fn get_peers(_state: State<Arc<AppState>>) -> (StatusCode, Json<PeerResponse>) {
    (StatusCode::OK, Json(PeerResponse { peers: vec![] }))
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_get_peers() {
        let state = Arc::new(AppState {
            peers: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        });
        let response = get_peers(State(Arc::clone(&state))).await;
        slogger::info!("Response: {response:#?}");
        assert_eq!(response.0, StatusCode::OK);
        assert_eq!(response.1.0.peers, vec![] as Vec<String>);
    }
}