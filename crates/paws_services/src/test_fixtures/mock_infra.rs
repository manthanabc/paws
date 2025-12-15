//! Mock infrastructure for attachment service tests.
//!
//! This module provides reusable mock implementations of infrastructure traits
//! for testing attachment-related functionality.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use paws_app::domain::{CommandOutput, Environment, ToolDefinition, ToolName, ToolOutput};
use paws_app::{
    CommandInfra, DirectoryReaderInfra, EnvironmentInfra, FileDirectoryInfra, FileInfoInfra,
    FileReaderInfra, FileRemoverInfra, FileWriterInfra, McpClientInfra, McpServerInfra, UserInfra,
};
use paws_domain::FileInfo;
use serde_json::Value;

#[derive(Debug)]
pub struct MockEnvironmentInfra {}

#[async_trait::async_trait]
impl EnvironmentInfra for MockEnvironmentInfra {
    fn get_environment(&self) -> Environment {
        use fake::{Fake, Faker};
        let max_bytes: f64 = 250.0 * 1024.0; // 250 KB
        let fixture: Environment = Faker.fake();
        fixture
            .max_search_lines(25)
            .max_search_result_bytes(max_bytes.ceil() as usize)
            .max_read_size(2000)
            .max_file_size(256 << 10)
            .cwd(PathBuf::from("/test")) // Set fixed CWD for predictable tests
    }

    fn get_env_var(&self, _key: &str) -> Option<String> {
        None
    }

    fn get_env_vars(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }
}

#[derive(Debug)]
pub struct MockFileService {
    pub files: Mutex<Vec<(PathBuf, Bytes)>>,
    pub binary_exts: HashSet<String>,
}

impl Default for MockFileService {
    fn default() -> Self {
        Self::new()
    }
}

impl MockFileService {
    pub fn new() -> Self {
        let mut files = HashMap::new();
        // Add some mock files
        files.insert(
            PathBuf::from("/test/file1.txt"),
            "This is a text file content".to_string(),
        );
        files.insert(
            PathBuf::from("/test/image.png"),
            "mock-binary-content".to_string(),
        );
        files.insert(
            PathBuf::from("/test/image with spaces.jpg"),
            "mock-jpeg-content".to_string(),
        );

        let binary_exts = [
            "exe", "dll", "so", "dylib", "bin", "obj", "o", "class", "pyc", "jar", "war", "ear",
            "zip", "tar", "gz", "rar", "7z", "iso", "img", "pdf", "doc", "docx", "xls", "xlsx",
            "ppt", "pptx", "bmp", "ico", "mp3", "mp4", "avi", "mov", "sqlite", "db", "bin",
        ];
        let binary_exts = binary_exts.into_iter().map(|s| s.to_string()).collect();

        Self {
            files: Mutex::new(
                files
                    .into_iter()
                    .map(|(a, b)| (a, Bytes::from(b)))
                    .collect::<Vec<_>>(),
            ),
            binary_exts,
        }
    }

    pub fn add_file(&self, path: PathBuf, content: String) {
        let mut files = self.files.lock().unwrap();
        files.push((path, Bytes::from_owner(content)));
    }

    pub fn add_dir(&self, path: PathBuf) {
        let mut files = self.files.lock().unwrap();
        files.push((path, Bytes::new()));
    }
}

#[async_trait::async_trait]
impl FileReaderInfra for MockFileService {
    async fn read_utf8(&self, path: &Path) -> anyhow::Result<String> {
        let files = self.files.lock().unwrap();
        match files.iter().find(|v| v.0 == path) {
            Some((_, content)) => {
                let bytes = content.clone();
                String::from_utf8(bytes.to_vec())
                    .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in file: {path:?}: {e}"))
            }
            None => Err(anyhow::anyhow!("File not found: {path:?}")),
        }
    }

    async fn read(&self, path: &Path) -> anyhow::Result<Vec<u8>> {
        let files = self.files.lock().unwrap();
        match files.iter().find(|v| v.0 == path) {
            Some((_, content)) => Ok(content.to_vec()),
            None => Err(anyhow::anyhow!("File not found: {path:?}")),
        }
    }

    async fn range_read_utf8(
        &self,
        path: &Path,
        start_line: u64,
        end_line: u64,
    ) -> anyhow::Result<(String, FileInfo)> {
        // Read the full content first
        let full_content = self.read_utf8(path).await?;
        let all_lines: Vec<&str> = full_content.lines().collect();

        // Apply range filtering based on parameters
        let start_idx = start_line.saturating_sub(1) as usize;
        let end_idx = if end_line > 0 {
            std::cmp::min(end_line as usize, all_lines.len())
        } else {
            all_lines.len()
        };

        let filtered_lines = if start_idx < all_lines.len() {
            &all_lines[start_idx..end_idx]
        } else {
            &[]
        };

        let filtered_content = filtered_lines.join("\n");
        let actual_start = if filtered_lines.is_empty() {
            0
        } else {
            start_line
        };
        let actual_end = if filtered_lines.is_empty() {
            0
        } else {
            start_idx as u64 + filtered_lines.len() as u64
        };

        Ok((
            filtered_content,
            paws_domain::FileInfo::new(actual_start, actual_end, all_lines.len() as u64),
        ))
    }
}

#[async_trait::async_trait]
impl FileRemoverInfra for MockFileService {
    async fn remove(&self, path: &Path) -> anyhow::Result<()> {
        if !self.exists(path).await? {
            return Err(anyhow::anyhow!("File not found: {path:?}"));
        }
        self.files.lock().unwrap().retain(|(p, _)| p != path);
        Ok(())
    }
}

#[async_trait::async_trait]
impl FileDirectoryInfra for MockFileService {
    async fn create_dirs(&self, path: &Path) -> anyhow::Result<()> {
        self.files
            .lock()
            .unwrap()
            .push((path.to_path_buf(), Bytes::new()));
        Ok(())
    }
}

#[async_trait::async_trait]
impl DirectoryReaderInfra for MockFileService {
    async fn list_directory_entries(
        &self,
        directory: &Path,
    ) -> anyhow::Result<Vec<(PathBuf, bool)>> {
        let files = self.files.lock().unwrap();
        let mut results = Vec::new();

        for (path, content) in files.iter() {
            // Check if this entry is a direct child of the directory
            if let Some(parent) = path.parent()
                && parent == directory
            {
                // Check if it's a directory (empty bytes)
                let is_dir = content.is_empty();
                results.push((path.clone(), is_dir));
            }
        }

        Ok(results)
    }

    async fn read_directory_files(
        &self,
        directory: &Path,
        _pattern: Option<&str>,
    ) -> anyhow::Result<Vec<(PathBuf, String)>> {
        let files = self.files.lock().unwrap();
        let mut results = Vec::new();

        for (path, content) in files.iter() {
            // Check if this entry is a direct child of the directory
            if let Some(parent) = path.parent()
                && parent == directory
            {
                let content_str = String::from_utf8(content.to_vec()).unwrap_or_default();
                results.push((path.clone(), content_str));
            }
        }

        Ok(results)
    }
}

#[async_trait::async_trait]
impl FileWriterInfra for MockFileService {
    async fn write(&self, path: &Path, contents: Bytes) -> anyhow::Result<()> {
        let index = self.files.lock().unwrap().iter().position(|v| v.0 == path);
        if let Some(index) = index {
            self.files.lock().unwrap().remove(index);
        }
        self.files
            .lock()
            .unwrap()
            .push((path.to_path_buf(), contents));
        Ok(())
    }

    async fn write_temp(&self, _: &str, _: &str, content: &str) -> anyhow::Result<PathBuf> {
        let temp_dir = crate::utils::TempDir::new().unwrap();
        let path = temp_dir.path();

        self.write(&path, content.to_string().into()).await?;

        Ok(path)
    }
}

#[async_trait::async_trait]
impl FileInfoInfra for MockFileService {
    async fn is_file(&self, path: &Path) -> anyhow::Result<bool> {
        Ok(self
            .files
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v.0.extension().is_some())
            .any(|(p, _)| p == path))
    }

    async fn is_binary(&self, _path: &Path) -> anyhow::Result<bool> {
        let ext = _path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase());
        Ok(ext.map(|e| self.binary_exts.contains(&e)).unwrap_or(false))
    }

    async fn exists(&self, path: &Path) -> anyhow::Result<bool> {
        Ok(self.files.lock().unwrap().iter().any(|(p, _)| p == path))
    }

    async fn file_size(&self, path: &Path) -> anyhow::Result<u64> {
        let files = self.files.lock().unwrap();
        if let Some((_, content)) = files.iter().find(|(p, _)| p == path) {
            Ok(content.len() as u64)
        } else {
            Err(anyhow::anyhow!("File not found: {}", path.display()))
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Mock;

#[async_trait::async_trait]
impl McpClientInfra for Mock {
    async fn list(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        Ok(vec![])
    }

    async fn call(&self, _: &ToolName, _: Value) -> anyhow::Result<ToolOutput> {
        Ok(ToolOutput::default())
    }
}

#[async_trait::async_trait]
impl McpServerInfra for Mock {
    type Client = Mock;

    async fn connect(
        &self,
        _: paws_app::domain::McpServerConfig,
        _: &BTreeMap<String, String>,
    ) -> anyhow::Result<Self::Client> {
        Ok(Mock)
    }
}

#[async_trait::async_trait]
impl CommandInfra for Mock {
    async fn execute_command(
        &self,
        command: String,
        working_dir: PathBuf,
        _silent: bool,
        _env_vars: Option<Vec<String>>,
    ) -> anyhow::Result<CommandOutput> {
        // For test purposes, we'll create outputs that match what the shell tests
        // expect Check for common command patterns
        if command == "echo 'Hello, World!'" {
            return Ok(CommandOutput {
                stdout: "Mock command executed successfully\n".to_string(),
                stderr: "".to_string(),
                command,
                exit_code: Some(0),
            });
        } else if command.contains("echo") {
            if command.contains(">") && command.contains(">&2") {
                let stdout = if command.contains("to stdout") {
                    "to stdout\n"
                } else {
                    "stdout output\n"
                };
                let stderr = if command.contains("to stderr") {
                    "to stderr\n"
                } else {
                    "stderr output\n"
                };
                return Ok(CommandOutput {
                    stdout: stdout.to_string(),
                    stderr: stderr.to_string(),
                    command,
                    exit_code: Some(0),
                });
            } else if command.contains(">&2") {
                let content = command.split("echo").nth(1).unwrap_or("").trim();
                let content = content.trim_matches(|c| c == '\'' || c == '"');
                return Ok(CommandOutput {
                    stdout: "".to_string(),
                    stderr: format!("{content}\n"),
                    command,
                    exit_code: Some(0),
                });
            } else {
                let content = if command == "echo ''" {
                    "\n".to_string()
                } else if command.contains("&&") {
                    "first\nsecond\n".to_string()
                } else if command.contains("$PATH") {
                    "/usr/bin:/bin:/usr/sbin:/sbin\n".to_string()
                } else {
                    let parts: Vec<&str> = command.split("echo").collect();
                    if parts.len() > 1 {
                        let content = parts[1].trim();
                        let content = content.trim_matches(|c| c == '\'' || c == '"');
                        format!("{content}\n")
                    } else {
                        "Hello, World!\n".to_string()
                    }
                };

                return Ok(CommandOutput {
                    stdout: content,
                    stderr: "".to_string(),
                    command,
                    exit_code: Some(0),
                });
            }
        } else if command == "pwd" || command == "cd" {
            return Ok(CommandOutput {
                stdout: format!("{working_dir}\n", working_dir = working_dir.display()),
                stderr: "".to_string(),
                command,
                exit_code: Some(0),
            });
        } else if command == "true" {
            return Ok(CommandOutput {
                stdout: "".to_string(),
                stderr: "".to_string(),
                command,
                exit_code: Some(0),
            });
        } else if command.starts_with("/bin/ls") || command.contains("whoami") {
            return Ok(CommandOutput {
                stdout: "user\n".to_string(),
                stderr: "".to_string(),
                command,
                exit_code: Some(0),
            });
        } else if command == "non_existent_command" {
            return Ok(CommandOutput {
                stdout: "".to_string(),
                stderr: "command not found: non_existent_command\n".to_string(),
                command,
                exit_code: Some(-1),
            });
        }

        // Default response for other commands
        Ok(CommandOutput {
            stdout: "Mock command executed successfully\n".to_string(),
            stderr: "".to_string(),
            command,
            exit_code: Some(0),
        })
    }

    async fn execute_command_raw(
        &self,
        _: &str,
        _: PathBuf,
        _env_vars: Option<Vec<String>>,
    ) -> anyhow::Result<std::process::ExitStatus> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl UserInfra for Mock {
    /// Prompts the user with question
    async fn prompt_question(&self, question: &str) -> anyhow::Result<Option<String>> {
        // For testing, we can just return the question as the answer
        Ok(Some(question.to_string()))
    }

    /// Prompts the user to select a single option from a list
    async fn select_one<T: std::fmt::Display + Send + 'static>(
        &self,
        _: &str,
        options: Vec<T>,
    ) -> anyhow::Result<Option<T>> {
        // For testing, we can just return the first option
        if options.is_empty() {
            return Err(anyhow::anyhow!("No options provided"));
        }
        Ok(Some(options.into_iter().next().unwrap()))
    }

    /// Prompts the user to select multiple options from a list
    async fn select_many<T: std::fmt::Display + Clone + Send + 'static>(
        &self,
        _: &str,
        options: Vec<T>,
    ) -> anyhow::Result<Option<Vec<T>>> {
        // For testing, we can just return all options
        if options.is_empty() {
            return Err(anyhow::anyhow!("No options provided"));
        }
        Ok(Some(options))
    }
}

// Create a composite mock service that implements the required traits
#[derive(Debug, Clone)]
pub struct MockCompositeService {
    pub file_service: Arc<MockFileService>,
    pub env_service: Arc<MockEnvironmentInfra>,
}

impl Default for MockCompositeService {
    fn default() -> Self {
        Self::new()
    }
}

impl MockCompositeService {
    pub fn new() -> Self {
        Self {
            file_service: Arc::new(MockFileService::new()),
            env_service: Arc::new(MockEnvironmentInfra {}),
        }
    }

    pub fn add_file(&self, path: PathBuf, content: String) {
        self.file_service.add_file(path, content);
    }
}

#[async_trait::async_trait]
impl FileReaderInfra for MockCompositeService {
    async fn read_utf8(&self, path: &Path) -> anyhow::Result<String> {
        self.file_service.read_utf8(path).await
    }

    async fn read(&self, path: &Path) -> anyhow::Result<Vec<u8>> {
        self.file_service.read(path).await
    }

    async fn range_read_utf8(
        &self,
        path: &Path,
        start_line: u64,
        end_line: u64,
    ) -> anyhow::Result<(String, paws_domain::FileInfo)> {
        self.file_service
            .range_read_utf8(path, start_line, end_line)
            .await
    }
}

#[async_trait::async_trait]
impl EnvironmentInfra for MockCompositeService {
    fn get_environment(&self) -> Environment {
        self.env_service.get_environment()
    }

    fn get_env_var(&self, _key: &str) -> Option<String> {
        None
    }

    fn get_env_vars(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }
}

#[async_trait::async_trait]
impl FileInfoInfra for MockCompositeService {
    async fn is_binary(&self, path: &Path) -> anyhow::Result<bool> {
        self.file_service.is_binary(path).await
    }

    async fn is_file(&self, path: &Path) -> anyhow::Result<bool> {
        self.file_service.is_file(path).await
    }

    async fn exists(&self, path: &Path) -> anyhow::Result<bool> {
        self.file_service.exists(path).await
    }

    async fn file_size(&self, path: &Path) -> anyhow::Result<u64> {
        self.file_service.file_size(path).await
    }
}

#[async_trait::async_trait]
impl DirectoryReaderInfra for MockCompositeService {
    async fn list_directory_entries(
        &self,
        directory: &Path,
    ) -> anyhow::Result<Vec<(PathBuf, bool)>> {
        self.file_service.list_directory_entries(directory).await
    }

    async fn read_directory_files(
        &self,
        directory: &Path,
        pattern: Option<&str>,
    ) -> anyhow::Result<Vec<(PathBuf, String)>> {
        self.file_service
            .read_directory_files(directory, pattern)
            .await
    }
}
