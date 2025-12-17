# Simple PAWS Client-Server Demo

This is a simplified demonstration of a client-server architecture for the Paws AI assistant.

## Architecture

The codebase has been refactored to support two modes:

1. **Server Mode**: `paws server` - Runs the business logic server in the background
2. **Client Mode**: `paws` - Runs the terminal UI client that connects to the server

## Usage

### Start the server manually:
```bash
paws server --socket /tmp/paws.sock --verbose
```

### Run the client (auto-starts server if needed):
```bash
paws --verbose
```

## Features Demonstrated

- **Single Binary**: The same executable runs in both client and server modes
- **Auto-Start**: Client automatically starts the server if it's not running
- **IPC**: Unix domain sockets for inter-process communication
- **Clean Architecture**: Server contains all business logic, client handles UI
- **Graceful Shutdown**: Signal handling for clean server shutdown

## Protocol

The communication uses simple line-based protocol:
- Client sends: `REQUEST:<command>\n`
- Server responds: `OK:<data>\n` or `ERROR:<message>\n`

### Supported Commands
- `ping` - Server responds with `pong`
- `status` - Server responds with `server_running`
- `version` - Server responds with version info
- `shutdown` - Server shuts down gracefully

## Benefits

1. **Separation of Concerns**: UI logic separate from business logic
2. **Resource Efficiency**: Server can be reused across multiple client sessions
3. **Process Isolation**: Crashes in UI don't affect the server
4. **Future Extensibility**: Easy to add remote clients or web interface

## Next Steps

This foundation can be extended to:
- Add full Paws API functionality to the server
- Implement proper authentication
- Add streaming support for chat responses
- Support multiple simultaneous client connections
- Add server persistence and data management