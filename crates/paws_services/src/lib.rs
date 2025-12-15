mod agent_registry;
mod app_config;
mod attachment;
mod auth;
mod clipper;
mod command;
mod context_engine;
mod conversation;
mod discovery;
mod env;
mod error;
mod http;
mod instructions;
mod mcp;
mod paws_services;
mod policy;
mod provider;
mod provider_auth;
mod range;
pub mod snaps;
mod template;
mod tool_services;
pub mod tracker;
mod utils;
mod workflow;

pub use app_config::*;
pub use clipper::*;
pub use command::*;
pub use context_engine::*;
pub use discovery::*;
pub use error::*;
pub use instructions::*;
pub use paws_services::*;
pub use policy::*;
pub use provider_auth::*;

/// Converts a type from its external representation into its domain model
/// representation.
pub trait IntoDomain {
    type Domain;

    fn into_domain(self) -> Self::Domain;
}
