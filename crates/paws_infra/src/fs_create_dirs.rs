use std::path::Path;

use paws_app::FileDirectoryInfra;

#[derive(Default)]
pub struct PawsCreateDirsService;

#[async_trait::async_trait]
impl FileDirectoryInfra for PawsCreateDirsService {
    async fn create_dirs(&self, path: &Path) -> anyhow::Result<()> {
        Ok(paws_common::fs::PawsFS::create_dir_all(path).await?)
    }
}
