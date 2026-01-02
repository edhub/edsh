use anyhow::Result;
use clap::{Parser, Subcommand};
use iroh::{EndpointId, RelayUrl};
use serde::Deserialize;
use tracing_subscriber::EnvFilter;

mod client;
mod protocol;
mod server;

#[derive(Parser)]
#[command(author, version, about = "edsh - iroh-based SSH proxy", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// The Endpoint ID of the edsh-server to connect to (Client mode).
    /// This allows using 'edsh <EndpointID>' directly.
    #[arg(index = 1)]
    endpoint_id: Option<EndpointId>,

    /// Optional relay URL to use for discovery and NAT traversal.
    /// Can be specified multiple times.
    #[arg(short = 'r', global = true)]
    relay_urls: Vec<RelayUrl>,
}

#[derive(Deserialize, Default)]
struct Config {
    #[serde(alias = "relayurls")]
    relay_urls: Option<Vec<String>>,
}

fn load_config() -> Config {
    let config_paths = [
        dirs::home_dir().map(|p| p.join(".edsh.toml")),
        dirs::home_dir().map(|p| p.join(".config/edsh/edsh.toml")),
    ];

    for path in config_paths.into_iter().flatten() {
        tracing::debug!("Checking config path: {:?}", path);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(config) = toml::from_str::<Config>(&content) {
                    tracing::info!("Loaded config from {:?}", path);
                    return config;
                }
            }
        }
    }
    tracing::debug!("No config file found, using defaults");
    Config::default()
}

#[derive(Subcommand)]
enum Commands {
    /// Run as an edsh server
    Server,
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
    let config = load_config();

    // Merge relay URLs: CLI arguments take precedence if provided,
    // otherwise use config file, otherwise empty.
    let relay_urls = if !cli.relay_urls.is_empty() {
        cli.relay_urls
    } else {
        config
            .relay_urls
            .unwrap_or_default()
            .into_iter()
            .filter_map(|s| s.parse().ok())
            .collect()
    };

    match (cli.command, cli.endpoint_id) {
        (Some(Commands::Server), _) => {
            server::run_server(relay_urls).await?;
        }
        (None, Some(endpoint_id)) => {
            // If no subcommand but an endpoint_id is provided, run as client
            client::run_client(endpoint_id, relay_urls).await?;
        }
        _ => {
            // This case handles missing arguments or invalid combinations
            use clap::CommandFactory;
            Cli::command().print_help()?;
            std::process::exit(1);
        }
    }

    Ok(())
}
