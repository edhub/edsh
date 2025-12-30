use anyhow::{Context, Result};
use clap::Parser;
use edsh_common::protocol::{EDSH_ALPN, IS_ALPN};
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayUrl, SecretKey};
use tokio::io::{self, AsyncWriteExt};

#[derive(Parser)]
#[command(author, version, about = "edsh - iroh-based SSH proxy client", long_about = None)]
struct Cli {
    /// The Endpoint ID of the edsh-server to connect to.
    #[arg()]
    endpoint_id: EndpointId,

    /// Optional relay URL to use for discovery and NAT traversal.
    #[arg(short = 'r')]
    relay_url: RelayUrl,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logging
    // We use stderr for logging because stdout is used for data
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    // 1. 创建一个 Endpoint
    // In iroh 0.95, Endpoint::builder() is the standard way to start configuration.
    let endpoint = Endpoint::builder()
        // .secret_key(secret_key)
        .alpns(vec![IS_ALPN.to_vec()])
        .bind()
        .await?;

    let endpoint_addr = EndpointAddr::new(cli.endpoint_id).with_relay_url(cli.relay_url);
    // 2 & 3. 使用 ALPN = edsh 连接到目标 Endpoint id
    // iroh handles the ALPN negotiation and connection establishment.
    let conn = endpoint.connect(endpoint_addr, IS_ALPN).await?;

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
    // Wait for either direction to finish or error out.
    tokio::select! {
        res = client_to_server => {
            if let Err(e) = res {
                tracing::error!("stdin to iroh error: {:?}", e);
            }
        }
        res = server_to_client => {
            if let Err(e) = res {
                tracing::error!("iroh to stdout error: {:?}", e);
            }
        }
    }

    Ok(())
}
