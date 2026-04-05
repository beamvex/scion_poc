use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct HealthResponse {
    pub message: String,
}

impl std::fmt::Display for HealthResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[derive(Serialize, Debug)]
pub struct PeerResponse {
    pub peers: Vec<String>,
}

impl std::fmt::Display for PeerResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.peers)
    }
}
