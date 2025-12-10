//! Omniclip CLI - Cross-platform clipboard sync.

mod commands;
mod process;
mod ui;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "omniclip")]
#[command(about = "Cross-platform clipboard sync", long_about = None)]
struct Cli {
    /// Device name to advertise
    #[arg(short, long, default_value_t = default_device_name())]
    name: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

fn default_device_name() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "omniclip-device".to_string())
}

#[derive(Subcommand)]
enum Commands {
    /// Start the omniclip service (default)
    Run,
    /// Show device info
    Info,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("omniclip=info".parse()?)
                .add_directive("mdns_sd=warn".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => commands::run_service(cli.name).await?,
        Commands::Info => commands::show_info(cli.name),
    }

    Ok(())
}
