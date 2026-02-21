//! Response types for Gemini's native generateContent API.
//!
//! Gemini streaming responses come as a series of JSON objects,
//! each containing partial content in the `candidates` array.

use paws_domain::{
    ChatCompletionMessage, Content as DomainContent, FinishReason, ModelId, TokenCount, ToolCallId,
    ToolCallPart, ToolName,
};
use serde::Deserialize;

use super::request::Content;

/// Streaming response from Gemini's generateContent API
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub candidates: Option<Vec<Candidate>>,
    #[serde(default)]
    pub usage_metadata: Option<UsageMetadata>,
    /// Error information if the request failed
    pub error: Option<ErrorResponse>,
}

/// A candidate response
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub content: Option<Content>,
    pub finish_reason: Option<GeminiFinishReason>,
    #[serde(default)]
    pub index: u32,
    pub safety_ratings: Option<Vec<SafetyRating>>,
}

/// Finish reason from Gemini
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GeminiFinishReason {
    Stop,
    MaxTokens,
    Safety,
    Recitation,
    Other,
    FinishReasonUnspecified,
    Blocklist,
    ProhibitedContent,
    Spii,
    MalformedFunctionCall,
}

impl From<GeminiFinishReason> for FinishReason {
    fn from(reason: GeminiFinishReason) -> Self {
        match reason {
            GeminiFinishReason::Stop => FinishReason::Stop,
            GeminiFinishReason::MaxTokens => FinishReason::Length,
            GeminiFinishReason::Safety
            | GeminiFinishReason::Recitation
            | GeminiFinishReason::Blocklist
            | GeminiFinishReason::ProhibitedContent
            | GeminiFinishReason::Spii => FinishReason::ContentFilter,
            GeminiFinishReason::MalformedFunctionCall => FinishReason::ToolCalls,
            GeminiFinishReason::Other | GeminiFinishReason::FinishReasonUnspecified => {
                FinishReason::Stop
            }
        }
    }
}

/// Safety rating information
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyRating {
    pub category: String,
    pub probability: String,
    #[serde(default)]
    pub blocked: bool,
}

/// Usage metadata from the response
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    #[serde(default)]
    pub prompt_token_count: usize,
    #[serde(default)]
    pub candidates_token_count: usize,
    #[serde(default)]
    pub total_token_count: usize,
    #[serde(default)]
    pub cached_content_token_count: Option<usize>,
}

impl From<UsageMetadata> for paws_domain::Usage {
    fn from(usage: UsageMetadata) -> Self {
        Self {
            prompt_tokens: TokenCount::Actual(usage.prompt_token_count),
            completion_tokens: TokenCount::Actual(usage.candidates_token_count),
            total_tokens: TokenCount::Actual(usage.total_token_count),
            cached_tokens: usage
                .cached_content_token_count
                .map(TokenCount::Actual)
                .unwrap_or_default(),
            ..Default::default()
        }
    }
}

/// Error response from Gemini
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub code: Option<i32>,
    pub message: String,
    pub status: Option<String>,
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ErrorResponse {}

/// Model information from Gemini's models.list API
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub input_token_limit: Option<u64>,
    pub output_token_limit: Option<u64>,
    pub supported_generation_methods: Option<Vec<String>>,
}

impl From<ModelInfo> for paws_domain::Model {
    fn from(model: ModelInfo) -> Self {
        // Extract model ID from full name (e.g., "models/gemini-pro" -> "gemini-pro")
        let id = model
            .name
            .strip_prefix("models/")
            .unwrap_or(&model.name)
            .to_string();

        Self {
            id: ModelId::new(id),
            name: model.display_name,
            description: model.description,
            context_length: model.input_token_limit,
            tools_supported: Some(true),
            supports_parallel_tool_calls: Some(true),
            supports_reasoning: None,
            input_modalities: vec![],
        }
    }
}

/// Response from Gemini's models.list API
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListModelsResponse {
    pub models: Vec<ModelInfo>,
    pub next_page_token: Option<String>,
}

impl TryFrom<Response> for ChatCompletionMessage {
    type Error = anyhow::Error;

    fn try_from(response: Response) -> Result<Self, Self::Error> {
        // Check for errors first
        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("{}", error.message));
        }

        let candidates = response.candidates.unwrap_or_default();

        if let Some(candidate) = candidates.first() {
            let mut message = ChatCompletionMessage::assistant(DomainContent::part(""));

            // Set finish reason if present
            if let Some(ref finish_reason) = candidate.finish_reason {
                message = message.finish_reason(finish_reason.clone());
            }

            // Process content parts
            if let Some(ref content) = candidate.content {
                let mut text_content = String::new();
                let mut has_tool_calls = false;

                for part in &content.parts {
                    // Handle text content
                    if let Some(ref text) = part.text {
                        text_content.push_str(text);
                    }

                    // Handle function calls
                    if let Some(ref function_call) = part.function_call {
                        has_tool_calls = true;
                        message = message.add_tool_call(ToolCallPart {
                            call_id: Some(ToolCallId::new(generate_tool_call_id(
                                &function_call.name,
                            ))),
                            name: Some(ToolName::new(&function_call.name)),
                            arguments_part: serde_json::to_string(&function_call.args)
                                .unwrap_or_default(),
                        });
                    }
                }

                // Set text content
                message = ChatCompletionMessage::assistant(DomainContent::part(text_content));

                // Re-add tool calls if any
                if has_tool_calls {
                    if let Some(ref content) = candidate.content {
                        for part in &content.parts {
                            if let Some(ref function_call) = part.function_call {
                                message = message.add_tool_call(ToolCallPart {
                                    call_id: Some(ToolCallId::new(generate_tool_call_id(
                                        &function_call.name,
                                    ))),
                                    name: Some(ToolName::new(&function_call.name)),
                                    arguments_part: serde_json::to_string(&function_call.args)
                                        .unwrap_or_default(),
                                });
                            }
                        }
                    }

                    // Set finish reason for tool calls
                    message = message.finish_reason(FinishReason::ToolCalls);
                }

                // Re-set finish reason if it was overwritten
                if let Some(ref finish_reason) = candidate.finish_reason
                    && !has_tool_calls
                {
                    message = message.finish_reason(finish_reason.clone());
                }
            }

            // Add usage if present
            if let Some(usage) = response.usage_metadata {
                message.usage = Some(usage.into());
            }

            Ok(message)
        } else {
            // No candidates - return empty message with usage if available
            let mut message = ChatCompletionMessage::assistant(DomainContent::part(""));
            if let Some(usage) = response.usage_metadata {
                message.usage = Some(usage.into());
            }
            Ok(message)
        }
    }
}

/// Generates a unique tool call ID based on function name and a simple counter
fn generate_tool_call_id(function_name: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("call_{}_{}", function_name, id)
}
