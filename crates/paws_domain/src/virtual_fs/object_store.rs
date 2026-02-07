use bytes::Bytes;
use dashmap::DashMap;

use super::ObjectId;

/// Thread-safe, content-addressed blob store.
///
/// `ObjectStore` maps `ObjectId` (SHA-256 hash) to file content (`Bytes`).
/// Identical content is stored only once, regardless of how many files or
/// sessions reference it. This makes cross-session sharing both memory
/// efficient and lock-free for reads.
#[derive(Debug)]
pub struct ObjectStore {
    objects: DashMap<ObjectId, Bytes>,
}

impl ObjectStore {
    /// Creates a new empty object store.
    pub fn new() -> Self {
        Self { objects: DashMap::new() }
    }

    /// Inserts content into the store and returns its `ObjectId`.
    ///
    /// If the same content already exists, this is a no-op and the
    /// existing `ObjectId` is returned.
    pub fn insert(&self, content: Bytes) -> ObjectId {
        let id = ObjectId::from_content(&content);
        self.objects.entry(id.clone()).or_insert(content);
        id
    }

    /// Retrieves content by its `ObjectId`.
    ///
    /// Returns `None` if the object has been garbage collected or never
    /// existed.
    pub fn get(&self, id: &ObjectId) -> Option<Bytes> {
        self.objects.get(id).map(|v| v.value().clone())
    }

    /// Returns the number of unique objects stored.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Returns `true` if the store contains no objects.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Returns `true` if the store contains the given object.
    pub fn contains(&self, id: &ObjectId) -> bool {
        self.objects.contains_key(id)
    }
}

impl Default for ObjectStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_insert_and_get() {
        let store = ObjectStore::new();
        let content = Bytes::from("hello world");
        let id = store.insert(content.clone());
        let actual = store.get(&id).unwrap();
        assert_eq!(actual, content);
    }

    #[test]
    fn test_deduplication() {
        let store = ObjectStore::new();
        let content = Bytes::from("same content");
        let id1 = store.insert(content.clone());
        let id2 = store.insert(content);
        assert_eq!(id1, id2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_different_content() {
        let store = ObjectStore::new();
        store.insert(Bytes::from("aaa"));
        store.insert(Bytes::from("bbb"));
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_get_missing_object() {
        let store = ObjectStore::new();
        let id = ObjectId::from_content(b"not inserted");
        assert!(store.get(&id).is_none());
    }

    #[test]
    fn test_empty_store() {
        let store = ObjectStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_contains() {
        let store = ObjectStore::new();
        let content = Bytes::from("test");
        let id = store.insert(content);
        assert!(store.contains(&id));
        assert!(!store.contains(&ObjectId::from_content(b"other")));
    }
}
