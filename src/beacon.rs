use std::net::SocketAddr;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::{net::UdpSocket, time::{interval, Duration}};
use tracing::{info, warn};

use crate::state::{upsert_iface, AppState, IfaceEntry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaconPeer {
    pub peer: SocketAddr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeaconMsg {
    pub ifid: u16,
    pub next_hop: SocketAddr,
    #[serde(default)]
    pub bind: Option<SocketAddr>,
}

pub async fn run_beacon_rx(state: AppState, beacon_listen: SocketAddr) -> anyhow::Result<()> {
    let sock = UdpSocket::bind(beacon_listen)
        .await
        .with_context(|| format!("bind beacon udp {beacon_listen}"))?;
    info!(%beacon_listen, "beacon rx listening (udp)");

    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let (n, peer) = sock.recv_from(&mut buf).await.context("beacon recv")?;
        let raw = &buf[..n];
        let msg: BeaconMsg = match serde_json::from_slice(raw) {
            Ok(m) => m,
            Err(e) => {
                warn!(%peer, error=%e, "drop: invalid beacon json");
                continue;
            }
        };

        let entry = IfaceEntry {
            next_hop: msg.next_hop,
            bind: msg.bind,
        };

        if let Err(e) = upsert_iface(&state, msg.ifid, entry).await {
            warn!(%peer, ifid=msg.ifid, error=%e, "failed to apply beacon");
            continue;
        }

        info!(%peer, ifid=msg.ifid, next_hop=%msg.next_hop, "applied beacon");
    }
}

pub async fn run_beacon_tx(state: AppState, beacon_interval_secs: u64) -> anyhow::Result<()> {
    let sock = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0)))
        .await
        .context("bind beacon tx socket")?;

    let mut ticker = interval(Duration::from_secs(beacon_interval_secs.max(1)));
    loop {
        ticker.tick().await;

        let peers = {
            let peers = state.beacon_peers.read().await;
            peers.clone()
        };
        if peers.is_empty() {
            continue;
        }

        let ifaces = {
            let ifaces = state.ifaces.read().await;
            ifaces.clone()
        };

        for (ifid, iface) in ifaces {
            let msg = BeaconMsg {
                ifid,
                next_hop: iface.next_hop,
                bind: iface.bind,
            };
            let bytes = match serde_json::to_vec(&msg) {
                Ok(b) => b,
                Err(e) => {
                    warn!(ifid, error=%e, "beacon serialize failed");
                    continue;
                }
            };

            for peer in &peers {
                if let Err(e) = sock.send_to(&bytes, peer).await {
                    warn!(%peer, ifid, error=%e, "beacon send failed");
                }
            }
        }
    }
}
