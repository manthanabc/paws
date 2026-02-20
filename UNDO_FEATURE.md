# Undo System

The undo system allows you to revert the last interaction and restore the conversation to its previous state.

## How It Works

1. **Automatic Snapshots**: Before each user interaction, Paws automatically creates a snapshot of the conversation context.
2. **Snapshot History**: Up to 50 snapshots are stored per conversation to allow multiple undo operations.
3. **Snapshot Cleanup**: When the limit is reached, the oldest snapshots are automatically removed to maintain performance.

## Usage

### Undo Command

```
/undo
```

Use the `/undo` command to revert the last interaction. This will:
- Restore the conversation context to the state before the last interaction
- Remove all messages, tool calls, and results from that interaction
- Display a confirmation message showing what was undone

### Examples

```
> Can you add a new feature to the codebase?
< [AI response with code changes]

> /undo
Undone: Can you add a new feature to the codebase?
[Conversation reverted to state before the feature request]
```

### Multiple Undo Operations

You can call `/undo` multiple times to step back through your conversation history:

```
> First question
< Answer 1

> Second question
< Answer 2

> Third question
< Answer 3

> /undo
Undone: Third question
[Back to after Answer 2]

> /undo
Undone: Second question
[Back to after Answer 1]
```

### When Undo is Not Available

If there are no snapshots to undo (i.e., at the start of a conversation), you'll see:

```
> /undo
No more actions to undo
```

## Technical Details

- **Snapshot Limit**: 50 snapshots per conversation
- **Storage**: Snapshots are persisted with the conversation in the database
- **Performance**: Snapshots are lightweight and contain only the conversation context state
- **Summary**: Each snapshot includes a brief summary (first 100 characters of the user message) for display purposes

## Implementation

The undo system is implemented through:

1. **Domain Model** (`Conversation` struct):
   - `context_history: Vec<ContextSnapshot>` - stores snapshot history
   - `save_snapshot(summary)` - creates and saves a snapshot
   - `undo()` - reverts to the previous snapshot
   - `can_undo()` - checks if undo is available

2. **Orchestrator** (in `paws_app`):
   - Automatically creates snapshots before processing each user interaction

3. **Services** (`ConversationService`):
   - `undo_conversation(&conversation_id)` - performs the undo operation

4. **UI** (`SlashCommand::Undo`):
   - `/undo` command to trigger undo from the interactive prompt

## Future Enhancements

Potential future improvements:
- Redo functionality to reverse an undo
- Named snapshots for bookmarking important conversation states
- Snapshot browsing to see and select from history
- Configurable snapshot limit
- Differential snapshots to reduce memory usage for large conversations
