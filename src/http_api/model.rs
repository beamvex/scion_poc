use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
pub struct HealthResponse {
    pub message: String,
}

impl std::fmt::Display for HealthResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Peer {
    pub name: String,
    pub address: String,
}

impl std::fmt::Display for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.name, self.address)
    }
}

#[derive(Serialize, Debug)]
pub struct PeersResponse {
    pub peers: Vec<String>,
}

impl std::fmt::Display for PeersResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.peers)
    }
}


#[derive(Serialize, Debug)]
pub struct PeerResponse {
    pub peer: Peer,
}

impl std::fmt::Display for PeerResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.peer)
    }
}


#[derive(Serialize, Deserialize, Debug)]
pub struct PeerRequest {
    pub peer: Peer,
}

impl std::fmt::Display for PeerRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.peer)
    }
}