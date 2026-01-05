use std::time::Duration;

use crate::protocol::EDSH_ALPN;
use anyhow::Result;
use iroh::{
    Endpoint, EndpointAddr, EndpointId, RelayMap, RelayUrl, TransportAddr,
    discovery::static_provider::StaticProvider, endpoint::TransportConfig,
};
use tokio::io::{self, AsyncWriteExt};

pub async fn run_client(endpoint_id: EndpointId, relay_urls: Vec<RelayUrl>) -> Result<()> {
    let mut transport_config = TransportConfig::default();
    transport_config.keep_alive_interval(Some(Duration::from_secs(20)));
    transport_config.max_idle_timeout(Some(Duration::from_secs(60).try_into().unwrap()));

    let mut builder = Endpoint::builder().transport_config(transport_config);

    if !relay_urls.is_empty() {
        tracing::info!("Using relay URLs: {:?}", relay_urls);

        let discovery = StaticProvider::new();
        discovery.add_endpoint_info(EndpointAddr {
            id: endpoint_id,
            addrs: relay_urls
                .iter()
                .map(|url| TransportAddr::Relay(url.clone()))
                .collect(),
        });

        let relay_map: RelayMap = relay_urls.into_iter().collect();
        let relay_mode = iroh::RelayMode::Custom(relay_map);

        builder = builder
            .clear_discovery()
            .discovery(discovery)
            .relay_mode(relay_mode);
    }
    // 1. 创建一个 Endpoint
    // In iroh 0.95, Endpoint::builder() is the standard way to start configuration.
    let endpoint = builder.bind().await?;

    // 2 & 3. 使用 ALPN = edsh 连接到目标 Endpoint id
    // iroh handles the ALPN negotiation and connection establishment.
    let conn = endpoint.connect(endpoint_id, EDSH_ALPN).await?;

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
    tokio::select! {
        res = client_to_server => {
            if let Err(e) = res {
                tracing::info!("stdin to iroh error: {:?}", e);
            }
        }
        res = server_to_client => {
            if let Err(e) = res {
                tracing::info!("iroh to stdout error: {:?}", e);
            }
        }
    }

    Ok(())
}
