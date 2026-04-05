use std::sync::Arc;

use dashmap::DashMap;


#[derive(Debug)]
pub struct AppState {
    pub peers: Arc<DashMap<String, String>>,
}

