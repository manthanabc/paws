use std::path::Path;

use paws_app::FileRemoverInfra;

/// Low-level file remove service
///
/// Provides primitive file deletion operations without snapshot coordination.
/// Snapshot management should be handled at the service layer.
#[derive(Default)]
pub struct PawsFileRemoveService;

impl PawsFileRemoveService {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl FileRemoverInfra for PawsFileRemoveService {
    async fn remove(&self, path: &Path) -> anyhow::Result<()> {
        Ok(paws_common::fs::PawsFS::remove_file(path).await?)
    }
}
