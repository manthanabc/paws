use std::sync::Arc;

use paws_app::{
    AgentRepository, CommandInfra, DirectoryReaderInfra, EnvironmentInfra, FileDirectoryInfra,
    FileInfoInfra, FileReaderInfra, FileRemoverInfra, FileWriterInfra, HttpInfra, KVStore,
    McpServerInfra, Services, StrategyFactory, UserInfra, WalkerInfra,
};
use paws_domain::{
    AppConfigRepository, ConversationRepository, ProviderRepository, SkillRepository,
    SnapshotRepository,
};

use crate::PawsProviderAuthService;
use crate::agent_registry::PawsAgentRegistryService;
use crate::app_config::PawsAppConfigService;
use crate::attachment::PawsChatRequest;
use crate::auth::PawsAuthService;
use crate::command::CommandLoaderService as PawsCommandLoaderService;
use crate::conversation::PawsConversationService;
use crate::discovery::PawsDiscoveryService;
use crate::env::PawsEnvironmentService;
use crate::instructions::PawsCustomInstructionsService;
use crate::mcp::{PawsMcpManager, PawsMcpService};
use crate::policy::PawsPolicyService;
use crate::provider::PawsProviderService;
use crate::template::PawsTemplateService;
use crate::tool_services::{
    PawsFetch, PawsFollowup, PawsFsCreate, PawsFsPatch, PawsFsRead, PawsFsRemove, PawsFsSearch,
    PawsFsUndo, PawsImageRead, PawsPlanCreate, PawsShell, PawsSkillFetch,
};
use crate::workflow::PawsWorkflowService;

type McpService<F> = PawsMcpService<PawsMcpManager<F>, F, <F as McpServerInfra>::Client>;
type AuthService<F> = PawsAuthService<F>;

/// PawsApp is the main application container that implements the App trait.
/// It provides access to all core services required by the application.
///
/// Type Parameters:
/// - F: The infrastructure implementation that provides core services like
///   environment, file reading, vector indexing, and embedding.
/// - R: The repository implementation that provides data persistence
#[derive(Clone)]
pub struct PawsServices<
    F: HttpInfra
        + EnvironmentInfra
        + McpServerInfra
        + WalkerInfra
        + SnapshotRepository
        + ConversationRepository
        + AppConfigRepository
        + KVStore
        + ProviderRepository
        + AgentRepository
        + SkillRepository,
> {
    chat_service: Arc<PawsProviderService<F>>,
    config_service: Arc<PawsAppConfigService<F>>,
    conversation_service: Arc<PawsConversationService<F>>,
    template_service: Arc<PawsTemplateService<F>>,
    attachment_service: Arc<PawsChatRequest<F>>,
    workflow_service: Arc<PawsWorkflowService<F>>,
    discovery_service: Arc<PawsDiscoveryService<F>>,
    mcp_manager: Arc<PawsMcpManager<F>>,
    file_create_service: Arc<PawsFsCreate<F>>,
    plan_create_service: Arc<PawsPlanCreate<F>>,
    file_read_service: Arc<PawsFsRead<F>>,
    image_read_service: Arc<PawsImageRead<F>>,
    file_search_service: Arc<PawsFsSearch<F>>,
    file_remove_service: Arc<PawsFsRemove<F>>,
    file_patch_service: Arc<PawsFsPatch<F>>,
    file_undo_service: Arc<PawsFsUndo<F>>,
    shell_service: Arc<PawsShell<F>>,
    fetch_service: Arc<PawsFetch>,
    followup_service: Arc<PawsFollowup<F>>,
    mcp_service: Arc<McpService<F>>,
    env_service: Arc<PawsEnvironmentService<F>>,
    custom_instructions_service: Arc<PawsCustomInstructionsService<F>>,
    auth_service: Arc<AuthService<F>>,
    agent_registry_service: Arc<PawsAgentRegistryService<F>>,
    command_loader_service: Arc<PawsCommandLoaderService<F>>,
    policy_service: PawsPolicyService<F>,
    provider_auth_service: PawsProviderAuthService<F>,
    skill_service: Arc<PawsSkillFetch<F>>,
}

impl<
    F: McpServerInfra
        + EnvironmentInfra
        + FileWriterInfra
        + FileInfoInfra
        + FileReaderInfra
        + HttpInfra
        + WalkerInfra
        + DirectoryReaderInfra
        + CommandInfra
        + UserInfra
        + SnapshotRepository
        + ConversationRepository
        + AppConfigRepository
        + ProviderRepository
        + KVStore
        + AgentRepository
        + SkillRepository,
> PawsServices<F>
{
    pub fn new(infra: Arc<F>) -> Self {
        let mcp_manager = Arc::new(PawsMcpManager::new(infra.clone()));
        let mcp_service = Arc::new(PawsMcpService::new(mcp_manager.clone(), infra.clone()));
        let template_service = Arc::new(PawsTemplateService::new(infra.clone()));
        let attachment_service = Arc::new(PawsChatRequest::new(infra.clone()));
        let workflow_service = Arc::new(PawsWorkflowService::new(infra.clone()));
        let suggestion_service = Arc::new(PawsDiscoveryService::new(infra.clone()));
        let conversation_service = Arc::new(PawsConversationService::new(infra.clone()));
        let auth_service = Arc::new(PawsAuthService::new(infra.clone()));
        let chat_service = Arc::new(PawsProviderService::new(infra.clone()));
        let config_service = Arc::new(PawsAppConfigService::new(infra.clone()));
        let file_create_service = Arc::new(PawsFsCreate::new(infra.clone()));
        let plan_create_service = Arc::new(PawsPlanCreate::new(infra.clone()));
        let file_read_service = Arc::new(PawsFsRead::new(infra.clone()));
        let image_read_service = Arc::new(PawsImageRead::new(infra.clone()));
        let file_search_service = Arc::new(PawsFsSearch::new(infra.clone()));
        let file_remove_service = Arc::new(PawsFsRemove::new(infra.clone()));
        let file_patch_service = Arc::new(PawsFsPatch::new(infra.clone()));
        let file_undo_service = Arc::new(PawsFsUndo::new(infra.clone()));
        let shell_service = Arc::new(PawsShell::new(infra.clone()));
        let fetch_service = Arc::new(PawsFetch::new());
        let followup_service = Arc::new(PawsFollowup::new(infra.clone()));
        let env_service = Arc::new(PawsEnvironmentService::new(infra.clone()));
        let custom_instructions_service =
            Arc::new(PawsCustomInstructionsService::new(infra.clone()));
        let agent_registry_service = Arc::new(PawsAgentRegistryService::new(infra.clone()));
        let command_loader_service = Arc::new(PawsCommandLoaderService::new(infra.clone()));
        let policy_service = PawsPolicyService::new(infra.clone());
        let provider_auth_service = PawsProviderAuthService::new(infra.clone());
        let skill_service = Arc::new(PawsSkillFetch::new(infra.clone()));

        Self {
            conversation_service,
            attachment_service,
            template_service,
            workflow_service,
            discovery_service: suggestion_service,
            mcp_manager,
            file_create_service,
            plan_create_service,
            file_read_service,
            image_read_service,
            file_search_service,
            file_remove_service,
            file_patch_service,
            file_undo_service,
            shell_service,
            fetch_service,
            followup_service,
            mcp_service,
            env_service,
            custom_instructions_service,
            auth_service,
            chat_service,
            config_service,
            agent_registry_service,
            command_loader_service,
            policy_service,
            provider_auth_service,
            skill_service,
        }
    }
}

impl<
    F: FileReaderInfra
        + FileWriterInfra
        + CommandInfra
        + UserInfra
        + McpServerInfra
        + FileRemoverInfra
        + FileInfoInfra
        + FileDirectoryInfra
        + EnvironmentInfra
        + DirectoryReaderInfra
        + HttpInfra
        + WalkerInfra
        + Clone
        + SnapshotRepository
        + ConversationRepository
        + AppConfigRepository
        + KVStore
        + ProviderRepository
        + AgentRepository
        + SkillRepository
        + StrategyFactory
        + Clone
        + 'static,
> Services for PawsServices<F>
{
    type ProviderService = PawsProviderService<F>;
    type AppConfigService = PawsAppConfigService<F>;
    type ConversationService = PawsConversationService<F>;
    type TemplateService = PawsTemplateService<F>;
    type ProviderAuthService = PawsProviderAuthService<F>;

    fn provider_auth_service(&self) -> &Self::ProviderAuthService {
        &self.provider_auth_service
    }
    type AttachmentService = PawsChatRequest<F>;
    type EnvironmentService = PawsEnvironmentService<F>;
    type CustomInstructionsService = PawsCustomInstructionsService<F>;
    type WorkflowService = PawsWorkflowService<F>;
    type FileDiscoveryService = PawsDiscoveryService<F>;
    type McpConfigManager = PawsMcpManager<F>;
    type FsCreateService = PawsFsCreate<F>;
    type PlanCreateService = PawsPlanCreate<F>;
    type FsPatchService = PawsFsPatch<F>;
    type FsReadService = PawsFsRead<F>;
    type ImageReadService = PawsImageRead<F>;
    type FsRemoveService = PawsFsRemove<F>;
    type FsSearchService = PawsFsSearch<F>;
    type FollowUpService = PawsFollowup<F>;
    type FsUndoService = PawsFsUndo<F>;
    type NetFetchService = PawsFetch;
    type ShellService = PawsShell<F>;
    type McpService = McpService<F>;
    type AuthService = AuthService<F>;
    type AgentRegistry = PawsAgentRegistryService<F>;
    type CommandLoaderService = PawsCommandLoaderService<F>;
    type PolicyService = PawsPolicyService<F>;
    type SkillFetchService = PawsSkillFetch<F>;

    fn provider_service(&self) -> &Self::ProviderService {
        &self.chat_service
    }

    fn config_service(&self) -> &Self::AppConfigService {
        &self.config_service
    }

    fn conversation_service(&self) -> &Self::ConversationService {
        &self.conversation_service
    }

    fn template_service(&self) -> &Self::TemplateService {
        &self.template_service
    }

    fn attachment_service(&self) -> &Self::AttachmentService {
        &self.attachment_service
    }

    fn environment_service(&self) -> &Self::EnvironmentService {
        &self.env_service
    }
    fn custom_instructions_service(&self) -> &Self::CustomInstructionsService {
        &self.custom_instructions_service
    }

    fn workflow_service(&self) -> &Self::WorkflowService {
        self.workflow_service.as_ref()
    }

    fn file_discovery_service(&self) -> &Self::FileDiscoveryService {
        self.discovery_service.as_ref()
    }

    fn mcp_config_manager(&self) -> &Self::McpConfigManager {
        self.mcp_manager.as_ref()
    }

    fn fs_create_service(&self) -> &Self::FsCreateService {
        &self.file_create_service
    }

    fn plan_create_service(&self) -> &Self::PlanCreateService {
        &self.plan_create_service
    }

    fn fs_patch_service(&self) -> &Self::FsPatchService {
        &self.file_patch_service
    }

    fn fs_read_service(&self) -> &Self::FsReadService {
        &self.file_read_service
    }

    fn image_read_service(&self) -> &Self::ImageReadService {
        &self.image_read_service
    }

    fn fs_remove_service(&self) -> &Self::FsRemoveService {
        &self.file_remove_service
    }

    fn fs_search_service(&self) -> &Self::FsSearchService {
        &self.file_search_service
    }

    fn follow_up_service(&self) -> &Self::FollowUpService {
        &self.followup_service
    }

    fn fs_undo_service(&self) -> &Self::FsUndoService {
        &self.file_undo_service
    }

    fn net_fetch_service(&self) -> &Self::NetFetchService {
        &self.fetch_service
    }

    fn shell_service(&self) -> &Self::ShellService {
        &self.shell_service
    }

    fn mcp_service(&self) -> &Self::McpService {
        &self.mcp_service
    }

    fn auth_service(&self) -> &Self::AuthService {
        self.auth_service.as_ref()
    }

    fn agent_registry(&self) -> &Self::AgentRegistry {
        &self.agent_registry_service
    }

    fn command_loader_service(&self) -> &Self::CommandLoaderService {
        &self.command_loader_service
    }

    fn policy_service(&self) -> &Self::PolicyService {
        &self.policy_service
    }

    fn skill_fetch_service(&self) -> &Self::SkillFetchService {
        &self.skill_service
    }
}
