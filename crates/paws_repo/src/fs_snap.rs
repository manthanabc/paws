use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use paws_domain::{Environment, Snapshot, SnapshotRepository};

pub struct PawsFileSnapshotService {
    inner: Arc<paws_snaps::SnapshotService>,
}

impl PawsFileSnapshotService {
    pub fn new(env: Environment) -> Self {
        Self {
            inner: Arc::new(paws_snaps::SnapshotService::new(env.snapshot_path())),
        }
    }
}

#[async_trait::async_trait]
impl SnapshotRepository for PawsFileSnapshotService {
    // Creation
    async fn insert_snapshot(&self, file_path: &Path) -> Result<Snapshot> {
        self.inner.create_snapshot(file_path.to_path_buf()).await
    }

    // Undo
    async fn undo_snapshot(&self, file_path: &Path) -> Result<()> {
        self.inner.undo_snapshot(file_path.to_path_buf()).await
    }
}
