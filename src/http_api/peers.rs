use axum::http::StatusCode;
use axum::extract::State;
use axum::Json;
use std::sync::Arc;

use crate::state::AppState;
use crate::http_api::model::{ PeerRequest, PeersResponse};

pub async fn get_peers(state: State<Arc<AppState>>) -> (StatusCode, Json<PeersResponse>) {

    let peers = state.peers.iter().map(|kv| kv.key().clone()).collect();

    (StatusCode::OK, Json(PeersResponse { peers }))
}

pub async fn add_peer(
    state: State<Arc<AppState>>,
    Json(req): Json<PeerRequest>,
) -> (StatusCode, Json<()>) {
    state.peers.insert(req.peer.name.clone(), req.peer.address);
    (StatusCode::CREATED, Json(()))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::http_api::model::Peer;

    #[tokio::test]
    async fn test_get_peers_empty() {
        let state = Arc::new(AppState {
            peers: Arc::new(dashmap::DashMap::new()),
        });
        let response = get_peers(State(Arc::clone(&state))).await;
        slogger::info!("Response: {response:#?}");
        assert_eq!(response.0, StatusCode::OK);
        assert_eq!(response.1.0.peers, vec![] as Vec<String>);
    }
    
    #[tokio::test]
    async fn test_get_peers_with_peers() {
        let state = Arc::new(AppState {
            peers: Arc::new(dashmap::DashMap::new()),
        });
        state.peers.insert("peer1".to_string(), "peer1".to_string());
        state.peers.insert("peer2".to_string(), "peer2".to_string());
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
            peers: Arc::new(dashmap::DashMap::new()),
        });
        let response = add_peer(State(Arc::clone(&state)), axum::Json(PeerRequest { peer: Peer { name: "peer1".to_string(), address: "peer1".to_string() } })).await;
        slogger::info!("Response: {response:#?}");
        assert_eq!(response.0, StatusCode::CREATED);
        assert_eq!(state.peers.len(), 1);
        assert!(state.peers.contains_key("peer1"));
    }
    
    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn test_add_peer_multithreaded() {

        const CONCURRENCY: usize = 100000;
        const REQUESTS_PER_TASK: usize = 1;

        let state = Arc::new(AppState {
            peers: Arc::new(dashmap::DashMap::new()),
        });

        let tasks = (0..CONCURRENCY).map(|task_id| {
            let state = Arc::clone(&state);
            tokio::spawn(async move {
                for request_id in 0..REQUESTS_PER_TASK {
                    let name = format!("peer{task_id}_{request_id}");
                    let address = format!("addr{task_id}_{request_id}");
                    let response = add_peer(
                        State(Arc::clone(&state)),
                        axum::Json(PeerRequest {
                            peer: Peer { name, address },
                        }),
                    )
                    .await;
                    assert_eq!(response.0, StatusCode::CREATED);
                }
            })
        });

        for task in tasks {
            task.await.expect("task panicked");
        }

        let expected = CONCURRENCY * REQUESTS_PER_TASK;
        assert_eq!(state.peers.len(), expected);
        assert!(state.peers.contains_key("peer0_0"));
        assert!(state
            .peers
            .contains_key(&format!("peer{}_{}", CONCURRENCY - 1, REQUESTS_PER_TASK - 1)));
    }

}