# Agent Client Protocol (ACP) Support

Paws implements the [Agent Client Protocol](https://github.com/agentclientprotocol/spec) (ACP), allowing it to be controlled by any ACP-compatible client. This enables integration with various development tools, IDEs, and custom automation workflows.

## What is ACP?

The Agent Client Protocol is a standard JSON-RPC protocol for communication between AI agents and client applications. It defines:

- Session management (create, load, and manage conversation sessions)
- Prompt/response streaming
- Authentication mechanisms
- Extensibility through custom methods

## Starting Paws in ACP Server Mode

To start Paws as an ACP server, use the `acp` subcommand:

```bash
paws acp
```

This will start Paws in server mode, communicating over stdin/stdout using the ACP protocol.

## Session Management

### Creating a New Session

When a client calls `newSession`, Paws will:
1. Generate a new conversation ID (UUID)
2. Initialize an empty conversation
3. Return the session ID to the client

### Loading an Existing Session

When a client calls `loadSession` with a session ID:
1. Paws checks if the conversation exists
2. If it doesn't exist, a new conversation is created with that ID
3. The session is ready to accept prompts

### Sending Prompts

Clients can send prompts using the `prompt` method:
- The prompt content is converted to a Paws `UserPrompt`
- Paws processes the prompt through its chat pipeline
- Responses are streamed back as `AgentMessageChunk` notifications

## Integration Examples

### Using with a Custom Client

Here's a minimal example of connecting to Paws via ACP:

```rust
use agent_client_protocol::{self as acp, Agent as _};
use tokio::process::Command;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Start Paws in ACP mode
    let mut child = Command::new("paws")
        .arg("acp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let stdin = child.stdin.take().unwrap().compat_write();
    let stdout = child.stdout.take().unwrap().compat();

    // Connect to the agent
    let (connection, handle_io) = acp::ClientSideConnection::new(
        stdin,
        stdout,
        |fut| { tokio::spawn(fut); }
    );

    // Run IO in background
    tokio::spawn(async move {
        handle_io.await.ok();
    });

    // Initialize the connection
    let init_response = connection.initialize(acp::InitializeRequest {
        protocol_version: acp::V1,
        client_info: acp::Implementation {
            name: "example-client".to_string(),
            title: None,
            version: "0.1.0".to_string(),
        },
        meta: None,
    }).await?;

    println!("Connected to: {:?}", init_response.agent_info);

    // Create a new session
    let session = connection.new_session(acp::NewSessionRequest {
        meta: None,
    }).await?;

    println!("Session created: {}", session.session_id.0);

    // Send a prompt
    let prompt_response = connection.prompt(acp::PromptRequest {
        session_id: session.session_id,
        prompt: vec![
            acp::Content::Text(acp::TextContent {
                text: "Hello, Paws!".to_string(),
                annotations: None,
            })
        ],
        meta: None,
    }).await?;

    println!("Prompt completed: {:?}", prompt_response.stop_reason);

    Ok(())
}
```

### Using with Claude Desktop

Add Paws as an MCP server in Claude Desktop's configuration:

```json
{
  "mcpServers": {
    "paws": {
      "command": "paws",
      "args": ["acp"]
    }
  }
}
```

## Features Supported

- ✅ Session management (create, load)
- ✅ Prompt/response streaming
- ✅ Text content
- ✅ Conversation persistence
- ⚠️ Image content (recognized but not fully processed)
- ⚠️ Document content (recognized but not fully processed)
- ❌ Authentication (no-op, returns success)
- ❌ Session modes
- ❌ Model selection per session

## Implementation Details

### Conversation Mapping

ACP sessions map directly to Paws conversations:
- ACP `session_id` = Paws `conversation_id` (UUID format)
- Sessions are persisted in Paws' conversation store
- All Paws features (tools, MCP servers, etc.) work in ACP mode

### Streaming

Paws streams responses back to the client in real-time:
- Each chunk of text is sent as an `AgentMessageChunk` notification
- Tool calls and reasoning are filtered out (only final text is sent)
- The stream completes with a `PromptResponse` containing `EndTurn` stop reason

### Error Handling

Errors are mapped to ACP error codes:
- Invalid session IDs → `InvalidParams`
- Chat failures → `InternalError`
- Stream errors → `InternalError`

All errors are logged via tracing for debugging.

## Troubleshooting

### Session Not Found

If a client tries to use a session that doesn't exist, Paws will automatically create it. This is by design to allow flexible session management.

### Connection Issues

Make sure:
1. Paws is started with the `acp` subcommand
2. The client is reading from stdout and writing to stdin
3. No other output is being written to stdout (Paws suppresses normal output in ACP mode)

### Logging

To enable debug logging for ACP operations:

```bash
FORGE_LOG=debug paws acp
```

Logs are written to `~/.paws/logs/paws.log` by default.

## See Also

- [Agent Client Protocol Specification](https://github.com/agentclientprotocol/spec)
- [Rust SDK](https://github.com/agentclientprotocol/rust-sdk)
- [Paws Configuration Guide](./configuration.md)
