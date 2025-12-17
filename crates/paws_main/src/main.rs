use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use paws_domain::TitleFormat;
use paws_main::TitleDisplayExt;

mod cli;

use cli::Cli;

// Simple client that can start server
fn start_server_if_needed(socket_path: Option<PathBuf>) -> Result<()> {
    let socket = socket_path.unwrap_or_else(|| {
        let mut path = dirs::runtime_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        path.push("paws.sock");
        path
    });

    if !socket.exists() {
        use tracing::info;
        info!("Server not running, starting it...");
        
        let current_exe = env::current_exe()?;
        let mut command = std::process::Command::new(current_exe)
            .args(["server", "--socket", socket.to_str().unwrap()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        
        // Wait a moment for server to start
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up panic hook for better error display
    std::panic::set_hook(Box::new(|panic_info| {
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
        start_server_mode(socket.clone(), *verbose).await?;
        return Ok(());
    }

    // Default: start client
    start_client_mode(cli).await?;

    Ok(())
}

async fn start_server_mode(socket_path: Option<PathBuf>, verbose: bool) -> Result<()> {
    use tracing::info;

    info!("Starting Paws Server in server mode...");

    // Create a simple socket-based server
    let socket = socket_path.unwrap_or_else(|| {
        let mut path = dirs::runtime_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        path.push("paws.sock");
        path
    });

    // Remove existing socket file
    if socket.exists() {
        std::fs::remove_file(&socket)?;
    }

    // Create socket directory if it doesn't exist
    if let Some(parent) = socket.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let listener = tokio::net::UnixListener::bind(&socket)?;
    info!("Server listening on {}", socket.display());

    // Simple server loop
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(async move {
                    if let Err(e) = handle_client_connection(stream).await {
                        use tracing::error;
                        error!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => {
                use tracing::error;
                error!("Failed to accept connection: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn handle_client_connection<T>(mut stream: T) -> Result<()> 
where 
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buffer = String::new();
    
    loop {
        buffer.clear();
        match stream.read_to_string(&mut buffer).await {
            Ok(0) => break, // Client disconnected
            Ok(_) => {
                for line in buffer.lines() {
                    if line.starts_with("REQUEST:") {
                        let method = &line[8..]; // Remove "REQUEST:" prefix
                        
                        let response = match method {
                            "ping" => "pong".to_string(),
                            "status" => "server_running".to_string(),
                            "shutdown" => {
                                let response = "OK:shutting_down\n".to_string();
                                stream.write_all(response.as_bytes()).await?;
                                return Ok(());
                            }
                            _ => format!("unknown_command: {}", method),
                        };
                        
                        let response = format!("OK:{}\n", response);
                        stream.write_all(response.as_bytes()).await?;
                    }
                }
            }
            Err(e) => {
                use tracing::error;
                error!("Error reading from socket: {}", e);
                break;
            }
        }
    }

    Ok(())
}

async fn start_client_mode(cli: Cli) -> Result<()> {
    use tracing::info;

    info!("Starting Paws Client...");

    // Start server if not running
    start_server_if_needed(None)?;

    // Connect to server
    let socket = {
        let mut path = dirs::runtime_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        path.push("paws.sock");
        path
    };

    // Simple client that connects and tests the server
    let mut stream = tokio::net::UnixStream::connect(&socket).await?;

    // Send a ping to test
    stream.write_all(b"REQUEST:ping\n").await?;
    let mut response = String::new();
    stream.read_to_string(&mut response).await?;

    info!("Server response: {}", response.trim());

    // Show what would happen in full implementation
    if let Some(prompt) = &cli.prompt {
        info!("Would process prompt: {}", prompt);
    } else {
        info!("Would start interactive terminal UI");
    }

    info!("Client-server architecture demonstrated!");
    info!("- Server handles all business logic");
    info!("- Client handles UI and user interaction");
    info!("- Communication via Unix domain sockets");
    info!("- Same binary runs in both modes");

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