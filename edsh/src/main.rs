use anyhow::Result;
use clap::Parser;
use iroh::{EndpointId, RelayUrl};

#[derive(Parser)]
#[command(author, version, about = "edsh - iroh-based SSH proxy client", long_about = None)]
struct Cli {
    #[arg()]
    endpoint_id: EndpointId,

    #[arg(short = 'r')]
    relay_url: Option<RelayUrl>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("endpoint {}", cli.endpoint_id);
    // println!("url {}", cli.relay_url.unwrap_or("None"));
    cli.relay_url.map(|v| println!("{v:?}"));

    Ok(())
}
