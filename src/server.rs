use crate::protocol::EDSH_ALPN;
use anyhow::{Context, Result};
use iroh::{Endpoint, RelayMap, RelayUrl, SecretKey};
use std::path::PathBuf;
use tokio::io::{self, AsyncWriteExt};
use tokio::net::TcpStream;

pub async fn run_server(relay_url: Option<RelayUrl>) -> Result<()> {
    // 1. Handle SecretKey - Load from disk or generate new one
    let key_path = PathBuf::from("edsh_server.key");
    let secret_key = if key_path.exists() {
        let bytes = tokio::fs::read(&key_path).await?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid key file size: expected 32 bytes"))?;
        SecretKey::from_bytes(&bytes)
    } else {
        let mut rng = rand::rng();
        let key = SecretKey::generate(&mut rng);
        tokio::fs::write(&key_path, key.to_bytes()).await?;
        key
    };

    let public_key = secret_key.public();
    tracing::info!("Server started. Public Key (Endpoint ID): {}", public_key);
    println!("Server started. Public Key (Endpoint ID): {}", public_key);

    // 2. Create Iroh Endpoint
    let mut builder = Endpoint::builder()
        .secret_key(secret_key)
        .alpns(vec![EDSH_ALPN.to_vec()]);

    if let Some(url) = relay_url {
        tracing::info!("Using relay URL: {}", url);
        let relay_map = RelayMap::from(RelayUrl::from(url));

        let relay_mode = iroh::RelayMode::Custom(relay_map);
        builder = builder.relay_mode(relay_mode);
    }

    let endpoint = builder.bind().await?;

    tracing::info!("Listening on ALPN: {:?}", std::str::from_utf8(EDSH_ALPN)?);

    // 3. Wait for bi connections - Handle multiple connections
    while let Some(incoming) = endpoint.accept().await {
        let accepting = match incoming.accept() {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("Failed to accept incoming connection: {:?}", e);
                continue;
            }
        };

        tokio::spawn(async move {
            let conn = match accepting.await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::error!("Failed to establish connection: {:?}", e);
                    return;
                }
            };

            let remote_node_id = conn.remote_id();
            tracing::info!("Accepted connection from {}", remote_node_id);

            match handle_connection(conn).await {
                Ok(_) => tracing::info!("Connection from {} closed", remote_node_id),
                Err(e) => {
                    tracing::error!("Error handling connection from {}: {:?}", remote_node_id, e)
                }
            }
        });
    }

    Ok(())
}

async fn handle_connection(conn: iroh::endpoint::Connection) -> Result<()> {
    // Accept bidirectional streams
    loop {
        match conn.accept_bi().await {
            Ok((mut send_stream, mut recv_stream)) => {
                tracing::debug!("Accepted new bidirectional stream");
                tokio::spawn(async move {
                    if let Err(e) = forward_to_ssh(&mut send_stream, &mut recv_stream).await {
                        tracing::error!("Forwarding error: {:?}", e);
                    }
                });
            }
            Err(e) => {
                // Connection closed or error
                return Err(e.into());
            }
        }
    }
}

async fn forward_to_ssh(
    iroh_send: &mut iroh::endpoint::SendStream,
    iroh_recv: &mut iroh::endpoint::RecvStream,
) -> Result<()> {
    // 4. Connect to local SSH server (typically port 22)
    let mut ssh_tcp = TcpStream::connect("127.0.0.1:22")
        .await
        .context("Failed to connect to local SSH server on port 22")?;

    let (mut tcp_read, mut tcp_write) = ssh_tcp.split();

    // Data exchange using tokio::io::copy
    let iroh_to_ssh = async {
        io::copy(iroh_recv, &mut tcp_write).await?;
        tcp_write.shutdown().await?;
        anyhow::Ok(())
    };

    let ssh_to_iroh = async {
        io::copy(&mut tcp_read, iroh_send).await?;
        // finish() is sync in iroh 0.95 and returns Result<(), ClosedStream>
        let _ = iroh_send.finish();
        anyhow::Ok(())
    };

    // 使用 try_join! 等待两个方向的任务都正常结束
    tokio::try_join!(iroh_to_ssh, ssh_to_iroh)?;

    Ok(())
}
