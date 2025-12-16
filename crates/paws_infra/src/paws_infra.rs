use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::Arc;

use bytes::Bytes;
use paws_app::{
    CommandInfra, DirectoryReaderInfra, EnvironmentInfra, FileDirectoryInfra, FileInfoInfra,
    FileReaderInfra, FileRemoverInfra, FileWriterInfra, HttpInfra, McpServerInfra, StrategyFactory,
    UserInfra, WalkerInfra,
};
use paws_domain::{
    AuthMethod, CommandOutput, Environment, FileInfo as FileInfoData, McpServerConfig, ProviderId,
    URLParam,
};
use reqwest::header::HeaderMap;
use reqwest::{Response, Url};
use reqwest_eventsource::EventSource;

use crate::auth::{AnyAuthStrategy, PawsAuthStrategyFactory};
use crate::env::PawsEnvironmentInfra;
use crate::executor::PawsCommandExecutorService;
use crate::fs_create_dirs::PawsCreateDirsService;
use crate::fs_meta::PawsFileMetaService;
use crate::fs_read::PawsFileReadService;
use crate::fs_read_dir::PawsDirectoryReaderService;
use crate::fs_remove::PawsFileRemoveService;
use crate::fs_write::PawsFileWriteService;
use crate::http::PawsHttpInfra;
use crate::inquire::PawsInquire;
use crate::mcp_client::PawsMcpClient;
use crate::mcp_server::PawsMcpServer;
use crate::walker::PawsWalkerService;

#[derive(Clone)]
pub struct PawsInfra {
    // TODO: Drop the "Service" suffix. Use names like PawsFileReader, PawsFileWriter,
    // PawsHttpClient etc.
    file_read_service: Arc<PawsFileReadService>,
    file_write_service: Arc<PawsFileWriteService>,
    file_remove_service: Arc<PawsFileRemoveService>,
    environment_service: Arc<PawsEnvironmentInfra>,
    file_meta_service: Arc<PawsFileMetaService>,
    create_dirs_service: Arc<PawsCreateDirsService>,
    directory_reader_service: Arc<PawsDirectoryReaderService>,
    command_executor_service: Arc<PawsCommandExecutorService>,
    inquire_service: Arc<PawsInquire>,
    mcp_server: PawsMcpServer,
    walker_service: Arc<PawsWalkerService>,
    http_service: Arc<PawsHttpInfra<PawsFileWriteService>>,
    strategy_factory: Arc<PawsAuthStrategyFactory>,
}

impl PawsInfra {
    pub fn new(restricted: bool, cwd: PathBuf) -> Self {
        let environment_service = Arc::new(PawsEnvironmentInfra::new(restricted, cwd));
        let env = environment_service.get_environment();

        let file_write_service = Arc::new(PawsFileWriteService::new());
        let http_service = Arc::new(PawsHttpInfra::new(env.clone(), file_write_service.clone()));
        let file_read_service = Arc::new(PawsFileReadService::new());
        let file_meta_service = Arc::new(PawsFileMetaService);
        let directory_reader_service = Arc::new(PawsDirectoryReaderService);

        Self {
            file_read_service,
            file_write_service,
            file_remove_service: Arc::new(PawsFileRemoveService::new()),
            environment_service,
            file_meta_service,
            create_dirs_service: Arc::new(PawsCreateDirsService),
            directory_reader_service,
            command_executor_service: Arc::new(PawsCommandExecutorService::new(
                restricted,
                env.clone(),
            )),
            inquire_service: Arc::new(PawsInquire::new()),
            mcp_server: PawsMcpServer,
            walker_service: Arc::new(PawsWalkerService::new()),
            strategy_factory: Arc::new(PawsAuthStrategyFactory::new()),
            http_service,
        }
    }
}

impl EnvironmentInfra for PawsInfra {
    fn get_environment(&self) -> Environment {
        self.environment_service.get_environment()
    }

    fn get_env_var(&self, key: &str) -> Option<String> {
        self.environment_service.get_env_var(key)
    }

    fn get_env_vars(&self) -> BTreeMap<String, String> {
        self.environment_service.get_env_vars()
    }
}

#[async_trait::async_trait]
impl FileReaderInfra for PawsInfra {
    async fn read_utf8(&self, path: &Path) -> anyhow::Result<String> {
        self.file_read_service.read_utf8(path).await
    }

    async fn read(&self, path: &Path) -> anyhow::Result<Vec<u8>> {
        self.file_read_service.read(path).await
    }

    async fn range_read_utf8(
        &self,
        path: &Path,
        start_line: u64,
        end_line: u64,
    ) -> anyhow::Result<(String, FileInfoData)> {
        self.file_read_service
            .range_read_utf8(path, start_line, end_line)
            .await
    }
}

#[async_trait::async_trait]
impl FileWriterInfra for PawsInfra {
    async fn write(&self, path: &Path, contents: Bytes) -> anyhow::Result<()> {
        self.file_write_service.write(path, contents).await
    }

    async fn write_temp(&self, prefix: &str, ext: &str, content: &str) -> anyhow::Result<PathBuf> {
        self.file_write_service
            .write_temp(prefix, ext, content)
            .await
    }
}

#[async_trait::async_trait]
impl FileInfoInfra for PawsInfra {
    async fn is_binary(&self, path: &Path) -> anyhow::Result<bool> {
        self.file_meta_service.is_binary(path).await
    }

    async fn is_file(&self, path: &Path) -> anyhow::Result<bool> {
        self.file_meta_service.is_file(path).await
    }

    async fn exists(&self, path: &Path) -> anyhow::Result<bool> {
        self.file_meta_service.exists(path).await
    }

    async fn file_size(&self, path: &Path) -> anyhow::Result<u64> {
        self.file_meta_service.file_size(path).await
    }
}
#[async_trait::async_trait]
impl FileRemoverInfra for PawsInfra {
    async fn remove(&self, path: &Path) -> anyhow::Result<()> {
        self.file_remove_service.remove(path).await
    }
}

#[async_trait::async_trait]
impl FileDirectoryInfra for PawsInfra {
    async fn create_dirs(&self, path: &Path) -> anyhow::Result<()> {
        self.create_dirs_service.create_dirs(path).await
    }
}

#[async_trait::async_trait]
impl CommandInfra for PawsInfra {
    async fn execute_command(
        &self,
        command: String,
        working_dir: PathBuf,
        silent: bool,
        env_vars: Option<Vec<String>>,
    ) -> anyhow::Result<CommandOutput> {
        self.command_executor_service
            .execute_command(command, working_dir, silent, env_vars)
            .await
    }

    async fn execute_command_raw(
        &self,
        command: &str,
        working_dir: PathBuf,
        env_vars: Option<Vec<String>>,
    ) -> anyhow::Result<ExitStatus> {
        self.command_executor_service
            .execute_command_raw(command, working_dir, env_vars)
            .await
    }
}

#[async_trait::async_trait]
impl UserInfra for PawsInfra {
    async fn prompt_question(&self, question: &str) -> anyhow::Result<Option<String>> {
        self.inquire_service.prompt_question(question).await
    }

    async fn select_one<T: std::fmt::Display + Send + 'static>(
        &self,
        message: &str,
        options: Vec<T>,
    ) -> anyhow::Result<Option<T>> {
        self.inquire_service.select_one(message, options).await
    }

    async fn select_many<T: std::fmt::Display + Clone + Send + 'static>(
        &self,
        message: &str,
        options: Vec<T>,
    ) -> anyhow::Result<Option<Vec<T>>> {
        self.inquire_service.select_many(message, options).await
    }
}

#[async_trait::async_trait]
impl McpServerInfra for PawsInfra {
    type Client = PawsMcpClient;

    async fn connect(
        &self,
        config: McpServerConfig,
        env_vars: &BTreeMap<String, String>,
    ) -> anyhow::Result<Self::Client> {
        self.mcp_server.connect(config, env_vars).await
    }
}

#[async_trait::async_trait]
impl WalkerInfra for PawsInfra {
    async fn walk(&self, config: paws_app::Walker) -> anyhow::Result<Vec<paws_app::WalkedFile>> {
        self.walker_service.walk(config).await
    }
}

#[async_trait::async_trait]
impl HttpInfra for PawsInfra {
    async fn http_get(&self, url: &Url, headers: Option<HeaderMap>) -> anyhow::Result<Response> {
        self.http_service.http_get(url, headers).await
    }

    async fn http_post(&self, url: &Url, body: Bytes) -> anyhow::Result<Response> {
        self.http_service.http_post(url, body).await
    }

    async fn http_delete(&self, url: &Url) -> anyhow::Result<Response> {
        self.http_service.http_delete(url).await
    }
    async fn http_eventsource(
        &self,
        url: &Url,
        headers: Option<HeaderMap>,
        body: Bytes,
    ) -> anyhow::Result<EventSource> {
        self.http_service.http_eventsource(url, headers, body).await
    }
}
#[async_trait::async_trait]
impl DirectoryReaderInfra for PawsInfra {
    async fn list_directory_entries(
        &self,
        directory: &Path,
    ) -> anyhow::Result<Vec<(PathBuf, bool)>> {
        self.directory_reader_service
            .list_directory_entries(directory)
            .await
    }

    async fn read_directory_files(
        &self,
        directory: &Path,
        pattern: Option<&str>,
    ) -> anyhow::Result<Vec<(PathBuf, String)>> {
        self.directory_reader_service
            .read_directory_files(directory, pattern)
            .await
    }
}

impl StrategyFactory for PawsInfra {
    type Strategy = AnyAuthStrategy;
    fn create_auth_strategy(
        &self,
        provider_id: ProviderId,
        method: AuthMethod,
        required_params: Vec<URLParam>,
    ) -> anyhow::Result<Self::Strategy> {
        self.strategy_factory
            .create_auth_strategy(provider_id, method, required_params)
    }
}
