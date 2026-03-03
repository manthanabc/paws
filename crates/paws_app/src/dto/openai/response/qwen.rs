use serde::{Deserialize, Serialize};
use serde::de::{self, Deserializer};

use super::{FunctionType, ReasoningDetail, ToolCallId, ToolName};
use crate::dto::openai::error::{Error, ErrorCode, ErrorResponse};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QwenResponse {
    pub id: String,
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    pub choices: Vec<QwenChoice>,
    pub created: u64,
    pub object: Option<String>,
    pub system_fingerprint: Option<String>,
    pub usage: Option<super::ResponseUsage>,
    #[serde(default)]
    pub prompt_filter_results: Option<Vec<super::PromptFilterResult>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum QwenChoice {
    NonChat {
        finish_reason: Option<String>,
        text: String,
        error: Option<ErrorResponse>,
    },
    NonStreaming {
        logprobs: Option<serde_json::Value>,
        index: u32,
        finish_reason: Option<String>,
        message: QwenResponseMessage,
        error: Option<ErrorResponse>,
    },
    Streaming {
        finish_reason: Option<String>,
        delta: QwenResponseMessage,
        error: Option<ErrorResponse>,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QwenResponseMessage {
    pub content: Option<String>,
    #[serde(alias = "reasoning_content")]
    pub reasoning: Option<String>,
    pub role: Option<String>,
    pub tool_calls: Option<Vec<QwenToolCall>>,
    pub refusal: Option<String>,
    pub reasoning_details: Option<Vec<ReasoningDetail>>,
    // GitHub Copilot format (flat fields instead of array)
    pub reasoning_text: Option<String>,
    pub reasoning_opaque: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QwenToolCall {
    pub id: Option<ToolCallId>,
    pub r#type: FunctionType,
    pub function: QwenFunctionCall,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct QwenFunctionCall {
    // Only the first event typically has the name of the function call
    pub name: Option<ToolName>,
    // Qwen expects arguments as a JSON object rather than a string
    // For responses, this deserializes from either string or object format
    // For requests, this serializes directly as JSON object
    #[serde(deserialize_with = "deserialize_qwen_arguments")]
    pub arguments: serde_json::Value,
}

// Custom deserializer to handle both string and object formats for Qwen arguments
fn deserialize_qwen_arguments<'de, D>(deserializer: D) -> Result<serde_json::Value, D::Error>
where
    D: Deserializer<'de>,
{
    use serde_json::Value;
    
    let value = Value::deserialize(deserializer)?;
    
    match value {
        Value::String(s) => {
            // If it's a string, try to parse it as JSON
            serde_json::from_str(&s).map_err(de::Error::custom)
        }
        Value::Object(_) | Value::Array(_) => {
            // Already an object or array, return as-is
            Ok(value)
        }
        _ => {
            // For other types, just return as-is
            Ok(value)
        }
    }
}

use paws_domain::{
    ChatCompletionMessage, Content, FinishReason, ToolCallFull, ToolCallPart,
};
use std::str::FromStr;

impl TryFrom<QwenResponse> for ChatCompletionMessage {
    type Error = anyhow::Error;

    fn try_from(res: QwenResponse) -> Result<Self, Self::Error> {
        match res {
            QwenResponse { choices, usage, prompt_filter_results, .. } => {
                if let Some(choice) = choices.first() {
                    // Check if the choice has an error first
                    let error = match choice {
                        QwenChoice::NonChat { error, .. } => error,
                        QwenChoice::NonStreaming { error, .. } => error,
                        QwenChoice::Streaming { error, .. } => error,
                    };

                    if let Some(error) = error {
                        return Err(Error::Response(error.clone()).into());
                    }

                    let mut response = match choice {
                        QwenChoice::NonChat { text, finish_reason, .. } => {
                            ChatCompletionMessage::assistant(Content::full(text)).finish_reason_opt(
                                finish_reason
                                    .clone()
                                    .and_then(|s| FinishReason::from_str(&s).ok()),
                            )
                        }
                        QwenChoice::NonStreaming { message, finish_reason, .. } => {
                            let mut resp = ChatCompletionMessage::assistant(Content::full(
                                message.content.clone().unwrap_or_default(),
                            ))
                            .finish_reason_opt(
                                finish_reason
                                    .clone()
                                    .and_then(|s| FinishReason::from_str(&s).ok()),
                            );
                            if let Some(reasoning) = &message.reasoning {
                                resp = resp.reasoning(Content::full(reasoning.clone()));
                            }

                            if let Some(reasoning_details) = &message.reasoning_details {
                                let converted_details: Vec<paws_domain::ReasoningFull> =
                                    reasoning_details
                                        .clone()
                                        .into_iter()
                                        .map(paws_domain::ReasoningFull::from)
                                        .collect();

                                resp = resp.add_reasoning_detail(paws_domain::Reasoning::Full(
                                    converted_details,
                                ));
                            }

                            if let Some(tool_calls) = &message.tool_calls {
                                for tool_call in tool_calls {
                                    resp = resp.add_tool_call(ToolCallFull {
                                        call_id: tool_call.id.clone(),
                                        name: tool_call
                                            .function
                                            .name
                                            .clone()
                                            .ok_or(paws_domain::Error::ToolCallMissingName)?,
                                        arguments: tool_call.function.arguments.clone().into(),
                                        timestamp: None,
                                        cwd: None,
                                    });
                                }
                            }
                            resp
                        }
                        QwenChoice::Streaming { delta, finish_reason, .. } => {
                            let mut resp = ChatCompletionMessage::assistant(Content::part(
                                delta.content.clone().unwrap_or_default(),
                            ))
                            .finish_reason_opt(
                                finish_reason
                                    .clone()
                                    .and_then(|s| FinishReason::from_str(&s).ok()),
                            );

                            if let Some(reasoning) = &delta.reasoning {
                                resp = resp.reasoning(Content::part(reasoning.clone()));
                            }

                            if let Some(reasoning_details) = &delta.reasoning_details {
                                let converted_details: Vec<paws_domain::ReasoningPart> =
                                    reasoning_details
                                        .clone()
                                        .into_iter()
                                        .map(paws_domain::ReasoningPart::from)
                                        .collect();
                                resp = resp.add_reasoning_detail(paws_domain::Reasoning::Part(
                                    converted_details,
                                ));
                            }

                            if let Some(tool_calls) = &delta.tool_calls {
                                for tool_call in tool_calls {
                                    resp = resp.add_tool_call(ToolCallPart {
                                        call_id: tool_call.id.clone(),
                                        name: tool_call.function.name.clone(),
                                        arguments_part: tool_call.function.arguments.to_string(),
                                    });
                                }
                            }
                            resp
                        }
                    };

                    if let Some(usage) = usage {
                        response.usage = Some(usage.into());
                    }
                    Ok(response)
                } else {
                    // Check if content was filtered
                    if let Some(filter_results) = prompt_filter_results
                        && let Some(filter_result) = filter_results.first()
                    {
                        let filtered_categories: Vec<String> = [
                            filter_result
                                .content_filter_results
                                .hate
                                .as_ref()
                                .filter(|f| f.filtered)
                                .map(|_| "hate"),
                            filter_result
                                .content_filter_results
                                .self_harm
                                .as_ref()
                                .filter(|f| f.filtered)
                                .map(|_| "self_harm"),
                            filter_result
                                .content_filter_results
                                .sexual
                                .as_ref()
                                .filter(|f| f.filtered)
                                .map(|_| "sexual"),
                            filter_result
                                .content_filter_results
                                .violence
                                .as_ref()
                                .filter(|f| f.filtered)
                                .map(|_| "violence"),
                        ]
                        .into_iter()
                        .flatten()
                        .map(String::from)
                        .collect();

                        if !filtered_categories.is_empty() {
                            let error = ErrorResponse::default()
                                .message(format!(
                                    "Content was filtered due to: {}",
                                    filtered_categories.join(", ")
                                ))
                                .code(ErrorCode::String("content_filter".to_string()));
                            return Err(Error::Response(error).into());
                        }
                    }

                    let mut default_response = ChatCompletionMessage::assistant(Content::full(""));
                    // No choices – this can happen with Ollama/LMStudio streaming where the final
                    // chunk only contains usage information.
                    if let Some(u) = usage {
                        default_response.usage = Some(u.into());
                    }
                    Ok(default_response)
                }
            }
        }
    }
}