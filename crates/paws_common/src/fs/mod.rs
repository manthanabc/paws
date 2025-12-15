//! # PawsFS
//!
//! A file system abstraction layer that standardizes error handling for file
//! operations.
//!
//! PawsFS wraps tokio's filesystem operations with consistent error context
//! using anyhow::Context. Each method provides standardized error messages in
//! the format "Failed to [operation] [path]", ensuring uniform error reporting
//! throughout the application while preserving the original error cause.

mod binary_detection;
mod error;
mod file_size;
mod is_binary;
mod meta;
mod read;
mod read_range;
mod write;

pub use crate::fs::binary_detection::is_binary;
pub use crate::fs::error::Error;

mod file_info;
pub use file_info::FileInfo;

/// PawsFS provides a standardized interface for file system operations
/// with consistent error handling.
#[derive(Debug)]
pub struct PawsFS;
