use std::fmt::{Display, Formatter};
use std::sync::Arc;

use anyhow::{Context, Result};
use dashmap::DashMap;
use uuid::Uuid;

use super::object_store::ObjectStore;
use super::session_fs::SessionFS;

/// Unique identifier for a virtual file system session.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Creates a new random session identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Multi-session virtual file system with content-addressed storage.
///
/// `VirtualFileSystem` enables multiple parallel tasks to operate on their
/// own isolated view of files without touching the disk. All file content
/// is stored once in a shared `ObjectStore` and referenced by SHA-256
/// hash. Each session sees only its own modifications.
///
/// ## Architecture
///
/// ```text
/// VirtualFileSystem
/// ├── ObjectStore (shared, thread-safe content blobs)
/// │   └── DashMap<ObjectId, Bytes>
/// └── sessions: DashMap<SessionId, Mutex<SessionFS>>
///     └── SessionFS (per-session file overlay)
///         ├── files: HashMap<PathBuf, ObjectId>
///         ├── snapshots: HashMap<PathBuf, Vec<ObjectId>>
///         └── deleted: HashSet<PathBuf>
/// ```
///
/// ## Concurrency
///
/// - Cross-session: fully concurrent via `DashMap`
/// - Within-session: serialized via `tokio::sync::Mutex` (async safe)
/// - Object store: lock-free reads via `DashMap`
pub struct VirtualFileSystem {
    store: Arc<ObjectStore>,
    sessions: DashMap<SessionId, tokio::sync::Mutex<SessionFS>>,
}

impl VirtualFileSystem {
    /// Creates a new virtual file system with an empty object store.
    pub fn new() -> Self {
        let store = Arc::new(ObjectStore::new());
        Self { store, sessions: DashMap::new() }
    }

    /// Creates a new session and returns its identifier.
    pub fn create_session(&self) -> SessionId {
        let id = SessionId::new();
        let session = SessionFS::new(self.store.clone());
        self.sessions
            .insert(id.clone(), tokio::sync::Mutex::new(session));
        id
    }

    /// Removes a session and all its file mappings.
    ///
    /// Content in the object store is not removed since other sessions
    /// may reference it.
    pub fn remove_session(&self, id: &SessionId) -> Result<()> {
        self.sessions
            .remove(id)
            .context(format!("Session {id} not found"))?;
        Ok(())
    }

    /// Returns the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Provides access to the shared object store.
    pub fn object_store(&self) -> &ObjectStore {
        &self.store
    }

    /// Executes an async closure with mutable access to the given session.
    ///
    /// This is the primary access pattern: callers pass a closure that
    /// receives a `&mut SessionFS` and returns a result. The session
    /// mutex is held only for the duration of the closure.
    pub async fn with_session<T, F>(&self, id: &SessionId, f: F) -> Result<T>
    where
        F: FnOnce(&mut SessionFS) -> Result<T>,
    {
        let entry = self
            .sessions
            .get(id)
            .context(format!("Session {id} not found"))?;
        let mut session = entry.value().lock().await;
        f(&mut session)
    }

    /// Executes an async closure with read-only access to the given
    /// session.
    pub async fn with_session_ref<T, F>(&self, id: &SessionId, f: F) -> Result<T>
    where
        F: FnOnce(&SessionFS) -> Result<T>,
    {
        let entry = self
            .sessions
            .get(id)
            .context(format!("Session {id} not found"))?;
        let session = entry.value().lock().await;
        f(&session)
    }
}

impl Default for VirtualFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn test_create_session() {
        let vfs = VirtualFileSystem::new();
        let id = vfs.create_session();
        assert_eq!(vfs.session_count(), 1);
        vfs.remove_session(&id).unwrap();
        assert_eq!(vfs.session_count(), 0);
    }

    #[tokio::test]
    async fn test_session_isolation() {
        let vfs = VirtualFileSystem::new();
        let s1 = vfs.create_session();
        let s2 = vfs.create_session();
        let path = Path::new("/shared/file.txt");

        // Write in session 1
        vfs.with_session(&s1, |fs| {
            fs.write_utf8(path, "session 1 content");
            Ok(())
        })
        .await
        .unwrap();

        // Write different content in session 2
        vfs.with_session(&s2, |fs| {
            fs.write_utf8(path, "session 2 content");
            Ok(())
        })
        .await
        .unwrap();

        // Each session sees its own version
        let actual_s1 = vfs
            .with_session_ref(&s1, |fs| fs.read_utf8(path))
            .await
            .unwrap();
        let actual_s2 = vfs
            .with_session_ref(&s2, |fs| fs.read_utf8(path))
            .await
            .unwrap();

        assert_eq!(actual_s1, Some("session 1 content".to_string()));
        assert_eq!(actual_s2, Some("session 2 content".to_string()));
    }

    #[tokio::test]
    async fn test_content_deduplication_across_sessions() {
        let vfs = VirtualFileSystem::new();
        let s1 = vfs.create_session();
        let s2 = vfs.create_session();
        let path = Path::new("/file.txt");
        let content = "identical content";

        vfs.with_session(&s1, |fs| {
            fs.write_utf8(path, content);
            Ok(())
        })
        .await
        .unwrap();

        vfs.with_session(&s2, |fs| {
            fs.write_utf8(path, content);
            Ok(())
        })
        .await
        .unwrap();

        // Only one object in the store
        assert_eq!(vfs.object_store().len(), 1);
    }

    #[tokio::test]
    async fn test_snapshot_and_undo_within_session() {
        let vfs = VirtualFileSystem::new();
        let sid = vfs.create_session();
        let path = Path::new("/file.txt");

        vfs.with_session(&sid, |fs| {
            fs.write_utf8(path, "v1");
            fs.snapshot(path)?;
            fs.write_utf8(path, "v2");
            Ok(())
        })
        .await
        .unwrap();

        let before_undo = vfs
            .with_session_ref(&sid, |fs| fs.read_utf8(path))
            .await
            .unwrap();
        assert_eq!(before_undo, Some("v2".to_string()));

        vfs.with_session(&sid, |fs| fs.undo(path))
            .await
            .unwrap();

        let after_undo = vfs
            .with_session_ref(&sid, |fs| fs.read_utf8(path))
            .await
            .unwrap();
        assert_eq!(after_undo, Some("v1".to_string()));
    }

    #[tokio::test]
    async fn test_remove_nonexistent_session() {
        let vfs = VirtualFileSystem::new();
        let result = vfs.remove_session(&SessionId::new());
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_with_session_nonexistent() {
        let vfs = VirtualFileSystem::new();
        let result = vfs
            .with_session(&SessionId::new(), |_fs| Ok(()))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parallel_sessions() {
        let vfs = Arc::new(VirtualFileSystem::new());
        let path = Path::new("/parallel.txt");

        let mut handles = Vec::new();
        for i in 0..10 {
            let vfs = vfs.clone();
            let sid = vfs.create_session();
            handles.push(tokio::spawn(async move {
                let content = format!("session-{i}");
                vfs.with_session(&sid, |fs| {
                    fs.write_utf8(path, &content);
                    Ok(())
                })
                .await
                .unwrap();

                let actual = vfs
                    .with_session_ref(&sid, |fs| fs.read_utf8(path))
                    .await
                    .unwrap();
                assert_eq!(actual, Some(content));
                sid
            }));
        }

        for handle in handles {
            let sid = handle.await.unwrap();
            vfs.remove_session(&sid).unwrap();
        }

        assert_eq!(vfs.session_count(), 0);
    }

    #[tokio::test]
    async fn test_file_operations_across_session_lifecycle() {
        let vfs = VirtualFileSystem::new();
        let sid = vfs.create_session();
        let path = Path::new("/lifecycle.txt");

        // Create → snapshot → modify → snapshot → delete → undo → undo
        vfs.with_session(&sid, |fs| {
            fs.write_utf8(path, "created");
            fs.snapshot(path)?;
            fs.write_utf8(path, "modified");
            fs.snapshot(path)?;
            fs.remove(path);
            Ok(())
        })
        .await
        .unwrap();

        let deleted = vfs
            .with_session_ref(&sid, |fs| Ok(fs.exists(path)))
            .await
            .unwrap();
        assert!(!deleted);

        // Undo remove → should see "modified"
        vfs.with_session(&sid, |fs| fs.undo(path))
            .await
            .unwrap();
        let actual = vfs
            .with_session_ref(&sid, |fs| fs.read_utf8(path))
            .await
            .unwrap();
        assert_eq!(actual, Some("modified".to_string()));

        // Undo modify → should see "created"
        vfs.with_session(&sid, |fs| fs.undo(path))
            .await
            .unwrap();
        let actual = vfs
            .with_session_ref(&sid, |fs| fs.read_utf8(path))
            .await
            .unwrap();
        assert_eq!(actual, Some("created".to_string()));
    }
}
