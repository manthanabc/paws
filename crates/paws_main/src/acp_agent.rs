use std::sync::Arc;

use agent_client_protocol::{self as acp, Client as _};
use anyhow::Result;
use paws_api::{API, ChatRequest, ChatResponse, ConversationId, Event, TextMessage, UserPrompt};
use paws_domain::ChatResponseContent;
use serde_json::json;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt;
use tokio_util::compat::{TokioAsyncReadCompatExt as _, TokioAsyncWriteCompatExt as _};

/// Paws Agent implementation for Agent Client Protocol
///
/// This agent bridges the Paws API with the Agent Client Protocol,
/// allowing Paws to be controlled via the ACP standard.
pub struct PawsAcpAgent<A: API> {
    api: Arc<A>,
    session_update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
}

impl<A: API> PawsAcpAgent<A> {
    /// Creates a new Paws ACP Agent
    ///
    /// # Arguments
    /// * `api` - The Paws API instance to use for executing prompts
    /// * `session_update_tx` - Channel for sending session notifications back to the client
    pub fn new(
        api: Arc<A>,
        session_update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    ) -> Self {
        Self {
            api,
            session_update_tx,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl<A: API + 'static> acp::Agent for PawsAcpAgent<A> {
    async fn initialize(
        &self,
        _arguments: acp::InitializeRequest,
    ) -> Result<acp::InitializeResponse, acp::Error> {
        tracing::info!("Initializing Paws ACP Agent");
        Ok(acp::InitializeResponse {
            protocol_version: acp::V1,
            agent_capabilities: acp::AgentCapabilities::default(),
            auth_methods: Vec::new(),
            agent_info: Some(acp::Implementation {
                name: "paws".to_string(),
                title: Some("Paws AI Agent".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            }),
            meta: None,
        })
    }

    async fn authenticate(
        &self,
        _arguments: acp::AuthenticateRequest,
    ) -> Result<acp::AuthenticateResponse, acp::Error> {
        tracing::info!("ACP authenticate request (no-op)");
        Ok(acp::AuthenticateResponse::default())
    }

    async fn new_session(
        &self,
        _arguments: acp::NewSessionRequest,
    ) -> Result<acp::NewSessionResponse, acp::Error> {
        let conversation = paws_api::Conversation::generate();
        let session_id = conversation.id.into_string();

        self.api
            .upsert_conversation(conversation)
            .await
            .map_err(|e| {
                acp::Error::internal_error_with_message(format!(
                    "Failed to create conversation: {}",
                    e
                ))
            })?;

        Ok(acp::NewSessionResponse {
            session_id: acp::SessionId(session_id.into()),
            modes: None,
            #[cfg(feature = "unstable_session_model")]
            models: None,
            meta: None,
        })
    }

    async fn load_session(
        &self,
        arguments: acp::LoadSessionRequest,
    ) -> Result<acp::LoadSessionResponse, acp::Error> {
        let conversation_id = ConversationId::parse(arguments.session_id.0.to_string())
            .map_err(|e| acp::Error::invalid_params(format!("Invalid session ID: {}", e)))?;

        // Ensure conversation exists - create it if it doesn't.
        let conversation_exists = self
            .api
            .conversation(&conversation_id)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, "Failed to check conversation");
                acp::Error::internal_error()
            })?
            .is_some();

        if !conversation_exists {
            let conversation = paws_api::Conversation::new(conversation_id);
            self.api
                .upsert_conversation(conversation)
                .await
                .map_err(|e| {
                    tracing::error!(error = ?e, "Failed to create conversation");
                    acp::Error::internal_error()
                })?;
        }

        Ok(acp::LoadSessionResponse {
            modes: None,
            #[cfg(feature = "unstable_session_model")]
            models: None,
            meta: None,
        })
    }

    async fn prompt(
        &self,
        arguments: acp::PromptRequest,
    ) -> Result<acp::PromptResponse, acp::Error> {
        // Convert ACP session ID to Paws conversation ID
        let conversation_id = ConversationId::parse(arguments.session_id.0.to_string())
            .map_err(|e| acp::Error::invalid_params(format!("Invalid session ID: {}", e)))?;

        // Convert ACP prompt content to Paws UserPrompt
        let prompt_text = arguments
            .prompt
            .iter()
            .map(|content| match content {
                acp::Content::Text(text) => text.text.as_str(),
                acp::Content::Image(_) => "[Image content]",
                acp::Content::Document(_) => "[Document content]",
            })
            .collect::<Vec<_>>()
            .join("\n");

        let user_prompt = UserPrompt::from(prompt_text);
        let event = Event::Message(TextMessage::User(user_prompt));
        let chat_request = ChatRequest::new(event, conversation_id);

        // Execute the chat request and stream responses
        let mut stream = self
            .api
            .chat(chat_request)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, "Chat failed");
                acp::Error::internal_error()
            })?;

        // Stream responses back to the client
        while let Some(response) = stream.next().await {
            let response = response.map_err(|e| {
                tracing::error!(error = ?e, "Stream error");
                acp::Error::internal_error()
            })?;

            let content = match response {
                ChatResponse::TaskMessage { content } => match content {
                    ChatResponseContent::PlainText(text) => text,
                    ChatResponseContent::Markdown(text) => text,
                    ChatResponseContent::Title(title) => title.title,
                },
                ChatResponse::TaskReasoning { content } => content,
                ChatResponse::TaskComplete => continue,
                ChatResponse::ToolCallStart(_) => continue,
                ChatResponse::ToolCallEnd(_) => continue,
                ChatResponse::RetryAttempt { .. } => continue,
                ChatResponse::Interrupt { .. } => {
                    return Ok(acp::PromptResponse {
                        stop_reason: acp::StopReason::EndTurn,
                        meta: None,
                    });
                }
            };

            if !content.is_empty() {
                let (tx, rx) = oneshot::channel();
                self.session_update_tx
                    .send((
                        acp::SessionNotification {
                            session_id: arguments.session_id.clone(),
                            update: acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk {
                                content: acp::Content::Text(acp::TextContent {
                                    text: content,
                                    annotations: None,
                                }),
                                meta: None,
                            }),
                            meta: None,
                        },
                        tx,
                    ))
                    .map_err(|_| acp::Error::internal_error())?;
                rx.await.map_err(|_| acp::Error::internal_error())?;
            }
        }

        Ok(acp::PromptResponse {
            stop_reason: acp::StopReason::EndTurn,
            meta: None,
        })
    }

    async fn cancel(&self, _args: acp::CancelNotification) -> Result<(), acp::Error> {
        tracing::info!("Received ACP cancel request");
        Ok(())
    }

    async fn set_session_mode(
        &self,
        _args: acp::SetSessionModeRequest,
    ) -> Result<acp::SetSessionModeResponse, acp::Error> {
        tracing::info!("Received ACP set session mode request");
        Ok(acp::SetSessionModeResponse::default())
    }

    #[cfg(feature = "unstable_session_model")]
    async fn set_session_model(
        &self,
        _args: acp::SetSessionModelRequest,
    ) -> Result<acp::SetSessionModelResponse, acp::Error> {
        tracing::info!("Received ACP set session model request");
        Ok(acp::SetSessionModelResponse::default())
    }

    #[cfg(feature = "unstable_session_config_options")]
    async fn set_session_config_option(
        &self,
        _args: acp::SetSessionConfigOptionRequest,
    ) -> Result<acp::SetSessionConfigOptionResponse, acp::Error> {
        tracing::info!("Received ACP set session config option request");
        Ok(acp::SetSessionConfigOptionResponse::default())
    }

    async fn ext_method(&self, args: acp::ExtRequest) -> Result<acp::ExtResponse, acp::Error> {
        tracing::info!(
            "Received ACP extension method call: method={}, params={:?}",
            args.method,
            args.params
        );
        Ok(serde_json::value::to_raw_value(&json!({"status": "ok"}))?.into())
    }

    async fn ext_notification(&self, args: acp::ExtNotification) -> Result<(), acp::Error> {
        tracing::info!(
            "Received ACP extension notification: method={}, params={:?}",
            args.method,
            args.params
        );
        Ok(())
    }
}

/// Runs the Paws Agent in ACP server mode
///
/// This function sets up the ACP connection over stdio and handles
/// session notifications in the background.
///
/// # Arguments
/// * `api` - The Paws API instance to use
///
/// # Errors
/// Returns an error if the ACP connection fails
pub async fn run_acp_server<A: API + 'static>(api: Arc<A>) -> Result<()> {
    let outgoing = tokio::io::stdout().compat_write();
    let incoming = tokio::io::stdin().compat();

    // The AgentSideConnection will spawn futures onto our Tokio runtime.
    // LocalSet and spawn_local are used because the futures from the
    // agent-client-protocol crate are not Send.
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            
            // Start up the PawsAcpAgent connected to stdio.
            let agent = PawsAcpAgent::new(api, tx);
            let (conn, handle_io) =
                acp::AgentSideConnection::new(agent, outgoing, incoming, |fut| {
                    tokio::task::spawn_local(fut);
                });
            
            // Kick off a background task to send session notifications to the client.
            tokio::task::spawn_local(async move {
                while let Some((session_notification, tx)) = rx.recv().await {
                    let result = conn.session_notification(session_notification).await;
                    if let Err(e) = result {
                        tracing::error!("Failed to send session notification: {}", e);
                        break;
                    }
                    tx.send(()).ok();
                }
            });
            
            // Run until stdin/stdout are closed.
            handle_io.await
        })
        .await
        .map_err(|e| anyhow::anyhow!("ACP server error: {}", e))
}
