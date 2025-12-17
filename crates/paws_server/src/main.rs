use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Command line arguments for the server
#[derive(clap::Parser, Debug)]
struct ServerArgs {
    /// Unix socket path for IPC
    #[arg(long, default_value_t = default_socket_path())]
    socket: PathBuf,

    /// Enable verbose logging
    #[arg(long)]
    verbose: bool,
}

fn default_socket_path() -> PathBuf {
    let mut path = dirs::runtime_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    path.push("paws.sock");
    path
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = ServerArgs::parse();

    // Initialize logging
    let log_level = if args.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(format!(
            "paws_server={},tokio={}",
            log_level, log_level
        )))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Paws Server...");

    // Remove existing socket file
    if args.socket.exists() {
        std::fs::remove_file(&args.socket)?;
    }

    // Create socket directory if it doesn't exist
    if let Some(parent) = args.socket.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let listener = UnixListener::bind(&args.socket)?;
    info!("Server listening on {}", args.socket.display());

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

    // Start accepting connections
    tokio::select! {
        result = accept_connections(listener) => {
            if let Err(e) = result {
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

async fn accept_connections(listener: UnixListener) -> Result<()> {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream).await {
                        error!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
                break;
            }
        }
    }
    Ok(())
}

async fn handle_client(mut stream: UnixStream) -> Result<()> {
    let mut buffer = String::new();
    
    loop {
        buffer.clear();
        match stream.read_to_string(&mut buffer).await {
            Ok(0) => break, // Client disconnected
            Ok(_) => {
                for line in buffer.lines() {
                    if let Err(e) = process_request(line, &mut stream).await {
                        error!("Error processing request: {}", e);
                        let error_msg = format!("ERROR:{}\n", e);
                        let _ = stream.write_all(error_msg.as_bytes()).await;
                    }
                }
            }
            Err(e) => {
                error!("Error reading from socket: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn process_request(line: &str, stream: &mut UnixStream) -> Result<()> {
    if line.starts_with("REQUEST:") {
        let method = &line[8..]; // Remove "REQUEST:" prefix
        
        let response = match method {
            "ping" => "pong".to_string(),
            "status" => "server_running".to_string(),
            "version" => "paws_client_server_demo".to_string(),
            "shutdown" => {
                let response = "OK:shutting_down\n".to_string();
                stream.write_all(response.as_bytes()).await?;
                std::process::exit(0);
            }
            _ => format!("unknown_command: {}", method),
        };
        
        let response = format!("OK:{}\n", response);
        stream.write_all(response.as_bytes()).await?;
    }
    
    Ok(())
}