use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use bytes::Bytes;

use super::ObjectId;
use super::object_store::ObjectStore;

/// Per-session virtual file system overlay.
///
/// Each `SessionFS` maintains its own view of the file tree, mapping file
/// paths to `ObjectId`s in a shared `ObjectStore`. File content is never
/// duplicated: two sessions that have not modified a file share the same
/// underlying blob.
///
/// ## Snapshot / Undo
///
/// Every mutating operation (`write`, `remove`) can be preceded by a
/// `snapshot` call that pushes the current `ObjectId` onto a per-path
/// history stack. Calling `undo` pops the most recent entry and restores
/// it, exactly mirroring the disk-based `SnapshotService`.
#[derive(Debug)]
pub struct SessionFS {
    store: Arc<ObjectStore>,
    /// Current path → object mapping (the "working tree")
    files: HashMap<PathBuf, ObjectId>,
    /// Per-path snapshot history (most recent at the back)
    snapshots: HashMap<PathBuf, Vec<ObjectId>>,
    /// Paths that have been deleted in this session
    deleted: HashSet<PathBuf>,
}

impl SessionFS {
    /// Creates a new empty session backed by the given object store.
    pub fn new(store: Arc<ObjectStore>) -> Self {
        Self {
            store,
            files: HashMap::new(),
            snapshots: HashMap::new(),
            deleted: HashSet::new(),
        }
    }

    /// Reads the content of a file in this session.
    ///
    /// Returns `None` if the file does not exist in the session.
    pub fn read(&self, path: &Path) -> Result<Option<Bytes>> {
        if self.deleted.contains(path) {
            return Ok(None);
        }
        match self.files.get(path) {
            Some(id) => {
                let content = self.store.get(id).context(format!(
                    "Object {id} missing from store for {}",
                    path.display()
                ))?;
                Ok(Some(content))
            }
            None => Ok(None),
        }
    }

    /// Reads the content of a file as a UTF-8 string.
    ///
    /// Returns `None` if the file does not exist in the session.
    pub fn read_utf8(&self, path: &Path) -> Result<Option<String>> {
        match self.read(path)? {
            Some(bytes) => {
                let s = String::from_utf8(bytes.to_vec())
                    .context(format!("File {} is not valid UTF-8", path.display()))?;
                Ok(Some(s))
            }
            None => Ok(None),
        }
    }

    /// Writes content to a file in this session.
    ///
    /// The content is inserted into the shared object store and the
    /// session's path mapping is updated. If the file was previously
    /// deleted in this session, the deletion is cleared.
    pub fn write(&mut self, path: &Path, content: Bytes) -> ObjectId {
        let id = self.store.insert(content);
        self.files.insert(path.to_path_buf(), id.clone());
        self.deleted.remove(path);
        id
    }

    /// Writes a UTF-8 string to a file in this session.
    pub fn write_utf8(&mut self, path: &Path, content: &str) -> ObjectId {
        self.write(path, Bytes::from(content.to_owned()))
    }

    /// Returns `true` if the file exists in this session.
    pub fn exists(&self, path: &Path) -> bool {
        !self.deleted.contains(path) && self.files.contains_key(path)
    }

    /// Removes a file from this session.
    ///
    /// The file is marked as deleted but its content remains in the
    /// object store (it may be referenced by other sessions or
    /// snapshots).
    pub fn remove(&mut self, path: &Path) {
        self.files.remove(path);
        self.deleted.insert(path.to_path_buf());
    }

    /// Captures a snapshot of the current file state for undo support.
    ///
    /// Pushes the current `ObjectId` for the given path onto the
    /// snapshot stack. If the file does not currently exist, pushes a
    /// sentinel `None` entry.
    pub fn snapshot(&mut self, path: &Path) -> Result<()> {
        let history = self.snapshots.entry(path.to_path_buf()).or_default();
        if let Some(id) = self.files.get(path) {
            history.push(id.clone());
        }
        Ok(())
    }

    /// Restores the most recent snapshot for the given path.
    ///
    /// Pops the last entry from the snapshot stack and restores the
    /// path mapping. Returns an error if no snapshots exist.
    pub fn undo(&mut self, path: &Path) -> Result<()> {
        let history = self
            .snapshots
            .get_mut(path)
            .context(format!("No snapshots found for {}", path.display()))?;

        let id = history
            .pop()
            .context(format!("No snapshots found for {}", path.display()))?;

        self.files.insert(path.to_path_buf(), id);
        self.deleted.remove(path);
        Ok(())
    }

    /// Returns the `ObjectId` for a file if it exists.
    pub fn object_id(&self, path: &Path) -> Option<&ObjectId> {
        if self.deleted.contains(path) {
            return None;
        }
        self.files.get(path)
    }

    /// Returns the number of live (non-deleted) files in this session.
    pub fn file_count(&self) -> usize {
        self.files
            .keys()
            .filter(|p| !self.deleted.contains(p.as_path()))
            .count()
    }

    /// Lists all live file paths in this session.
    pub fn list_files(&self) -> Vec<&Path> {
        self.files
            .keys()
            .filter(|p| !self.deleted.contains(p.as_path()))
            .map(|p| p.as_path())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn fixture() -> (Arc<ObjectStore>, SessionFS) {
        let store = Arc::new(ObjectStore::new());
        let session = SessionFS::new(store.clone());
        (store, session)
    }

    #[test]
    fn test_write_and_read() {
        let (_store, mut session) = fixture();
        let path = Path::new("/tmp/test.txt");
        session.write_utf8(path, "hello");
        let actual = session.read_utf8(path).unwrap();
        let expected = Some("hello".to_string());
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_read_nonexistent() {
        let (_store, session) = fixture();
        let actual = session.read(Path::new("/no/such/file")).unwrap();
        assert_eq!(actual, None);
    }

    #[test]
    fn test_overwrite() {
        let (_store, mut session) = fixture();
        let path = Path::new("/tmp/test.txt");
        session.write_utf8(path, "v1");
        session.write_utf8(path, "v2");
        let actual = session.read_utf8(path).unwrap();
        let expected = Some("v2".to_string());
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_remove_file() {
        let (_store, mut session) = fixture();
        let path = Path::new("/tmp/test.txt");
        session.write_utf8(path, "content");
        session.remove(path);
        assert!(!session.exists(path));
        assert_eq!(session.read(path).unwrap(), None);
    }

    #[test]
    fn test_snapshot_and_undo() {
        let (_store, mut session) = fixture();
        let path = Path::new("/tmp/test.txt");

        session.write_utf8(path, "original");
        session.snapshot(path).unwrap();
        session.write_utf8(path, "modified");

        assert_eq!(
            session.read_utf8(path).unwrap(),
            Some("modified".to_string())
        );

        session.undo(path).unwrap();
        assert_eq!(
            session.read_utf8(path).unwrap(),
            Some("original".to_string())
        );
    }

    #[test]
    fn test_multiple_snapshots() {
        let (_store, mut session) = fixture();
        let path = Path::new("/tmp/test.txt");

        session.write_utf8(path, "v1");
        session.snapshot(path).unwrap();

        session.write_utf8(path, "v2");
        session.snapshot(path).unwrap();

        session.write_utf8(path, "v3");

        session.undo(path).unwrap();
        assert_eq!(session.read_utf8(path).unwrap(), Some("v2".to_string()));

        session.undo(path).unwrap();
        assert_eq!(session.read_utf8(path).unwrap(), Some("v1".to_string()));
    }

    #[test]
    fn test_undo_no_snapshots() {
        let (_store, mut session) = fixture();
        let path = Path::new("/tmp/test.txt");
        let result = session.undo(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_undo_after_remove() {
        let (_store, mut session) = fixture();
        let path = Path::new("/tmp/test.txt");

        session.write_utf8(path, "content");
        session.snapshot(path).unwrap();
        session.remove(path);

        session.undo(path).unwrap();
        assert!(session.exists(path));
        assert_eq!(
            session.read_utf8(path).unwrap(),
            Some("content".to_string())
        );
    }

    #[test]
    fn test_exists() {
        let (_store, mut session) = fixture();
        let path = Path::new("/tmp/test.txt");
        assert!(!session.exists(path));
        session.write_utf8(path, "content");
        assert!(session.exists(path));
        session.remove(path);
        assert!(!session.exists(path));
    }

    #[test]
    fn test_file_count() {
        let (_store, mut session) = fixture();
        assert_eq!(session.file_count(), 0);

        session.write_utf8(Path::new("/a.txt"), "a");
        session.write_utf8(Path::new("/b.txt"), "b");
        assert_eq!(session.file_count(), 2);

        session.remove(Path::new("/a.txt"));
        assert_eq!(session.file_count(), 1);
    }

    #[test]
    fn test_object_id_content_addressing() {
        let (store, mut session) = fixture();
        let path_a = Path::new("/a.txt");
        let path_b = Path::new("/b.txt");

        // Same content → same ObjectId
        session.write_utf8(path_a, "same");
        session.write_utf8(path_b, "same");

        assert_eq!(session.object_id(path_a), session.object_id(path_b));
        // Object store has only 1 entry for "same"
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_write_after_remove() {
        let (_store, mut session) = fixture();
        let path = Path::new("/tmp/test.txt");

        session.write_utf8(path, "v1");
        session.remove(path);
        session.write_utf8(path, "v2");

        assert!(session.exists(path));
        assert_eq!(session.read_utf8(path).unwrap(), Some("v2".to_string()));
    }
}
