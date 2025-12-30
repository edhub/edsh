use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use edsh_common::protocol::{EDSH_ALPN, IS_ALPN};
use iroh::{Endpoint, EndpointId, RelayConfig, RelayMap, RelayUrl};
use tokio::io::{self, AsyncWriteExt};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(author, version, about = "edsh - iroh-based SSH proxy client", long_about = None)]
struct Cli {
    /// The Endpoint ID of the edsh-server to connect to.
    #[arg()]
    endpoint_id: EndpointId,

    /// Optional relay URL to use for discovery and NAT traversal.
    #[arg(short = 'r')]
    relay_url: Option<RelayUrl>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logging
    // We use stderr for logging because stdout is used for data
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("error"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cli = Cli::parse();

    let mut builder = Endpoint::builder().alpns(vec![IS_ALPN.to_vec(), EDSH_ALPN.to_vec()]);

    if let Some(url) = cli.relay_url {
        // tracing::info!("Using relay URL: {}", url);
        let relay_config = Arc::new(RelayConfig::from(url.clone()));
        let relay_map = RelayMap::empty();
        relay_map.insert(url, relay_config);
        let relay_mode = iroh::RelayMode::Custom(relay_map);
        builder = builder.relay_mode(relay_mode);
    }
    // 1. 创建一个 Endpoint
    // In iroh 0.95, Endpoint::builder() is the standard way to start configuration.
    let endpoint = builder.bind().await?;

    // 2 & 3. 使用 ALPN = edsh 连接到目标 Endpoint id
    // iroh handles the ALPN negotiation and connection establishment.
    let conn = endpoint.connect(cli.endpoint_id, IS_ALPN).await?;

    // Open a bidirectional stream for the SSH traffic
    let (mut send_stream, mut recv_stream) = conn.open_bi().await?;

    // 4. 使用 tokio::io::copy 转发流量
    // In a ProxyCommand context, we bridge stdin/stdout to the iroh stream.
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    // Bridge stdin -> iroh send_stream
    let client_to_server = async {
        io::copy(&mut stdin, &mut send_stream).await?;
        send_stream.shutdown().await?;
        anyhow::Ok(())
    };

    // Bridge iroh recv_stream -> stdout
    let server_to_client = async {
        io::copy(&mut recv_stream, &mut stdout).await?;
        stdout.flush().await?;
        anyhow::Ok(())
    };

    // 5. 连接断开后，结束
    // Wait for both directions to finish.
    // This ensures that all data from the server is received even if stdin is closed first.
    let (res1, res2) = tokio::join!(client_to_server, server_to_client);

    if let Err(e) = res1 {
        tracing::error!("stdin to iroh error: {:?}", e);
    }
    if let Err(e) = res2 {
        tracing::error!("iroh to stdout error: {:?}", e);
    }

    Ok(())
}
