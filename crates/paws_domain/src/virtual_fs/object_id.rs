use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Content-addressed identifier for file objects.
///
/// An `ObjectId` is a SHA-256 hash of the file's content, similar to how
/// Git identifies blobs. Two files with identical content will always
/// produce the same `ObjectId`, enabling deduplication in the object store.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectId(String);

impl ObjectId {
    /// Computes an `ObjectId` from raw byte content using SHA-256.
    pub fn from_content(content: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(content);
        Self(format!("{:x}", hasher.finalize()))
    }

    /// Returns the hex-encoded hash string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ObjectId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_deterministic_hash() {
        let content = b"hello world";
        let id1 = ObjectId::from_content(content);
        let id2 = ObjectId::from_content(content);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_different_content_produces_different_id() {
        let id1 = ObjectId::from_content(b"hello");
        let id2 = ObjectId::from_content(b"world");
        assert!(id1 != id2);
    }

    #[test]
    fn test_empty_content() {
        let id = ObjectId::from_content(b"");
        assert!(!id.as_str().is_empty());
    }

    #[test]
    fn test_display() {
        let id = ObjectId::from_content(b"test");
        let displayed = format!("{id}");
        assert_eq!(displayed, id.as_str());
    }
}
