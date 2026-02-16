//! Virtual file system with content-addressed storage for parallel sessions.
//!
//! This module provides an in-memory file system that allows multiple
//! independent sessions to read and write files without touching the disk.
//! File content is stored once using SHA-256 content addressing, enabling
//! efficient deduplication across sessions.
//!
//! ## Core Types
//!
//! - [`ObjectId`] – SHA-256 content hash identifying a file blob.
//! - [`ObjectStore`] – Thread-safe, deduplicated blob storage.
//! - [`SessionFS`] – Per-session file overlay with snapshot/undo support.
//! - [`VirtualFileSystem`] – Orchestrator managing sessions and the shared
//!   store.
//! - [`SessionId`] – Unique identifier for a session.

mod object_id;
mod object_store;
mod session_fs;
mod virtual_file_system;

pub use object_id::ObjectId;
pub use object_store::ObjectStore;
pub use session_fs::SessionFS;
pub use virtual_file_system::{SessionId, VirtualFileSystem};
