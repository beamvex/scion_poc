use std::net::SocketAddr;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "scion-router-proto")]
pub struct Cli {
    #[arg(long, default_value = "127.0.0.1:3000")]
    pub http_listen: SocketAddr,

    #[arg(long, default_value = "")]
    pub master: String,

    #[arg(long, default_value = "2")]
    pub beacon_interval_secs: u64,
}
