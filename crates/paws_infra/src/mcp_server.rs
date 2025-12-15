use std::collections::BTreeMap;

use paws_app::McpServerInfra;
use paws_domain::McpServerConfig;

use crate::mcp_client::PawsMcpClient;

#[derive(Clone)]
pub struct PawsMcpServer;

#[async_trait::async_trait]
impl McpServerInfra for PawsMcpServer {
    type Client = PawsMcpClient;

    async fn connect(
        &self,
        config: McpServerConfig,
        env_vars: &BTreeMap<String, String>,
    ) -> anyhow::Result<Self::Client> {
        Ok(PawsMcpClient::new(config, env_vars))
    }
}
