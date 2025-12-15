/// Shared traits and types for DTO modules.
///
/// This module provides common abstractions used across both OpenAI and Anthropic DTOs.

/// Trait for types that support cache control operations.
///
/// Implementors can enable or disable caching and check if caching is enabled.
pub trait Cacheable: Sized {
    /// Set cache control on this item.
    ///
    /// When `enable` is true, caching will be enabled. When false, any existing
    /// cache control will be removed.
    fn cached(self, enable: bool) -> Self;

    /// Check if this item has cache control enabled.
    fn is_cached(&self) -> bool;
}
