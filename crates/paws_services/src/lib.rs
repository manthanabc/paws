mod agent_registry;
mod app_config;
mod attachment;
mod auth;
mod clipper;
mod command;
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

#[cfg(test)]
mod attachment_tests;
#[cfg(test)]
pub mod test_fixtures;

pub use app_config::*;
pub use clipper::*;
pub use command::*;
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
