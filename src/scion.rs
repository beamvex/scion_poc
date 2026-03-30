#[derive(Debug, Clone, Copy)]
pub struct Ia {
    pub isd: u16,
    pub asn: [u8; 6],
}

impl Ia {
    pub fn to_string_key(self) -> String {
        let a0 = u16::from_be_bytes([self.asn[0], self.asn[1]]);
        let a1 = u16::from_be_bytes([self.asn[2], self.asn[3]]);
        let a2 = u16::from_be_bytes([self.asn[4], self.asn[5]]);
        format!("{}-{:x}:{:x}:{:x}", self.isd, a0, a1, a2)
    }
}

pub fn parse_scion_dst_ia(pkt: &[u8]) -> anyhow::Result<Ia> {
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
pub struct ScionFwdMeta {
    pub curr_hf: u8,
    pub total_hf: u8,
    pub curr_hf_byte_off: usize,
    pub egress_ifid: u16,
}

pub fn parse_scion_fwd_meta(pkt: &[u8]) -> anyhow::Result<ScionFwdMeta> {
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

    let dl = (pkt[9] >> 4) & 0x03;
    let sl = pkt[9] & 0x03;
    let dst_host_len = 4usize * (dl as usize + 1);
    let src_host_len = 4usize * (sl as usize + 1);
    let addr_hdr_len = 16usize + dst_host_len + src_host_len;
    let path_off = 12usize + addr_hdr_len;
    if path_off + 4 > hdr_len_bytes {
        anyhow::bail!("SCION header too short for PathMeta");
    }

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
