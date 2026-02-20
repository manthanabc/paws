# Paws HTTP API Specification

## Overview

The Paws HTTP Server provides a RESTful API for interacting with the Paws AI assistant. The server supports both standard JSON responses and Server-Sent Events (SSE) for streaming operations.

**Base URL:** `http://localhost:3000`

**Default Port:** `3000`

**CORS:** Enabled (permissive)

---

## Table of Contents

- [General](#general)
- [Resources](#resources)
- [Chat & Execution](#chat--execution)
- [Conversations](#conversations)
- [Configuration](#configuration)
- [MCP (Model Context Protocol)](#mcp-model-context-protocol)
- [Authentication](#authentication)
- [Platform Authentication](#platform-authentication)

---

## General

### Health Check

Check if the server is running.

```http
GET /api/health
```

**Response:** `200 OK`

```json
"OK"
```

---

### Environment Information

Get environment information including current working directory and other system details.

```http
GET /api/env
```

**Response:** `200 OK`

```json
{
  "cwd": "/path/to/current/directory",
  "home": "/home/user",
  "os": "linux",
  "arch": "x86_64"
}
```

---

## Resources

### Discover Files

List files in the current working directory.

```http
GET /api/files
```

**Response:** `200 OK`

```json
[
  {
    "name": "file.txt",
    "path": "/path/to/file.txt",
    "is_dir": false,
    "size": 1024
  }
]
```

---

### Get Tools

List all available tools.

```http
GET /api/tools
```

**Response:** `200 OK`

```json
[
  {
    "name": "read",
    "description": "Read file contents",
    "parameters": {
      "type": "object",
      "properties": {
        "path": { "type": "string" }
      }
    }
  }
]
```

---

### Get Models

List all available models.

```http
GET /api/models
```

**Response:** `200 OK`

```json
[
  {
    "id": "claude-3-opus",
    "name": "Claude 3 Opus",
    "provider_id": "anthropic",
    "context_length": 200000,
    "tools_supported": true
  }
]
```

---

### Get Agents

List all available agents.

```http
GET /api/agents
```

**Response:** `200 OK`

```json
[
  {
    "id": "sage",
    "title": "Research Agent",
    "description": "Performs research and investigation",
    "model": "claude-3-opus",
    "provider_id": "anthropic"
  }
]
```

---

### Get Active Agent

Get the currently active agent ID.

```http
GET /api/active-agent
```

**Response:** `200 OK`

```json
{
  "agent_id": "sage"
}
```

---

### Set Active Agent

Set the active agent.

```http
POST /api/active-agent
```

**Request Body:**

```json
{
  "agent_id": "sage"
}
```

**Response:** `200 OK`

---

### Get Providers

List all available providers.

```http
GET /api/providers
```

**Response:** `200 OK`

```json
[
  {
    "id": "anthropic",
    "name": "Anthropic",
    "provider_type": "llm",
    "url": "https://api.anthropic.com",
    "is_configured": true
  }
]
```

---

### Get Provider

Get details for a specific provider.

```http
GET /api/providers/:id
```

**Path Parameters:**

| Parameter | Type   | Description       |
|-----------|--------|-------------------|
| id        | string | Provider ID       |

**Response:** `200 OK`

```json
{
  "id": "anthropic",
  "name": "Anthropic",
  "provider_type": "llm",
  "url": "https://api.anthropic.com",
  "is_configured": true,
  "auth_methods": ["api_key", "oauth"]
}
```

---

### Get Skills

List all available skills.

```http
GET /api/skills
```

**Response:** `200 OK`

```json
[
  {
    "name": "debug-cli",
    "description": "Debug CLI commands",
    "path": "/path/to/skill"
  }
]
```

---

### Get Workflow

Get the merged workflow configuration.

```http
GET /api/workflow
```

**Response:** `200 OK`

```json
{
  "steps": [...],
  "configuration": {...}
}
```

---

### Get Commands

List all available commands.

```http
GET /api/commands
```

**Response:** `200 OK`

```json
[
  {
    "name": "test",
    "description": "Run tests"
  }
]
```

---

## Chat & Execution

### Chat

Send a chat message and receive a streaming response via SSE.

```http
POST /api/chat
```

**Request Body:**

```json
{
  "prompt": "Hello, how are you?",
  "conversation_id": "optional-conversation-id",
  "agent_id": "sage",
  "context": {}
}
```

**Response:** `200 OK` (SSE Stream)

Each event contains a JSON object:

```json
{
  "type": "content",
  "content": "Hello! I'm doing well, thank you.",
  "is_complete": false
}
```

**SSE Events:**

| Event Type | Description                |
|------------|----------------------------|
| data       | Response content           |
| error      | Error message              |
| keepalive  | Keep-alive ping (default)  |

---

### Execute Command

Execute a shell command.

```http
POST /api/command
```

**Request Body:**

```json
{
  "command": "ls -la",
  "working_dir": "/optional/path"
}
```

**Response:** `200 OK`

```json
{
  "stdout": "file1.txt\nfile2.txt\n",
  "stderr": "",
  "exit_code": 0,
  "success": true
}
```

---

### Generate Command

Generate a shell command from a natural language prompt.

```http
POST /api/generate-command
```

**Request Body:**

```json
{
  "prompt": "List all files in the current directory"
}
```

**Response:** `200 OK`

```json
{
  "command": "ls -la",
  "explanation": "Lists all files including hidden ones"
}
```

---

### Generate Data

Generate data from JSONL configuration via SSE stream.

```http
POST /api/data/generate
```

**Request Body:**

```json
{
  "jsonl_path": "/path/to/config.jsonl",
  "output_path": "/path/to/output",
  "num_samples": 10
}
```

**Response:** `200 OK` (SSE Stream)

Each event contains generated data:

```json
{
  "sample": 1,
  "data": {...},
  "status": "generating"
}
```

---

## Conversations

### Get Conversations

List all conversations.

```http
GET /api/conversations?limit=10
```

**Query Parameters:**

| Parameter | Type    | Description                | Default |
|-----------|---------|----------------------------|---------|
| limit     | integer | Maximum number of results  | null    |

**Response:** `200 OK`

```json
[
  {
    "id": "conv-123",
    "title": "Project Setup",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T01:00:00Z"
  }
]
```

---

### Create Conversation

Create a new conversation with minimal required fields.

```http
POST /api/conversations
```

**Request Body:**

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Optional Title"
}
```

| Field | Type   | Required | Description                    |
|-------|--------|----------|--------------------------------|
| id    | string | Yes      | UUID for the conversation      |
| title | string | No       | Optional title for display     |

**Response:** `201 Created`

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Optional Title",
  "created_at": "2024-01-01T00:00:00Z"
}
```

---

### Update Conversation

Update an existing conversation with full conversation data.

```http
PUT /api/conversations/:id
```

**Path Parameters:**

| Parameter | Type   | Description       |
|-----------|--------|-------------------|
| id        | string | Conversation ID    |

**Request Body:** Full `Conversation` object

**Response:** `200 OK`

**Response:** `400 Bad Request` (if path ID doesn't match body ID)

---

### Get Conversation

Get details of a specific conversation.

```http
GET /api/conversations/:id
```

**Path Parameters:**

| Parameter | Type   | Description       |
|-----------|--------|-------------------|
| id        | string | Conversation ID    |

**Response:** `200 OK`

```json
{
  "id": "conv-123",
  "title": "Project Setup",
  "context": {...},
  "metrics": {...},
  "metadata": {
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T01:00:00Z"
  }
}
```

**Response:** `404 Not Found`

```json
{
  "error": "Conversation not found: conv-123"
}
```

---

### Delete Conversation

Delete a conversation.

```http
DELETE /api/conversations/:id
```

**Path Parameters:**

| Parameter | Type   | Description       |
|-----------|--------|-------------------|
| id        | string | Conversation ID    |

**Response:** `204 No Content`

---

### Compact Conversation

Compact a conversation to reduce context size.

```http
POST /api/conversations/:id/compact
```

**Path Parameters:**

| Parameter | Type   | Description       |
|-----------|--------|-------------------|
| id        | string | Conversation ID    |

**Response:** `200 OK`

```json
{
  "original_size": 1000,
  "compressed_size": 200,
  "compression_ratio": 0.2
}
```

---

## Configuration

### Get Default Provider

Get the default provider.

```http
GET /api/config/default-provider
```

**Response:** `200 OK`

```json
{
  "provider_id": "anthropic"
}
```

---

### Set Default Provider

Set the default provider.

```http
POST /api/config/default-provider
```

**Request Body:**

```json
{
  "provider_id": "anthropic"
}
```

**Response:** `200 OK`

---

### Get Default Model

Get the default model.

```http
GET /api/config/default-model
```

**Response:** `200 OK`

```json
{
  "model_id": "claude-3-opus"
}
```

---

### Set Default Model

Set the default model.

```http
POST /api/config/default-model
```

**Request Body:**

```json
{
  "model_id": "claude-3-opus"
}
```

**Response:** `200 OK`

---

## MCP (Model Context Protocol)

### Read MCP Config

Read MCP configuration.

```http
GET /api/mcp/config?scope=user
```

**Query Parameters:**

| Parameter | Type   | Description              | Default |
|-----------|--------|--------------------------|---------|
| scope     | string | Configuration scope     | null    |

**Response:** `200 OK`

```json
{
  "servers": [
    {
      "name": "filesystem",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path"]
    }
  ]
}
```

---

### Write MCP Config

Write MCP configuration.

```http
POST /api/mcp/config
```

**Request Body:**

```json
{
  "scope": "user",
  "config": {
    "servers": [...]
  }
}
```

**Response:** `200 OK`

**Response:** `400 Bad Request` (if scope or config missing)

---

### Reload MCP

Reload MCP servers.

```http
POST /api/mcp/reload
```

**Response:** `200 OK`

---

## Authentication

### Init Provider Auth

Initialize provider authentication.

```http
POST /api/auth/init
```

**Request Body:**

```json
{
  "provider_id": "anthropic",
  "method": "api_key"
}
```

**Response:** `200 OK`

```json
{
  "auth_url": "https://auth.example.com",
  "state": "random-state-string",
  "expires_in": 300
}
```

---

### Complete Provider Auth

Complete provider authentication.

```http
POST /api/auth/complete
```

**Request Body:**

```json
{
  "provider_id": "anthropic",
  "context": {
    "code": "auth-code",
    "state": "random-state-string"
  },
  "timeout_secs": 60
}
```

**Response:** `200 OK`

---

### Logout

Logout from a provider or platform.

```http
POST /api/auth/logout
```

**Request Body:**

```json
{
  "provider_id": "anthropic"
}
```

**Response:** `200 OK`

**Note:** Omit `provider_id` to logout from platform.

---

### User Info

Get provider user information.

```http
GET /api/auth/user
```

**Response:** `200 OK`

```json
{
  "user_id": "user-123",
  "email": "user@example.com",
  "name": "John Doe"
}
```

---

### User Usage

Get provider usage information.

```http
GET /api/auth/usage
```

**Response:** `200 OK`

```json
{
  "total_tokens": 100000,
  "input_tokens": 60000,
  "output_tokens": 40000,
  "requests": 500
}
```

---

## Platform Authentication

### Init Platform Login

Initialize platform login.

```http
POST /api/platform/auth/init
```

**Response:** `200 OK`

```json
{
  "auth_url": "https://platform.example.com/auth",
  "state": "random-state-string"
}
```

---

### Platform Login

Complete platform login.

```http
POST /api/platform/auth/login
```

**Request Body:**

```json
{
  "auth_url": "https://platform.example.com/auth",
  "state": "random-state-string",
  "code": "auth-code"
}
```

**Response:** `200 OK`

---

### Platform User Info

Get platform login information.

```http
GET /api/platform/auth/info
```

**Response:** `200 OK`

```json
{
  "user_id": "user-123",
  "email": "user@example.com",
  "name": "John Doe",
  "is_authenticated": true
}
```

---

## Error Responses

All endpoints return error responses in JSON format:

```json
{
  "error": "Error message describing what went wrong",
  "details": "Optional additional context (only present when relevant)"
}
```

**Common HTTP Status Codes:**

| Status Code | Description              |
|-------------|--------------------------|
| 200         | Success                  |
| 201         | Created                  |
| 204         | No Content               |
| 400         | Bad Request              |
| 404         | Not Found                |
| 422         | Unprocessable Entity     |
| 500         | Internal Server Error    |

**Example Error Responses:**

`404 Not Found`:
```json
{
  "error": "Conversation not found: 550e8400-e29b-41d4-a716-446655440000"
}
```

`400 Bad Request`:
```json
{
  "error": "Conversation ID in path does not match body"
}
```

`500 Internal Server Error`:
```json
{
  "error": "Database connection failed"
}
```

---

## SSE (Server-Sent Events) Format

For streaming endpoints (`/api/chat`, `/api/data/generate`), responses use SSE format:

```
data: {"type":"content","content":"Hello","is_complete":false}

data: {"type":"content","content":"!","is_complete":true}

event: error
data: Connection lost

: keep-alive
```

**Event Types:**

| Event    | Description                      |
|----------|----------------------------------|
| data     | Normal data payload              |
| error    | Error occurred                   |
| (no type)| Keep-alive ping (default event)  |

---

## Running the Server

### Using Cargo

```bash
cargo run -- serve --port 3000
```

### Using Binary

```bash
paws serve --port 3000
```

### CLI Options

| Option   | Type   | Description          | Default |
|----------|--------|----------------------|---------|
| --port   | number | Port to listen on    | 3000    |

---

## Notes

- All datetime fields use ISO 8601 format (e.g., `2024-01-01T00:00:00Z`)
- IDs are strings and should be treated as opaque values
- CORS is enabled for all origins (permissive mode)
- The server logs all requests via the tracing layer
- Streaming endpoints use SSE with automatic keep-alive (default 15 seconds)
