use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use anyhow::Context;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::time::{interval, Duration};
use tokio::{
    net::UdpSocket,
    sync::RwLock,
    task::JoinSet,
};
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

#[derive(Parser, Debug, Clone)]
#[command(name = "scion-router-proto")]
struct Cli {
    #[arg(long, default_value = "127.0.0.1:3000")]
    http_listen: SocketAddr,

    #[arg(long, default_value = "127.0.0.1:4001")]
    data_listen: SocketAddr,

    #[arg(long, default_value = "127.0.0.1:4010")]
    beacon_listen: SocketAddr,

    #[arg(long, default_value = "2")]
    beacon_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BeaconPeer {
    peer: SocketAddr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BeaconMsg {
    ifid: u16,
    next_hop: SocketAddr,
    #[serde(default)]
    bind: Option<SocketAddr>,
}

async fn put_iface(
    Path(ifid): Path<u16>,
    State(state): State<AppState>,
    Json(entry): Json<IfaceEntry>,
) -> StatusCode {
    match upsert_iface(&state, ifid, entry).await {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(e) => {
            error!(ifid, error=%e, "upsert iface failed");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn delete_iface(Path(ifid): Path<u16>, State(state): State<AppState>) -> StatusCode {
    let removed = {
        let mut ifaces = state.ifaces.write().await;
        ifaces.remove(&ifid)
    };
    if removed.is_some() {
        let mut socks = state.iface_socks.write().await;
        socks.remove(&ifid);
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn upsert_iface(state: &AppState, ifid: u16, entry: IfaceEntry) -> anyhow::Result<()> {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RouteEntry {
    next_hop: SocketAddr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IfaceEntry {
    next_hop: SocketAddr,
    #[serde(default)]
    bind: Option<SocketAddr>,
}

#[derive(Debug, Default)]
struct RoutingTable {
    routes: HashMap<String, RouteEntry>,
}

impl RoutingTable {
    fn upsert(&mut self, dst: String, entry: RouteEntry) {
        self.routes.insert(dst, entry);
    }

    fn remove(&mut self, dst: &str) -> bool {
        self.routes.remove(dst).is_some()
    }
}

#[derive(Debug, Clone)]
struct AppState {
    rt: Arc<RwLock<RoutingTable>>,
    ifaces: Arc<RwLock<HashMap<u16, IfaceEntry>>>,
    iface_socks: Arc<RwLock<HashMap<u16, Arc<UdpSocket>>>>,
    beacon_peers: Arc<RwLock<Vec<SocketAddr>>>,
    metrics: Arc<Metrics>,
}

#[derive(Debug, Default)]
struct Metrics {
    rx_packets: AtomicU64,
    drop_not_scion: AtomicU64,
    drop_fwd_meta: AtomicU64,
    drop_end_of_path: AtomicU64,
    drop_no_iface: AtomicU64,
    forwarded: AtomicU64,
    send_errors: AtomicU64,
}

#[derive(Debug, Clone, Copy)]
struct Ia {
    isd: u16,
    asn: [u8; 6],
}

impl Ia {
    fn to_string_key(self) -> String {
        let a0 = u16::from_be_bytes([self.asn[0], self.asn[1]]);
        let a1 = u16::from_be_bytes([self.asn[2], self.asn[3]]);
        let a2 = u16::from_be_bytes([self.asn[4], self.asn[5]]);
        format!("{}-{:x}:{:x}:{:x}", self.isd, a0, a1, a2)
    }
}

fn parse_scion_dst_ia(pkt: &[u8]) -> anyhow::Result<Ia> {
    // Common header is 12 bytes. Destination ISD-AS immediately follows.
    // https://docs.scion.org/en/latest/protocols/scion-header.html
    if pkt.len() < 12 + 2 + 6 {
        anyhow::bail!("packet too short for SCION common+dstIA");
    }

    let version = pkt[0] >> 4;
    if version != 0 {
        anyhow::bail!("unsupported SCION version {version}");
    }

    let off = 12;
    let isd = u16::from_be_bytes([pkt[off], pkt[off + 1]]);
    let asn: [u8; 6] = pkt[off + 2..off + 8]
        .try_into()
        .expect("slice length checked");
    Ok(Ia { isd, asn })
}

#[derive(Debug, Clone, Copy)]
struct ScionFwdMeta {
    curr_hf: u8,
    total_hf: u8,
    curr_hf_byte_off: usize,
    egress_ifid: u16,
}

fn parse_scion_fwd_meta(pkt: &[u8]) -> anyhow::Result<ScionFwdMeta> {
    // Common header: 12 bytes.
    if pkt.len() < 12 {
        anyhow::bail!("packet too short for SCION common header");
    }

    let version = pkt[0] >> 4;
    if version != 0 {
        anyhow::bail!("unsupported SCION version {version}");
    }

    let hdr_len_bytes = (pkt[5] as usize) * 4;
    if hdr_len_bytes < 12 {
        anyhow::bail!("invalid SCION hdrlen {hdr_len_bytes}");
    }
    if pkt.len() < hdr_len_bytes {
        anyhow::bail!("packet shorter than SCION header length");
    }

    let path_type = pkt[8];
    if path_type != 1 {
        anyhow::bail!("unsupported PathType {path_type} (expected SCION=1)");
    }

    // Address header lengths are encoded in DL/SL (2 bits each) in byte 9.
    // We only need them to locate the path header.
    let dl = (pkt[9] >> 4) & 0x03;
    let sl = pkt[9] & 0x03;
    let dst_host_len = 4usize * (dl as usize + 1);
    let src_host_len = 4usize * (sl as usize + 1);
    let addr_hdr_len = 16usize + dst_host_len + src_host_len;
    let path_off = 12usize + addr_hdr_len;
    if path_off + 4 > hdr_len_bytes {
        anyhow::bail!("SCION header too short for PathMeta");
    }

    // PathMeta is a 4-byte big-endian word:
    // C:2 | CurrHF:6 | RSV:6 | Seg0Len:6 | Seg1Len:6 | Seg2Len:6
    let pm = u32::from_be_bytes([
        pkt[path_off],
        pkt[path_off + 1],
        pkt[path_off + 2],
        pkt[path_off + 3],
    ]);
    let curr_hf = ((pm >> 24) & 0x3f) as u8;
    let seg0 = ((pm >> 12) & 0x3f) as u8;
    let seg1 = ((pm >> 6) & 0x3f) as u8;
    let seg2 = (pm & 0x3f) as u8;
    let total_hf = seg0.saturating_add(seg1).saturating_add(seg2);
    if total_hf == 0 {
        anyhow::bail!("empty SCION path");
    }
    if curr_hf >= total_hf {
        anyhow::bail!("CurrHF out of range");
    }

    let num_inf: usize = if seg2 > 0 {
        3
    } else if seg1 > 0 {
        2
    } else {
        1
    };

    let hop_fields_off = path_off + 4 + num_inf * 8;
    let curr_hf_off = hop_fields_off + (curr_hf as usize) * 12;
    if curr_hf_off + 12 > hdr_len_bytes {
        anyhow::bail!("hop field out of SCION header bounds");
    }

    let egress_ifid = u16::from_be_bytes([pkt[curr_hf_off + 4], pkt[curr_hf_off + 5]]);

    Ok(ScionFwdMeta {
        curr_hf,
        total_hf,
        curr_hf_byte_off: path_off,
        egress_ifid,
    })
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn get_routes(State(state): State<AppState>) -> Json<HashMap<String, RouteEntry>> {
    let rt = state.rt.read().await;
    Json(rt.routes.clone())
}

async fn get_ifaces(State(state): State<AppState>) -> Json<HashMap<u16, IfaceEntry>> {
    let ifaces = state.ifaces.read().await;
    Json(ifaces.clone())
}

async fn get_beacon_peers(State(state): State<AppState>) -> Json<Vec<SocketAddr>> {
    let peers = state.beacon_peers.read().await;
    Json(peers.clone())
}

async fn post_beacon_peer(State(state): State<AppState>, Json(peer): Json<BeaconPeer>) -> StatusCode {
    let mut peers = state.beacon_peers.write().await;
    if !peers.contains(&peer.peer) {
        peers.push(peer.peer);
    }
    StatusCode::NO_CONTENT
}

async fn put_route(
    Path(dst): Path<String>,
    State(state): State<AppState>,
    Json(entry): Json<RouteEntry>,
) -> StatusCode {
    let mut rt = state.rt.write().await;
    rt.upsert(dst, entry);
    StatusCode::NO_CONTENT
}

async fn delete_route(Path(dst): Path<String>, State(state): State<AppState>) -> StatusCode {
    let mut rt = state.rt.write().await;
    if rt.remove(&dst) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn run_http(state: AppState, http_listen: SocketAddr) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/routes", get(get_routes))
        .route("/routes/:dst", post(put_route).delete(delete_route))
        .route("/ifaces", get(get_ifaces))
        .route("/ifaces/:ifid", post(put_iface).delete(delete_iface))
        .route("/beacon/peers", get(get_beacon_peers).post(post_beacon_peer))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(http_listen)
        .await
        .with_context(|| format!("bind http {http_listen}"))?;

    info!(%http_listen, "http listening");
    axum::serve(listener, app)
        .await
        .context("http server failed")?;
    Ok(())
}

async fn run_beacon_rx(state: AppState, beacon_listen: SocketAddr) -> anyhow::Result<()> {
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

async fn run_beacon_tx(state: AppState, beacon_interval_secs: u64) -> anyhow::Result<()> {
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

async fn run_dataplane(state: AppState, data_listen: SocketAddr) -> anyhow::Result<()> {
    let sock = UdpSocket::bind(data_listen)
        .await
        .with_context(|| format!("bind udp {data_listen}"))?;

    info!(%data_listen, "dataplane listening (udp)");

    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let (n, peer) = sock.recv_from(&mut buf).await.context("udp recv")?;
        state.metrics.rx_packets.fetch_add(1, Ordering::Relaxed);
        let raw = &mut buf[..n];

        let dst_ia = match parse_scion_dst_ia(raw) {
            Ok(ia) => ia,
            Err(e) => {
                state
                    .metrics
                    .drop_not_scion
                    .fetch_add(1, Ordering::Relaxed);
                warn!(%peer, error=%e, "drop: not a SCION packet (or unsupported)");
                continue;
            }
        };
        let dst_key = dst_ia.to_string_key();

        let fwd = match parse_scion_fwd_meta(raw) {
            Ok(m) => m,
            Err(e) => {
                state
                    .metrics
                    .drop_fwd_meta
                    .fetch_add(1, Ordering::Relaxed);
                warn!(dst=%dst_key, %peer, error=%e, "drop: cannot parse forwarding meta");
                continue;
            }
        };

        if fwd.curr_hf.saturating_add(1) >= fwd.total_hf {
            state
                .metrics
                .drop_end_of_path
                .fetch_add(1, Ordering::Relaxed);
            warn!(dst=%dst_key, %peer, curr_hf=fwd.curr_hf, total_hf=fwd.total_hf, "drop: end of path");
            continue;
        }

        let (next_hop, out_sock) = {
            let ifaces = state.ifaces.read().await;
            let socks = state.iface_socks.read().await;
            let next_hop = ifaces.get(&fwd.egress_ifid).map(|e| e.next_hop);
            let out_sock = socks.get(&fwd.egress_ifid).cloned();
            (next_hop, out_sock)
        };

        let (Some(next_hop), Some(out_sock)) = (next_hop, out_sock) else {
            state
                .metrics
                .drop_no_iface
                .fetch_add(1, Ordering::Relaxed);
            warn!(dst=%dst_key, %peer, egress_ifid=fwd.egress_ifid, "no iface mapping");
            continue;
        };

        // Advance CurrHF in PathMeta (keep the C bits intact).
        // CurrHF is stored in the low 6 bits of the first PathMeta byte.
        let pm0 = raw[fwd.curr_hf_byte_off];
        let c_bits = pm0 & 0b1100_0000;
        let next_curr = fwd.curr_hf + 1;
        raw[fwd.curr_hf_byte_off] = c_bits | (next_curr & 0b0011_1111);

        match out_sock.send_to(raw, next_hop).await {
            Ok(_) => {
                state.metrics.forwarded.fetch_add(1, Ordering::Relaxed);
                info!(dst=%dst_key, %peer, %next_hop, egress_ifid=fwd.egress_ifid, curr_hf=fwd.curr_hf, "forwarded");
            }
            Err(e) => {
                state
                    .metrics
                    .send_errors
                    .fetch_add(1, Ordering::Relaxed);
                error!(dst=%dst_key, %peer, %next_hop, egress_ifid=fwd.egress_ifid, error=%e, "send failed");
            }
        }
    }
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli = Cli::parse();

    let state = AppState {
        rt: Arc::new(RwLock::new(RoutingTable::default())),
        ifaces: Arc::new(RwLock::new(HashMap::new())),
        iface_socks: Arc::new(RwLock::new(HashMap::new())),
        beacon_peers: Arc::new(RwLock::new(Vec::new())),
        metrics: Arc::new(Metrics::default()),
    };

    let mut set = JoinSet::<anyhow::Result<()>>::new();
    set.spawn(run_http(state.clone(), cli.http_listen));
    set.spawn(run_dataplane(state.clone(), cli.data_listen));
    set.spawn(run_beacon_rx(state.clone(), cli.beacon_listen));
    set.spawn(run_beacon_tx(state.clone(), cli.beacon_interval_secs));

    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(())) => {
                info!("task exited");
                break;
            }
            Ok(Err(e)) => {
                error!(error=%e, "task failed");
                return Err(e);
            }
            Err(e) => {
                return Err(anyhow::anyhow!(e).context("join error"));
            }
        }
    }

    Ok(())
}
