use std::path::Path;

use anyhow::Result;
use paws_app::FileReaderInfra;

pub struct PawsFileReadService;

impl Default for PawsFileReadService {
    fn default() -> Self {
        Self
    }
}

impl PawsFileReadService {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl FileReaderInfra for PawsFileReadService {
    async fn read_utf8(&self, path: &Path) -> Result<String> {
        paws_fs::PawsFS::read_utf8(path).await
    }

    async fn read(&self, path: &Path) -> Result<Vec<u8>> {
        paws_fs::PawsFS::read(path).await
    }

    async fn range_read_utf8(
        &self,
        path: &Path,
        start_line: u64,
        end_line: u64,
    ) -> Result<(String, paws_domain::FileInfo)> {
        paws_fs::PawsFS::read_range_utf8(path, start_line, end_line).await
    }
}
