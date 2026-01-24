use derive_setters::Setters;
use serde::{Deserialize, Serialize};

use crate::{Environment, File, Skill};

/// Maps tool names to their string representations for template rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolNames {
    pub shell: String,
    pub fs_search: String,
    pub read: String,
    pub write: String,
    pub patch: String,
    pub undo: String,
}

impl ToolNames {
    /// Creates a new ToolNames instance with default tool names
    pub fn new() -> Self {
        Self {
            shell: "shell".to_string(),
            fs_search: "fs_search".to_string(),
            read: "read".to_string(),
            write: "write".to_string(),
            patch: "patch".to_string(),
            undo: "undo".to_string(),
        }
    }
}

impl Default for ToolNames {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Setters, Clone, Serialize, Deserialize)]
#[setters(strip_option)]
#[derive(Default)]
pub struct SystemContext {
    // Environment information to be included in the system context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Environment>,

    // Information about available tools that can be used by the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_information: Option<String>,

    /// Indicates whether the agent supports tools.
    /// This value is populated directly from the Agent configuration.
    #[serde(default)]
    pub tool_supported: bool,

    // List of files and directories that are relevant for the agent context
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<File>,

    #[serde(skip_serializing_if = "String::is_empty")]
    pub custom_rules: String,

    /// Indicates whether the agent supports parallel tool calls.
    #[serde(default)]
    pub supports_parallel_tool_calls: bool,

    /// List of available skills (always serialized even if empty to satisfy
    /// handlebars strict mode)
    #[serde(default)]
    pub skills: Vec<Skill>,

    /// Tool name mappings for template rendering
    #[serde(default)]
    pub tool_names: ToolNames,
}
