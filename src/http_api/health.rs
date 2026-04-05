use axum::http::StatusCode;
use axum::extract::State;
use axum::Json;
use std::sync::Arc;

use crate::state::AppState;
use crate::http_api::model::HealthResponse;


pub async fn health(_state: State<Arc<AppState>>) -> (StatusCode, Json<HealthResponse>) {
    (StatusCode::OK, Json(HealthResponse { message: "OK".to_string() }))
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_health() {
        let state = Arc::new(AppState {
            peers: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        });
        let response = health(State(Arc::clone(&state))).await;
        slogger::info!("Response: {response:#?}");
        assert_eq!(response.0, StatusCode::OK);
        assert_eq!(response.1.0.message, "OK");
    }
}