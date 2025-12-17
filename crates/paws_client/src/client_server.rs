use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use anyhow::{Context, Result};
use futures::stream::BoxStream;
use paws_common::stream::MpscStream;
use paws_domain::*;
use paws_api::{API, *};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

/// Simple client-server IPC using JSON messages over Unix domain sockets
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum IpcMessage {
    Request { id: u64, method: String, params: serde_json::Value },
    Response { 
        id: u64, 
        result: serde_json::Value, // Use a wrapper to handle Result serialization
    },
    Stream { 
        id: u64, 
        data: serde_json::Value, // Use a wrapper to handle Result serialization
    },
}

/// Client implementation that communicates with the server
pub struct Client {
    socket_path: PathBuf,
    server_process: Option<std::process::Child>,
}

impl Client {
    pub fn new(socket_path: Option<PathBuf>) -> Self {
        let default_socket = default_socket_path();
        Self {
            socket_path: socket_path.unwrap_or(default_socket),
            server_process: None,
        }
    }

    /// Start the server if it's not already running
    pub fn ensure_server_running(&mut self) -> Result<()> {
        if !self.socket_path.exists() {
            info!("Server not running, starting it...");
            self.start_server_process()?;
        }
        Ok(())
    }

    fn start_server_process(&mut self) -> Result<()> {
        let current_exe = env::current_exe()
            .context("Failed to get current executable path")?;
        
        let command = Command::new(current_exe)
            .args(["server", "--socket", self.socket_path.to_str().unwrap()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to start server process")?;

        self.server_process = Some(command);

        // Wait a moment for server to start
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Check if server started successfully
        if !self.socket_path.exists() {
            return Err(anyhow::anyhow!("Server failed to start"));
        }

        Ok(())
    }

    /// Connect to the server
    pub async fn connect(&self) -> Result<UnixStream> {
        UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to server")
    }

    /// Send a request to the server and get response
    pub async fn send_request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let mut stream = self.connect().await?;
        
        let request = IpcMessage::Request {
            id: 1, // Simple ID for now
            method: method.to_string(),
            params,
        };

        let message_str = serde_json::to_string(&request)?;
        stream.write_all(format!("{}\n", message_str).as_bytes()).await?;

        let mut response_buffer = String::new();
        stream.read_to_string(&mut response_buffer).await?;

        let response: IpcMessage = serde_json::from_str(&response_buffer)?;
        
        match response {
            IpcMessage::Response { result, .. } => {
                // Check if result is an error string
                if let Some(error_msg) = result.as_str() {
                    if error_msg.starts_with("Error: ") {
                        return Err(anyhow::anyhow!("Server error: {}", error_msg));
                    }
                }
                Ok(result)
            }
            _ => Err(anyhow::anyhow!("Invalid response format")),
        }
    }
}

/// Server that handles client connections
pub struct Server {
    api: Arc<dyn API>,
    socket_path: PathBuf,
}

impl Server {
    pub fn new(api: Arc<dyn API>, socket_path: PathBuf) -> Self {
        Self { api, socket_path }
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
        info!("Server listening on {}", self.socket_path.display());

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let api = self.api.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, api).await {
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

    async fn handle_client(mut stream: UnixStream, api: Arc<dyn API>) -> Result<()> {
        let mut buffer = String::new();
        
        loop {
            buffer.clear();
            match stream.read_to_string(&mut buffer).await {
                Ok(0) => break, // Client disconnected
                Ok(_) => {
                    for line in buffer.lines() {
                        if let Ok(message) = serde_json::from_str::<IpcMessage>(line) {
                            if let Err(e) = Self::process_message(&message, &api, &mut stream).await {
                                error!("Error processing message: {}", e);
                            }
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

    async fn process_message(
        message: &IpcMessage,
        api: &Arc<dyn API>,
        stream: &mut UnixStream,
    ) -> Result<()> {
        match message {
            IpcMessage::Request { id, method, params } => {
                let result = match method.as_str() {
                    "discover" => Ok(serde_json::to_value(api.discover().await?)?),
                    "get_tools" => Ok(serde_json::to_value(api.get_tools().await?)?),
                    "get_models" => Ok(serde_json::to_value(api.get_models().await?)?),
                    "get_agents" => Ok(serde_json::to_value(api.get_agents().await?)?),
                    "get_providers" => Ok(serde_json::to_value(api.get_providers().await?)?),
                    "chat" => {
                        let chat_request: ChatRequest = serde_json::from_value(params.clone())?;
                        let _response = api.chat(chat_request).await?;
                        Ok(serde_json::to_value("chat_started")?)
                    }
                    _ => Err(format!("Unknown method: {}", method)),
                };

                let response = IpcMessage::Response {
                    id: *id,
                    result: match result {
                        Ok(value) => value,
                        Err(error) => serde_json::to_value(format!("Error: {}", error))?,
                    },
                };

                let response_str = serde_json::to_string(&response)?;
                stream.write_all(format!("{}\n", response_str).as_bytes()).await?;
            }
            _ => {}
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