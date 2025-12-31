//! Gemini provider implementation for Google's Gemini API.
//!
//! This provider handles the native Gemini API format which differs from
//! OpenAI:
//! - Uses `/v1beta/models/{model}:streamGenerateContent` endpoint
//! - Content structure uses `contents` with `parts`
//! - Tool calling uses `functionCall` and `functionResponse`
//! - Streaming uses SSE with different JSON structure

use std::sync::Arc;

use anyhow::Context as _;
use paws_app::HttpClientService;
use paws_app::domain::{
    ChatCompletionMessage, Context as ChatContext, Model, ModelId, ResultStream,
};
use paws_app::dto::gemini::{ListModelsResponse, Request, Response};
use reqwest::Url;
use tracing::{debug, info};

use crate::provider::client::create_headers;
use crate::provider::event::into_chat_completion_message;
use crate::provider::utils::{format_http_context, sanitize_headers};

/// Gemini provider for Google's Gemini API
#[derive(Clone)]
pub struct GeminiProvider<T> {
    http: Arc<T>,
    api_key: String,
    base_url: Url,
    models: paws_domain::ModelSource<Url>,
}

impl<H: HttpClientService> GeminiProvider<H> {
    /// Creates a new Gemini provider
    ///
    /// # Arguments
    ///
    /// * `http` - HTTP client service for making requests
    /// * `api_key` - Google AI API key
    /// * `base_url` - Base URL for the Gemini API (e.g., https://generativelanguage.googleapis.com)
    /// * `models` - Model source configuration
    pub fn new(
        http: Arc<H>,
        api_key: String,
        base_url: Url,
        models: paws_domain::ModelSource<Url>,
    ) -> Self {
        Self { http, api_key, base_url, models }
    }

    /// Returns headers for API requests
    fn get_headers(&self) -> Vec<(String, String)> {
        // Gemini API uses API key as a query parameter, but we also support it in
        // headers for compatibility with some setups
        vec![("x-goog-api-key".to_string(), self.api_key.clone())]
    }

    /// Builds the streaming content generation URL for a specific model
    fn build_chat_url(&self, model: &ModelId) -> anyhow::Result<Url> {
        // Gemini API endpoint format:
        // POST https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent
        let path = format!(
            "v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            model.as_str(),
            self.api_key
        );
        self.base_url
            .join(&path)
            .with_context(|| format!("Failed to build chat URL for model {}", model.as_str()))
    }
}

impl<T: HttpClientService> GeminiProvider<T> {
    /// Sends a chat completion request and returns a stream of messages
    pub async fn chat(
        &self,
        model: &ModelId,
        context: ChatContext,
    ) -> ResultStream<ChatCompletionMessage, anyhow::Error> {
        let request = Request::try_from(context)?;

        let url = self.build_chat_url(model)?;
        let headers = create_headers(self.get_headers());

        info!(
            url = %url,
            model = %model,
            headers = ?sanitize_headers(&headers),
            message_count = %request.contents.len(),
            "Connecting to Gemini"
        );

        let json_bytes =
            serde_json::to_vec(&request).with_context(|| "Failed to serialize Gemini request")?;

        let es = self
            .http
            .eventsource(&url, Some(headers), json_bytes.into())
            .await
            .with_context(|| format_http_context(None, "POST", &url))?;

        let stream = into_chat_completion_message::<Response>(url, es);

        Ok(Box::pin(stream))
    }

    /// Fetches available models from the Gemini API
    pub async fn models(&self) -> anyhow::Result<Vec<Model>> {
        match &self.models {
            paws_domain::ModelSource::Url(url) => {
                debug!(url = %url, "Fetching Gemini models");

                // Add API key to the models URL
                let mut models_url = url.clone();
                models_url.set_query(Some(&format!("key={}", self.api_key)));

                let response = self
                    .http
                    .get(&models_url, Some(create_headers(self.get_headers())))
                    .await
                    .with_context(|| format_http_context(None, "GET", &models_url))
                    .with_context(|| "Failed to fetch Gemini models")?;

                let status = response.status();
                let ctx_msg = format_http_context(Some(status), "GET", &models_url);
                let text = response
                    .text()
                    .await
                    .with_context(|| ctx_msg.clone())
                    .with_context(|| "Failed to decode response into text")?;

                if status.is_success() {
                    let list_response: ListModelsResponse = serde_json::from_str(&text)
                        .with_context(|| ctx_msg)
                        .with_context(|| "Failed to deserialize Gemini models response")?;

                    // Filter to only include models that support generateContent
                    let models: Vec<Model> = list_response
                        .models
                        .into_iter()
                        .filter(|m| {
                            m.supported_generation_methods
                                .as_ref()
                                .map(|methods| {
                                    methods.iter().any(|method| {
                                        method == "generateContent"
                                            || method == "streamGenerateContent"
                                    })
                                })
                                .unwrap_or(false)
                        })
                        .map(Into::into)
                        .collect();

                    Ok(models)
                } else {
                    Err(anyhow::anyhow!(text))
                        .with_context(|| ctx_msg)
                        .with_context(|| "Failed to fetch Gemini models")
                }
            }
            paws_domain::ModelSource::Hardcoded(models) => {
                debug!("Using hardcoded Gemini models");
                Ok(models.clone())
            }
        }
    }
}
