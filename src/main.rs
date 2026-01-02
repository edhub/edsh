use anyhow::Result;
use clap::{Parser, Subcommand};
use iroh::{EndpointId, RelayUrl};
use tracing_subscriber::EnvFilter;

mod client;
mod protocol;
mod server;

#[derive(Parser)]
#[command(author, version, about = "edsh - iroh-based SSH proxy", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Connect to a remote edsh server (Client mode)
    Connect {
        /// The Endpoint ID of the edsh-server to connect to.
        #[arg()]
        endpoint_id: EndpointId,

        /// Optional relay URL to use for discovery and NAT traversal.
        #[arg(short = 'r')]
        relay_url: Option<RelayUrl>,
    },
    /// Run as an edsh server
    Server {
        /// Optional relay URL to use for discovery and NAT traversal.
        #[arg(short = 'r')]
        relay_url: Option<RelayUrl>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logging
    // We use stderr for logging because stdout is used for data in client mode
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("error"));
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Connect {
            endpoint_id,
            relay_url,
        } => {
            client::run_client(endpoint_id, relay_url).await?;
        }
        Commands::Server { relay_url } => {
            server::run_server(relay_url).await?;
        }
    }

    Ok(())
}
