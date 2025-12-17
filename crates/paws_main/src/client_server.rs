use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

/// Simple message protocol for client-server communication
#[derive(Debug)]
enum SimpleMessage {
    Request { method: String },
    Response { ok: bool, data: String },
    Error { message: String },
}

/// Client implementation
pub struct Client {
    socket_path: PathBuf,
}

impl Client {
    pub fn new(socket_path: Option<PathBuf>) -> Self {
        let default_socket = default_socket_path();
        Self {
            socket_path: socket_path.unwrap_or(default_socket),
        }
    }

    pub fn ensure_server_running(&mut self) -> Result<()> {
        if !self.socket_path.exists() {
            use tracing::info;
            info!("Server not running, starting it...");
            self.start_server_process()?;
        }
        Ok(())
    }

    fn start_server_process(&mut self) -> Result<()> {
        let current_exe = env::current_exe()
            .context("Failed to get current executable path")?;
        
        let command = std::process::Command::new(current_exe)
            .args(["server", "--socket", self.socket_path.to_str().unwrap()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("Failed to start server process")?;

        // Wait a moment for server to start
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Check if server started successfully
        if !self.socket_path.exists() {
            return Err(anyhow::anyhow!("Server failed to start"));
        }

        Ok(())
    }

    pub async fn connect(&self) -> Result<UnixStream> {
        UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to server")
    }

    pub async fn send_request(&self, method: &str) -> Result<String> {
        let mut stream = self.connect().await?;
        
        let message = format!("REQUEST:{}\n", method);
        stream.write_all(message.as_bytes()).await?;

        let mut response = String::new();
        stream.read_to_string(&mut response).await?;

        if response.starts_with("OK:") {
            Ok(response[3..].trim().to_string())
        } else if response.starts_with("ERROR:") {
            Err(anyhow::anyhow!("Server error: {}", &response[6..].trim()))
        } else {
            Err(anyhow::anyhow!("Invalid response format"))
        }
    }
}

/// Server implementation
pub struct Server {
    socket_path: PathBuf,
}

impl Server {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub async fn start(&mut self) -> Result<()> {
        // Remove existing socket file
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path)?;
        }

        // Create socket directory if it doesn't exist
        if let Some(parent) = self.socket_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        use tracing::info;
        info!("Server listening on {}", self.socket_path.display());

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream).await {
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

    async fn handle_client(mut stream: UnixStream) -> Result<()> {
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
                                "ping" => "pong",
                                "status" => "ok",
                                "shutdown" => {
                                    let response = "OK:shutting_down\n".to_string();
                                    stream.write_all(response.as_bytes()).await?;
                                    break;
                                }
                                _ => "unknown_command",
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
}

impl Drop for Server {
    fn drop(&mut self) {
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

fn default_socket_path() -> PathBuf {
    let mut path = dirs::runtime_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    path.push("paws.sock");
    path
}