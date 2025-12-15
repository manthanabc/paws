use std::path::Path;

use anyhow::Result;
use paws_app::FileInfoInfra;

pub struct PawsFileMetaService;
#[async_trait::async_trait]
impl FileInfoInfra for PawsFileMetaService {
    async fn is_file(&self, path: &Path) -> Result<bool> {
        Ok(paws_common::fs::PawsFS::is_file(path))
    }

    async fn is_binary(&self, path: &Path) -> Result<bool> {
        paws_common::fs::PawsFS::is_binary_file(path).await
    }

    async fn exists(&self, path: &Path) -> Result<bool> {
        Ok(paws_common::fs::PawsFS::exists(path))
    }

    async fn file_size(&self, path: &Path) -> Result<u64> {
        paws_common::fs::PawsFS::file_size(path).await
    }
}
