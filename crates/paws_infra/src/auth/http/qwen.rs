use paws_app::OAuthHttpProvider;
use paws_domain::{AuthCodeParams, OAuthConfig, OAuthTokenResponse};
use serde::{Deserialize, Serialize};

use crate::auth::util::{
    build_http_client, build_token_response, parse_token_response_with_resource_url,
};

/// Qwen Provider - OAuth2 Device Flow with PKCE
/// Qwen requires PKCE parameters in the device code request, which is not
/// supported by the standard oauth2 library's device flow implementation.
pub struct QwenHttpProvider;

#[derive(Debug, Serialize)]
struct QwenDeviceCodeRequest {
    client_id: String,
    scope: String,
    code_challenge: String,
    code_challenge_method: String,
}

#[derive(Debug, Deserialize)]
pub struct QwenDeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default)]
    pub verification_uri_complete: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
    #[serde(default)]
    pub interval: Option<u64>,
}

#[derive(Debug, Serialize)]
struct QwenTokenRequest {
    grant_type: String,
    device_code: String,
    client_id: String,
    code_verifier: String,
}

#[async_trait::async_trait]
impl OAuthHttpProvider for QwenHttpProvider {
    async fn build_auth_url(&self, _config: &OAuthConfig) -> anyhow::Result<AuthCodeParams> {
        // Qwen uses device flow, not authorization code flow
        // This method is not used for Qwen but must be implemented for the trait
        anyhow::bail!(
            "Qwen uses device flow, not authorization code flow. Use OAuthDeviceStrategy instead."
        )
    }

    async fn exchange_code(
        &self,
        _config: &OAuthConfig,
        _code: &str,
        _verifier: Option<&str>,
    ) -> anyhow::Result<OAuthTokenResponse> {
        // Qwen uses device flow, not authorization code flow
        anyhow::bail!("Qwen uses device flow, not authorization code flow.")
    }

    /// Create HTTP client with provider-specific headers/behavior
    fn build_http_client(&self, config: &OAuthConfig) -> anyhow::Result<reqwest::Client> {
        build_http_client(config.custom_headers.as_ref())
    }
}

impl QwenHttpProvider {
    /// Request device code with PKCE parameters
    pub async fn request_device_code(
        config: &OAuthConfig,
        code_challenge: &str,
    ) -> anyhow::Result<QwenDeviceCodeResponse> {
        let request = QwenDeviceCodeRequest {
            client_id: config.client_id.to_string(),
            scope: config.scopes.join(" "),
            code_challenge: code_challenge.to_string(),
            code_challenge_method: "S256".to_string(),
        };

        let client = Self::build_http_client_internal(config)?;
        let response = client
            .post(config.auth_url.as_str())
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Device code request failed with status {status}: {error_text}");
        }

        Ok(response.json().await?)
    }

    /// Poll for token using device code and PKCE verifier
    pub async fn poll_for_token(
        config: &OAuthConfig,
        device_code: &str,
        code_verifier: &str,
    ) -> anyhow::Result<OAuthTokenResponse> {
        let request = QwenTokenRequest {
            grant_type: "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            device_code: device_code.to_string(),
            client_id: config.client_id.to_string(),
            code_verifier: code_verifier.to_string(),
        };

        let client = Self::build_http_client_internal(config)?;
        let response = client
            .post(config.token_url.as_str())
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&request)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            // Parse OAuth error
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body)
                && let Some(error) = json.get("error").and_then(|e| e.as_str())
            {
                return Err(anyhow::anyhow!("OAuth error: {error}"));
            }
            anyhow::bail!("Token request failed with status {status}: {body}");
        }

        // Parse token response with resource_url
        let (access_token, refresh_token, expires_in, _resource_url) =
            parse_token_response_with_resource_url(&body)?;
        Ok(build_token_response(
            access_token,
            refresh_token,
            expires_in,
        ))
    }

    fn build_http_client_internal(config: &OAuthConfig) -> anyhow::Result<reqwest::Client> {
        build_http_client(config.custom_headers.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use paws_domain::OAuthConfig;
    use url::Url;

    use super::*;

    fn test_qwen_oauth_config() -> OAuthConfig {
        OAuthConfig {
            client_id: "f0304373b74a44d2b584a3fb70ca9e56".to_string().into(),
            auth_url: Url::parse("https://chat.qwen.ai/api/v1/oauth2/device/code").unwrap(),
            token_url: Url::parse("https://chat.qwen.ai/api/v1/oauth2/token").unwrap(),
            scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
                "model.completion".to_string(),
            ],
            redirect_uri: None,
            use_pkce: true,
            token_refresh_url: None,
            extra_auth_params: None,
            custom_headers: None,
        }
    }

    #[test]
    fn test_qwen_device_code_request_serialization() {
        let request = QwenDeviceCodeRequest {
            client_id: "test_client".to_string(),
            scope: "openid profile".to_string(),
            code_challenge: "test_challenge".to_string(),
            code_challenge_method: "S256".to_string(),
        };

        let serialized = serde_urlencoded::to_string(&request).unwrap();
        assert!(serialized.contains("client_id=test_client"));
        assert!(serialized.contains("scope=openid+profile"));
        assert!(serialized.contains("code_challenge=test_challenge"));
        assert!(serialized.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_qwen_token_request_serialization() {
        let request = QwenTokenRequest {
            grant_type: "urn:ietf:params:oauth:grant-type:device_code".to_string(),
            device_code: "test_device_code".to_string(),
            client_id: "test_client".to_string(),
            code_verifier: "test_verifier".to_string(),
        };

        let serialized = serde_urlencoded::to_string(&request).unwrap();
        // Note: colons are URL-encoded as %3A
        assert!(
            serialized
                .contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code")
        );
        assert!(serialized.contains("device_code=test_device_code"));
        assert!(serialized.contains("client_id=test_client"));
        assert!(serialized.contains("code_verifier=test_verifier"));
    }

    #[test]
    fn test_qwen_device_code_response_deserialization() {
        let json = r#"{
            "device_code": "test_device_code",
            "user_code": "ABCD-EFGH",
            "verification_uri": "https://example.com/verify",
            "verification_uri_complete": "https://example.com/verify?code=ABCD-EFGH",
            "expires_in": 600,
            "interval": 5
        }"#;

        let response: QwenDeviceCodeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.device_code, "test_device_code");
        assert_eq!(response.user_code, "ABCD-EFGH");
        assert_eq!(response.verification_uri, "https://example.com/verify");
        assert_eq!(
            response.verification_uri_complete,
            Some("https://example.com/verify?code=ABCD-EFGH".to_string())
        );
        assert_eq!(response.expires_in, Some(600));
        assert_eq!(response.interval, Some(5));
    }
}
