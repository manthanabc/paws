use std::env;
use std::io::Read;
use std::panic;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use paws_domain::TitleFormat;

mod cli;
mod client_server;

use cli::Cli;
use client_server::{Client, Server};

use paws_api::PawsAPI;
use paws_infra::PawsInfra;
use paws_repo::PawsRepo;
use paws_services::PawsServices;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Set up panic hook for better error display
    panic::set_hook(Box::new(|panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unexpected error occurred".to_string()
        };

        println!("{}", TitleFormat::error(message.to_string()).display());
        std::process::exit(1);
    }));

    let mut cli = Cli::parse();

    // Check if there's piped input
    if !atty::is(atty::Stream::Stdin) {
        let mut stdin_content = String::new();
        std::io::stdin().read_to_string(&mut stdin_content)?;
        let trimmed_content = stdin_content.trim();
        if !trimmed_content.is_empty() {
            cli.piped_input = Some(trimmed_content.to_string());
        }
    }

    // Initialize logging
    let log_level = if cli.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(format!(
            "paws={},tokio={}",
            log_level, log_level
        )))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Handle server subcommand
    if let Some(cli::TopLevelCommand::Server { socket, verbose }) = &cli.subcommands {
        start_server(socket.clone(), *verbose).await?;
        return Ok(());
    }

    // Default: start client
    start_client(cli).await?;

    Ok(())
}

async fn start_server(socket_path: Option<PathBuf>, verbose: bool) -> Result<()> {
    use tracing::info;

    info!("Starting Paws Server...");

    // Set up current working directory
    let cwd = std::env::current_dir()?;

    // Initialize the server components
    let restricted = false; // Server doesn't need restricted mode
    let infra = Arc::new(PawsInfra::new(restricted, cwd));
    let repo = Arc::new(PawsRepo::new(infra.clone()));
    let services = Arc::new(PawsServices::new(repo.clone()));
    let api = Arc::new(PawsAPI::new(services, repo));

    // Use default socket path if none provided
    let socket_path = socket_path.unwrap_or_else(|| {
        let mut path = dirs::runtime_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        path.push("paws.sock");
        path
    });

    // Create and start the server
    let mut server = Server::new(api, socket_path);

    // Handle shutdown signals
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    
    tokio::spawn(async move {
        use signal_hook::consts::TERM_SIGNALS;
        for sig in TERM_SIGNALS {
            signal_hook_tokio::Signals::new([sig])
                .await
                .expect("Failed to register signal handler");
        }
        info!("Server shutdown signal received");
        let _ = shutdown_tx.send(());
    });

    info!("Server listening on {}", server.socket_path().display());

    // Start the server
    tokio::select! {
        result = server.start() => {
            if let Err(e) = result {
                use tracing::error;
                error!("Server error: {}", e);
            }
        }
        _ = shutdown_rx.recv() => {
            info!("Shutting down server...");
        }
    }

    info!("Paws Server stopped");
    Ok(())
}

async fn start_client(cli: Cli) -> Result<()> {
    use tracing::info;

    info!("Starting Paws Client...");

    // Start server if not running
    let mut client = Client::new(None);
    client.ensure_server_running()?;

    // For now, just test the connection
    let stream = client.connect().await?;
    info!("Connected to server");

    // TODO: Implement full client UI functionality
    // This would include:
    // - Interactive terminal UI
    // - Command processing
    // - Conversation management
    // - etc.

    info!("Client functionality not yet implemented");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing_with_server_command() {
        let cli = Cli::parse_from(["paws", "server", "--socket", "/tmp/test.sock"]);
        if let Some(cli::TopLevelCommand::Server { socket, .. }) = cli.subcommands {
            assert_eq!(socket, Some(std::path::PathBuf::from("/tmp/test.sock")));
        } else {
            panic!("Expected server command");
        }
    }

    #[test]
    fn test_cli_parsing_default_mode() {
        let cli = Cli::parse_from(["paws", "--prompt", "hello"]);
        assert_eq!(cli.prompt, Some("hello".to_string()));
        assert!(cli.subcommands.is_none());
    }
}