use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;


#[derive(Debug)]
pub struct AppState {
    pub peers: Arc<RwLock<HashMap<String, String>>>,
}

