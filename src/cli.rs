use std::net::SocketAddr;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "scion-router-proto")]
pub struct Cli {
    #[arg(long, default_value = "127.0.0.1:3000")]
    pub http_listen: SocketAddr,

    #[arg(long, default_value = "127.0.0.1:4001")]
    pub data_listen: SocketAddr,

    #[arg(long, default_value = "127.0.0.1:4010")]
    pub beacon_listen: SocketAddr,

    #[arg(long, default_value = "2")]
    pub beacon_interval_secs: u64,
}
