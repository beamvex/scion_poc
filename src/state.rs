use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::AtomicU64,
        Arc,
    },
};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::{net::UdpSocket, sync::RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEntry {
    pub next_hop: SocketAddr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfaceEntry {
    pub next_hop: SocketAddr,
    #[serde(default)]
    pub bind: Option<SocketAddr>,
}

#[derive(Debug, Default)]
pub struct RoutingTable {
    pub routes: HashMap<String, RouteEntry>,
}

impl RoutingTable {
    pub fn upsert(&mut self, dst: String, entry: RouteEntry) {
        self.routes.insert(dst, entry);
    }

    pub fn remove(&mut self, dst: &str) -> bool {
        self.routes.remove(dst).is_some()
    }
}

#[derive(Debug, Default)]
pub struct Metrics {
    pub rx_packets: AtomicU64,
    pub drop_not_scion: AtomicU64,
    pub drop_fwd_meta: AtomicU64,
    pub drop_end_of_path: AtomicU64,
    pub drop_no_iface: AtomicU64,
    pub forwarded: AtomicU64,
    pub send_errors: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub rt: Arc<RwLock<RoutingTable>>,
    pub ifaces: Arc<RwLock<HashMap<u16, IfaceEntry>>>,
    pub iface_socks: Arc<RwLock<HashMap<u16, Arc<UdpSocket>>>>,
    pub beacon_peers: Arc<RwLock<Vec<SocketAddr>>>,
    pub metrics: Arc<Metrics>,
}

pub async fn upsert_iface(state: &AppState, ifid: u16, entry: IfaceEntry) -> anyhow::Result<()> {
    let bind = entry
        .bind
        .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)));

    let sock = UdpSocket::bind(bind)
        .await
        .with_context(|| format!("bind iface udp {bind}"))?;

    {
        let mut ifaces = state.ifaces.write().await;
        ifaces.insert(ifid, entry);
    }
    {
        let mut socks = state.iface_socks.write().await;
        socks.insert(ifid, Arc::new(sock));
    }

    Ok(())
}
