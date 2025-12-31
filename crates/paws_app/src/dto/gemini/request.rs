//! Request types for Gemini's native generateContent API.
//!
//! Gemini uses a different format from OpenAI:
//! - POST /v1beta/models/{model}:streamGenerateContent
//! - Uses `contents` array with `parts`
//! - Tool calling uses `functionCall` and `functionResponse`

use derive_setters::Setters;
use paws_domain::ContextMessage;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Request body for Gemini's generateContent API
#[derive(Debug, Serialize, Default, Setters)]
#[serde(rename_all = "camelCase")]
#[setters(into, strip_option)]
pub struct Request {
    pub contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<ToolConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
}

/// Represents a content block with role and parts
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    pub parts: Vec<Part>,
}

/// Role in the conversation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Model,
}

/// Part of a content block - can be text, inline data, or function
/// call/response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_data: Option<InlineData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_response: Option<FunctionResponse>,
}

impl Part {
    /// Creates a text part
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            inline_data: None,
            function_call: None,
            function_response: None,
        }
    }

    /// Creates an inline data part for images
    pub fn inline_data(mime_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            text: None,
            inline_data: Some(InlineData { mime_type: mime_type.into(), data: data.into() }),
            function_call: None,
            function_response: None,
        }
    }

    /// Creates a function call part
    pub fn function_call(name: impl Into<String>, args: serde_json::Value) -> Self {
        Self {
            text: None,
            inline_data: None,
            function_call: Some(FunctionCall { name: name.into(), args }),
            function_response: None,
        }
    }

    /// Creates a function response part
    pub fn function_response(name: impl Into<String>, response: serde_json::Value) -> Self {
        Self {
            text: None,
            inline_data: None,
            function_call: None,
            function_response: Some(FunctionResponse { name: name.into(), response }),
        }
    }
}

/// Inline data for images/media
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineData {
    pub mime_type: String,
    pub data: String,
}

/// Function call from the model
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCall {
    pub name: String,
    #[serde(default)]
    pub args: serde_json::Value,
}

/// Function response from the user
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}

/// Tool definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub function_declarations: Vec<FunctionDeclaration>,
}

/// Function declaration for tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_calling_config: Option<FunctionCallingConfig>,
}

/// Function calling mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCallingConfig {
    pub mode: FunctionCallingMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_function_names: Option<Vec<String>>,
}

/// Function calling mode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FunctionCallingMode {
    Auto,
    Any,
    None,
}

/// Generation configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

impl TryFrom<paws_domain::Context> for Request {
    type Error = anyhow::Error;

    fn try_from(context: paws_domain::Context) -> Result<Self, Self::Error> {
        // Extract system message as system_instruction
        let system_instruction = context
            .messages
            .iter()
            .filter_map(|msg| match &**msg {
                ContextMessage::Text(text_msg) if text_msg.has_role(paws_domain::Role::System) => {
                    Some(Content {
                        role: None, // System instruction doesn't have a role in Gemini
                        parts: vec![Part::text(&text_msg.content)],
                    })
                }
                _ => None,
            })
            .next();

        // Convert messages (excluding system messages)
        let contents = context
            .messages
            .iter()
            .filter(|msg| !msg.has_role(paws_domain::Role::System))
            .map(|msg| Content::try_from(&**msg))
            .collect::<Result<Vec<_>, _>>()?;

        // Convert tools
        let tools = if context.tools.is_empty() {
            None
        } else {
            let declarations: Vec<FunctionDeclaration> = context
                .tools
                .iter()
                .map(|t| {
                    let schema = serde_json::to_value(&t.input_schema).unwrap_or_default();
                    let cleaned_schema = clean_schema_for_gemini(schema);
                    FunctionDeclaration {
                        name: t.name.to_string(),
                        description: t.description.clone(),
                        parameters: Some(cleaned_schema),
                    }
                })
                .collect();
            Some(vec![Tool { function_declarations: declarations }])
        };

        // Convert tool_choice to tool_config
        let tool_config = context.tool_choice.map(|tc| {
            let mode = match tc {
                paws_domain::ToolChoice::Auto => FunctionCallingMode::Auto,
                paws_domain::ToolChoice::None => FunctionCallingMode::None,
                paws_domain::ToolChoice::Required => FunctionCallingMode::Any,
                paws_domain::ToolChoice::Call(name) => {
                    return ToolConfig {
                        function_calling_config: Some(FunctionCallingConfig {
                            mode: FunctionCallingMode::Any,
                            allowed_function_names: Some(vec![name.to_string()]),
                        }),
                    };
                }
            };
            ToolConfig {
                function_calling_config: Some(FunctionCallingConfig {
                    mode,
                    allowed_function_names: None,
                }),
            }
        });

        // Build generation config
        let generation_config = GenerationConfig {
            temperature: context.temperature.map(|t| t.value()),
            top_p: context.top_p.map(|t| t.value()),
            top_k: context.top_k.map(|t| t.value() as i32),
            max_output_tokens: context.max_tokens.map(|t| t as u32),
            stop_sequences: None,
        };

        let has_generation_config = generation_config.temperature.is_some()
            || generation_config.top_p.is_some()
            || generation_config.top_k.is_some()
            || generation_config.max_output_tokens.is_some();

        Ok(Self {
            contents,
            system_instruction,
            tools,
            tool_config,
            generation_config: if has_generation_config {
                Some(generation_config)
            } else {
                None
            },
        })
    }
}

impl TryFrom<&ContextMessage> for Content {
    type Error = anyhow::Error;

    fn try_from(msg: &ContextMessage) -> Result<Self, Self::Error> {
        match msg {
            ContextMessage::Text(text_msg) => {
                let role = match text_msg.role {
                    paws_domain::Role::User => Role::User,
                    paws_domain::Role::Assistant => Role::Model,
                    paws_domain::Role::System => {
                        // System messages should be filtered out before this conversion
                        return Err(anyhow::anyhow!(
                            "System messages should be converted to system_instruction"
                        ));
                    }
                };

                let mut parts = Vec::new();

                // Add text content if non-empty
                if !text_msg.content.is_empty() {
                    parts.push(Part::text(&text_msg.content));
                }

                // Add tool calls if present (assistant messages with tool_calls)
                if let Some(ref tool_calls) = text_msg.tool_calls {
                    for tool_call in tool_calls {
                        let args = tool_call
                            .arguments
                            .clone()
                            .parse()
                            .unwrap_or(serde_json::Value::Object(Default::default()));
                        parts.push(Part::function_call(tool_call.name.to_string(), args));
                    }
                }

                // If no parts were added, add an empty text part
                if parts.is_empty() {
                    parts.push(Part::text(""));
                }

                Ok(Content { role: Some(role), parts })
            }
            ContextMessage::Tool(tool_result) => {
                // Tool results come from the user in Gemini's model
                let response = tool_result
                    .output
                    .as_str()
                    .map(|s| serde_json::json!({ "output": s }))
                    .unwrap_or_else(|| {
                        if tool_result.output.is_error {
                            serde_json::json!({ "error": "Tool execution failed" })
                        } else {
                            serde_json::json!({ "output": "" })
                        }
                    });

                Ok(Content {
                    role: Some(Role::User),
                    parts: vec![Part::function_response(
                        tool_result.name.to_string(),
                        response,
                    )],
                })
            }
            ContextMessage::Image(image) => {
                // Images are sent as inline data
                let mime_type = image.mime_type().to_string();

                Ok(Content {
                    role: Some(Role::User),
                    parts: vec![Part::inline_data(mime_type, image.data())],
                })
            }
        }
    }
}

/// Cleans a JSON Schema to be compatible with Gemini's API.
///
/// Gemini doesn't support certain JSON Schema fields like `$schema`, `$id`,
/// `definitions`, `$defs`, etc. This function recursively removes these
/// unsupported fields.
fn clean_schema_for_gemini(mut schema: Value) -> Value {
    if let Value::Object(ref mut map) = schema {
        // Remove unsupported top-level fields
        let unsupported_fields = [
            "$schema",
            "$id",
            "$ref",
            "$defs",
            "definitions",
            "$comment",
            "examples",
            "default",
            "const",
            "contentMediaType",
            "contentEncoding",
            "if",
            "then",
            "else",
            "allOf",
            "anyOf",
            "oneOf",
            "not",
            "additionalItems",
            "contains",
            "propertyNames",
            "patternProperties",
            "dependencies",
            "dependentSchemas",
            "dependentRequired",
            "unevaluatedItems",
            "unevaluatedProperties",
        ];

        for field in unsupported_fields {
            map.remove(field);
        }

        // Recursively clean nested properties
        if let Some(Value::Object(properties)) = map.get_mut("properties") {
            for (_, prop_value) in properties.iter_mut() {
                *prop_value = clean_schema_for_gemini(prop_value.take());
            }
        }

        // Clean items schema for arrays
        if let Some(items) = map.get_mut("items") {
            *items = clean_schema_for_gemini(items.take());
        }

        // Clean additionalProperties if it's a schema object
        if let Some(additional) = map.get_mut("additionalProperties") {
            if additional.is_object() {
                *additional = clean_schema_for_gemini(additional.take());
            }
        }
    }

    schema
}
