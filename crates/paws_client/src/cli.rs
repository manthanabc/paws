use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use paws_domain::{AgentId, ConversationId, ProviderId};

#[derive(Parser)]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    /// Direct prompt to process without entering interactive mode.
    ///
    /// When provided, executes a single command and exits instead of starting
    /// an interactive session. Content can also be piped: `cat prompt.txt |
    /// paws`.
    #[arg(long, short = 'p', allow_hyphen_values = true)]
    pub prompt: Option<String>,

    /// Piped input from stdin (populated internally)
    ///
    /// This field is automatically populated when content is piped to paws
    /// via stdin. It's kept separate from the prompt to allow proper handling
    /// as a droppable message.
    #[arg(skip)]
    pub piped_input: Option<String>,

    /// Path to a JSON file containing the conversation to execute.
    #[arg(long)]
    pub conversation: Option<PathBuf>,

    /// Conversation ID to use for this session.
    ///
    /// When provided, resumes or continues an existing conversation instead of
    /// generating a new conversation ID.
    #[arg(long, alias = "cid")]
    pub conversation_id: Option<ConversationId>,

    /// Working directory to use before starting the session.
    ///
    /// When provided, changes to this directory before starting paws.
    #[arg(long, short = 'C')]
    pub directory: Option<PathBuf>,

    /// Name for an isolated git worktree to create for experimentation.
    #[arg(long)]
    pub sandbox: Option<String>,

    /// Enable verbose logging output.
    #[arg(long, default_value_t = false)]
    pub verbose: bool,

    /// Use restricted shell (rbash) for enhanced security.
    #[arg(long, default_value_t = false, short = 'r')]
    pub restricted: bool,

    /// Agent ID to use for this session.
    #[arg(long, alias = "aid")]
    pub agent: Option<AgentId>,

    /// Top-level subcommands.
    #[command(subcommand)]
    pub subcommands: Option<TopLevelCommand>,

    /// Path to a file containing the workflow to execute.
    #[arg(long, short = 'w')]
    pub workflow: Option<PathBuf>,

    /// Event to dispatch to the workflow in JSON format.
    #[arg(long, short = 'e')]
    pub event: Option<String>,
}

impl Cli {
    /// Determines whether the CLI should start in interactive mode.
    ///
    /// Returns true when no prompt, piped input, or subcommand is provided,
    /// indicating the user wants to enter interactive mode.
    pub fn is_interactive(&self) -> bool {
        self.prompt.is_none() && self.piped_input.is_none() && self.subcommands.is_none()
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum TopLevelCommand {
    /// Start the background server.
    Server {
        /// Unix socket path for IPC
        #[arg(long)]
        socket: Option<PathBuf>,

        /// Enable verbose logging
        #[arg(long)]
        verbose: bool,
    },

    /// Manage agents.
    Agent(AgentCommandGroup),

    /// Generate shell extension scripts.
    #[command(hide = true)]
    Extension(ExtensionCommandGroup),

    /// List agents, models, providers, tools, or MCP servers.
    List(ListCommandGroup),

    /// Display the banner with version information.
    Banner,

    /// Show configuration, active model, and environment status.
    Info {
        /// Conversation ID for session-specific information.
        #[arg(long, alias = "cid")]
        conversation_id: Option<ConversationId>,

        /// Output in machine-readable format.
        #[arg(long)]
        porcelain: bool,
    },

    /// Display environment information.
    Env,

    /// Get, set, or list configuration values.
    Config(ConfigCommandGroup),

    /// Manage conversation history and state.
    #[command(alias = "session")]
    Conversation(ConversationCommandGroup),

    /// Generate and optionally commit changes with AI-generated message
    Commit(CommitCommandGroup),

    /// Manage Model Context Protocol servers.
    Mcp(McpCommandGroup),

    /// Suggest shell commands from natural language.
    Suggest {
        /// Natural language description of the desired command.
        prompt: String,
    },

    /// Manage API provider authentication.
    Provider(ProviderCommandGroup),

    /// Run or list custom commands.
    Cmd(CmdCommandGroup),

    /// Process JSONL data through LLM with schema-constrained tools.
    Data(DataCommandGroup),
}

// For now, include the minimal required command structs
// The full CLI structure would be copied from the original paws_main

/// Command group for agent management.
#[derive(Parser, Debug, Clone)]
pub struct AgentCommandGroup {
    #[command(subcommand)]
    pub command: AgentCommand,
}

/// Agent management commands.
#[derive(Subcommand, Debug, Clone)]
pub enum AgentCommand {
    /// List available agents.
    #[command(alias = "ls")]
    List,
}

/// Configuration scope for settings.
#[derive(Copy, Clone, Debug, ValueEnum, Default)]
pub enum Scope {
    /// Local configuration (project-specific).
    #[default]
    Local,
    /// User configuration (global to the user).
    User,
}

/// Command group for listing resources.
#[derive(Parser, Debug, Clone)]
pub struct ListCommandGroup {
    #[command(subcommand)]
    pub command: ListCommand,
}

/// List commands.
#[derive(Subcommand, Debug, Clone)]
pub enum ListCommand {
    /// List available agents.
    #[command(alias = "agents")]
    Agent,
}

/// Generate shell extensions.
#[derive(Parser, Debug, Clone)]
pub struct ExtensionCommandGroup {
    #[command(subcommand)]
    pub command: ExtensionCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ExtensionCommand {
    /// Generate ZSH extension script.
    Zsh,
}

/// MCP server management.
#[derive(Parser, Debug, Clone)]
pub struct McpCommandGroup {
    #[command(subcommand)]
    pub command: McpCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum McpCommand {
    /// List configured servers.
    List,
}

/// Configuration management.
#[derive(Parser, Debug, Clone)]
pub struct ConfigCommandGroup {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigCommand {
    /// List configuration values.
    List,
}

/// Conversation management.
#[derive(Parser, Debug, Clone)]
pub struct ConversationCommandGroup {
    #[command(subcommand)]
    pub command: ConversationCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConversationCommand {
    /// List conversation history.
    List,
}

/// Commit commands.
#[derive(Parser, Debug, Clone)]
pub struct CommitCommandGroup {
    /// Preview changes without applying them.
    #[arg(long)]
    pub preview: bool,
}

/// Provider authentication management.
#[derive(Parser, Debug, Clone)]
pub struct ProviderCommandGroup {
    #[command(subcommand)]
    pub command: ProviderCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ProviderCommand {
    /// Login to provider.
    Login,
}

/// Custom command management.
#[derive(Parser, Debug, Clone)]
pub struct CmdCommandGroup {
    #[command(subcommand)]
    pub command: CmdCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CmdCommand {
    /// List custom commands.
    List,
}

/// Data processing commands.
#[derive(Parser, Debug, Clone)]
pub struct DataCommandGroup {
    #[command(subcommand)]
    pub command: DataCommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DataCommand {
    /// Process data.
    Process {
        /// Data parameters as JSON
        params: String,
    },
}