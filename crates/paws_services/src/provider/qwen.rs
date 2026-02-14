use std::sync::Arc;

use anyhow::{Context as _, Result};
use paws_app::HttpClientService;
use paws_app::domain::{
    ChatCompletionMessage, Context as ChatContext, Model, ModelId, ResultStream,
};
use paws_app::dto::openai::{Request, Response};
use paws_domain::Provider;
use reqwest::Url;
use tracing::debug;

use crate::provider::client::create_headers;
use crate::provider::event::into_chat_completion_message;

#[derive(Clone)]
pub struct QwenProvider<H> {
    provider: Provider<Url>,
    http: Arc<H>,
}

impl<H: HttpClientService> QwenProvider<H> {
    pub fn new(provider: Provider<Url>, http: Arc<H>) -> Self {
        Self { provider, http }
    }

    fn get_qwen_headers(&self) -> Vec<(String, String)> {
        let mut headers = Vec::new();

        // Add authorization header from provider
        if let Some(api_key) = self
            .provider
            .credential
            .as_ref()
            .map(|c| match &c.auth_details {
                paws_domain::AuthDetails::ApiKey(key) => key.as_str(),
                paws_domain::AuthDetails::OAuthWithApiKey { api_key, .. } => api_key.as_str(),
                paws_domain::AuthDetails::OAuth { tokens, .. } => tokens.access_token.as_str(),
            })
        {
            headers.push((
                reqwest::header::AUTHORIZATION.to_string(),
                format!("Bearer {api_key}"),
            ));
        }

        // Add Qwen-specific headers as per reference script
        headers.push((
            "X-DashScope-CacheControl".to_string(),
            "enable".to_string(),
        ));
        headers.push((
            "X-DashScope-AuthType".to_string(),
            "qwen-oauth".to_string(),
        ));

        debug!(
            headers = ?headers,
            "Qwen provider headers configured"
        );

        headers
    }

    async fn inner_chat(
        &self,
        model: &ModelId,
        context: ChatContext,
    ) -> ResultStream<ChatCompletionMessage, anyhow::Error> {
        let request = Request::from(context).model(model.clone());

        // Note: Qwen uses OpenAI-compatible API format, so we can use same
        // request/response handling
        let url = self.provider.url.clone();
        let headers = create_headers(self.get_qwen_headers());

        debug!(
            url = %url,
            model = %model,
            message_count = %request.message_count(),
            "Connecting to Qwen API"
        );

        let json_bytes =
            serde_json::to_vec(&request).with_context(|| "Failed to serialize request")?;

        let es = self
            .http
            .eventsource(&url, Some(headers), json_bytes.into())
            .await
            .with_context(|| format!("Failed to connect to Qwen API: {url}"))?;

        let stream = into_chat_completion_message::<Response>(url, es);

        Ok(Box::pin(stream))
    }

    async fn inner_models(&self) -> Result<Vec<Model>> {
        // Use hardcoded models for Qwen
        Ok(vec![Model {
            id: ModelId::from("coder-model".to_string()),
            name: Some("Coder Model".to_string()),
            description: None,
            context_length: Some(32768),
            tools_supported: Some(false),
            supports_parallel_tool_calls: Some(false),
            supports_reasoning: Some(false),
        }])
    }
}

impl<H: HttpClientService> QwenProvider<H> {
    pub async fn chat(
        &self,
        model: &ModelId,
        context: ChatContext,
    ) -> ResultStream<ChatCompletionMessage, anyhow::Error> {
        self.inner_chat(model, context).await
    }

    pub async fn models(&self) -> Result<Vec<Model>> {
        self.inner_models().await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use bytes::Bytes;
    use paws_app::domain::{ApiKey, AuthCredential, AuthDetails};
    use paws_domain::{ModelSource, ProviderResponse};
    use reqwest::header::HeaderMap;
    use reqwest_eventsource::EventSource;

    use super::*;

    // Simple mock for testing client functionality
    struct MockHttpClient;

    #[async_trait::async_trait]
    impl HttpClientService for MockHttpClient {
        async fn get(
            &self,
            _url: &Url,
            _headers: Option<HeaderMap>,
        ) -> anyhow::Result<reqwest::Response> {
            Err(anyhow::anyhow!("Mock HTTP client - no real requests"))
        }

        async fn post(&self, _url: &Url, _body: Bytes) -> anyhow::Result<reqwest::Response> {
            Err(anyhow::anyhow!("Mock HTTP client - no real requests"))
        }

        async fn delete(&self, _url: &Url) -> anyhow::Result<reqwest::Response> {
            Err(anyhow::anyhow!("Mock HTTP client - no real requests"))
        }

        async fn eventsource(
            &self,
            _url: &Url,
            _headers: Option<HeaderMap>,
            _body: Bytes,
        ) -> anyhow::Result<EventSource> {
            Err(anyhow::anyhow!("Mock HTTP client - no real requests"))
        }
    }

    fn make_test_provider() -> Provider<Url> {
        Provider {
            id: paws_domain::ProviderId::QWEN,
            provider_type: paws_domain::ProviderType::Llm,
            response: Some(ProviderResponse::Qwen),
            url: Url::parse("https://portal.qwen.ai/v1/chat/completions")
                .unwrap(),
            auth_methods: vec![paws_domain::AuthMethod::ApiKey],
            url_params: vec![],
            credential: Some(AuthCredential {
                id: paws_domain::ProviderId::QWEN,
                auth_details: AuthDetails::ApiKey(ApiKey::from("test-api-key".to_string())),
                url_params: HashMap::new(),
            }),
            models: Some(ModelSource::Hardcoded(vec![])),
        }
    }

    #[test]
    fn test_qwen_headers_include_portal_headers() {
        let provider = make_test_provider();
        let mock_http = Arc::new(MockHttpClient);
        let qwen = QwenProvider::new(provider, mock_http);

        let headers = qwen.get_qwen_headers();

        // Verify Qwen-specific headers are present
        assert!(headers
            .iter()
            .any(|(k, v)| k == "X-DashScope-CacheControl" && v == "enable"));
        assert!(headers
            .iter()
            .any(|(k, v)| k == "X-DashScope-AuthType" && v == "qwen-oauth"));
    }

    #[test]
    fn test_qwen_headers_include_authorization() {
        let provider = make_test_provider();
        let mock_http = Arc::new(MockHttpClient);
        let qwen = QwenProvider::new(provider, mock_http);

        let headers = qwen.get_qwen_headers();

        // Verify Authorization header is present
        assert!(headers
            .iter()
            .any(|(k, v)| k == "authorization" && v.starts_with("Bearer ")));
    }
}
