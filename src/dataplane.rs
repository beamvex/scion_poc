use std::net::SocketAddr;

use anyhow::Context;
use tokio::net::UdpSocket;
use tracing::{error, info, warn};

use crate::{
    scion::{parse_scion_dst_ia, parse_scion_fwd_meta},
    state::AppState,
};

pub async fn run_dataplane(state: AppState, data_listen: SocketAddr) -> anyhow::Result<()> {
    let sock = UdpSocket::bind(data_listen)
        .await
        .with_context(|| format!("bind udp {data_listen}"))?;

    info!(%data_listen, "dataplane listening (udp)");

    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let (n, peer) = sock.recv_from(&mut buf).await.context("udp recv")?;
        state
            .metrics
            .rx_packets
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let raw = &mut buf[..n];

        let dst_ia = match parse_scion_dst_ia(raw) {
            Ok(ia) => ia,
            Err(e) => {
                state
                    .metrics
                    .drop_not_scion
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                warn!(dst=%dst_key, %peer, error=%e, "drop: cannot parse forwarding meta");
                continue;
            }
        };

        if fwd.curr_hf.saturating_add(1) >= fwd.total_hf {
            state
                .metrics
                .drop_end_of_path
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            warn!(dst=%dst_key, %peer, egress_ifid=fwd.egress_ifid, "no iface mapping");
            continue;
        };

        let pm0 = raw[fwd.curr_hf_byte_off];
        let c_bits = pm0 & 0b1100_0000;
        let next_curr = fwd.curr_hf + 1;
        raw[fwd.curr_hf_byte_off] = c_bits | (next_curr & 0b0011_1111);

        match out_sock.send_to(raw, next_hop).await {
            Ok(_) => {
                state
                    .metrics
                    .forwarded
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                info!(dst=%dst_key, %peer, %next_hop, egress_ifid=fwd.egress_ifid, curr_hf=fwd.curr_hf, "forwarded");
            }
            Err(e) => {
                state
                    .metrics
                    .send_errors
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                error!(dst=%dst_key, %peer, %next_hop, egress_ifid=fwd.egress_ifid, error=%e, "send failed");
            }
        }
    }
}
