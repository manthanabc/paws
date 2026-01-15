# Paws UI and Event Control Flow State Diagram

## PlantUML State Diagram

```plantuml
@startuml
skinparam state {
    BackgroundColor<<UIState>> LightBlue
    BackgroundColor<<Processing>> LightYellow
    BackgroundColor<<API>> LightGreen
    BackgroundColor<<Response>> LightPink
    BackgroundColor<<Error>> LightRed
    BackgroundColor<<Command>> LightCyan
}

[*] --> UIInitialized : init()

state UIInitialized <<UIState>> {
    [*] --> Prompting : init_state()
    Prompting --> CommandParsing : User Input
    CommandParsing --> SlashCommand : Parse Successful
    CommandParsing --> Error : Parse Failed
    Error --> Prompting : Display Error
}

state SlashCommand <<Command>> {
    SlashCommand --> MessageCommand : /message or text
    SlashCommand --> AgentCommand : /agent or /paws, /muse, /sage
    SlashCommand --> ProviderCommand : /provider or /login, /logout
    SlashCommand --> ModelCommand : /model
    SlashCommand --> ConversationCommand : /conversations, /new, /delete
    SlashCommand --> ToolCommand : /tools
    SlashCommand --> ConfigCommand : /config
    SlashCommand --> ExitCommand : /exit
}

state MessageProcessing <<Processing>> {
    MessageCommand --> ConversationInit : on_message()
    ConversationInit --> ConversationLoaded : init_conversation()
    ConversationLoaded --> CreatingEvent : Build Event
    CreatingEvent --> ChatRequestCreated : Create ChatRequest
    ChatRequestCreated --> SendingToAPI : api.chat()
}

state APIProcessing <<API>> {
    SendingToAPI --> PawsAppProcessing : PawsApp::chat()
    PawsAppProcessing --> OrchestratorRunning : Orchestrator::run()
}

state OrchestratorLoop <<Processing>> {
    OrchestratorRunning --> BuildingContext : Build conversation context
    BuildingContext --> AddingSystemPrompt : SystemPrompt::new()
    AddingSystemPrompt --> AddingUserPrompt : UserPromptGenerator::new()
    AddingUserPrompt --> CheckingFiles : ChangedFiles::new()
    CheckingFiles --> ApplyingTunableParams : ApplyTunableParameters::new()
    ApplyingTunableParams --> SendingToLLM : execute_chat_turn()
}

state LLMCycle <<Processing>> {
    SendingToLLM --> WaitingForResponse : Stream open
    WaitingForResponse --> ReceivingStream : Stream data

    state ResponseHandling <<Response>> {
        ReceivingStream --> TaskReasoning : ChatResponse::TaskReasoning
        ReceivingStream --> TaskMessage : ChatResponse::TaskMessage
        ReceivingStream --> ToolCallStart : ChatResponse::ToolCallStart
        ReceivingStream --> ToolCallEnd : ChatResponse::ToolCallEnd
        ReceivingStream --> RetryAttempt : ChatResponse::RetryAttempt
        ReceivingStream --> Interrupt : ChatResponse::Interrupt
    }

    TaskReasoning --> DisplayReasoning : UI: handle_chat_response()
    DisplayReasoning --> WaitingForResponse

    TaskMessage --> DisplayContent : MarkdownWriter::add_chunk()
    DisplayContent --> WaitingForResponse

    ToolCallStart --> DisplayToolStart : Show tool name
    DisplayToolStart --> ExecutingTool : ToolExecutor::execute()
    ExecutingTool --> ToolCallEnd : Tool result ready
    ToolCallEnd --> DisplayToolResult : Show result (if verbose)
    DisplayToolResult --> UpdatingContext : context.append_message()

    state CompactionCheck <<Processing>> {
        UpdatingContext --> CheckCompaction : check_and_compact()
        CheckCompaction --> Compacting : Compaction needed
        CheckCompaction --> NoCompaction : No compaction needed

        Compacting --> UpdatingContextAfterCompaction : Context compacted
        NoCompaction --> UpdatingContextAfterCompaction : Context unchanged
    }

    UpdatingContextAfterCompaction --> CheckCompletion : Check finish_reason

    state CompletionCheck <<Processing>> {
        CheckCompletion --> TaskComplete : finish_reason == Stop
        CheckCompletion --> ContinueLoop : finish_reason != Stop
    }

    TaskComplete --> SavingConversation : services.upsert_conversation()
    ContinueLoop --> SendingToLLM : Next turn
}

state Saving <<Processing>> {
    SavingConversation --> Prompting : Return to prompt
    SavingConversation --> HeadlessExit : Non-interactive mode
}

state ErrorHandling <<Error>> {
    Interrupt --> DisplayInterrupt : Show interruption reason
    DisplayInterrupt --> PromptContinue : should_continue()
    PromptContinue --> SendingToLLM : User continues
    PromptContinue --> Prompting : User cancels

    RetryAttempt --> DisplayRetry : Show retry message
    DisplayRetry --> SendingToLLM : Retry with backoff
}

state AgentFlow <<Command>> {
    AgentCommand --> SelectingAgent : Agent selection
    SelectingAgent --> UpdatingActiveAgent : api.set_active_agent()
    UpdatingActiveAgent --> Prompting : Return to prompt
}

state ProviderFlow <<Command>> {
    ProviderCommand --> CheckingProviderConfig : Check if configured
    CheckingProviderConfig --> SelectingProvider : Provider selection
    SelectingProvider --> ConfiguringProvider : Authentication flow
    ConfiguringProvider --> ActivatingProvider : set_default_provider()
    ActivatingProvider --> SelectingModel : Model selection
    SelectingModel --> Prompting : Return to prompt
}

state ModelFlow <<Command>> {
    ModelCommand --> FetchingModels : api.get_models()
    FetchingModels --> DisplayingModels : Show model list
    DisplayingModels --> SelectingModel : User selects model
    SelectingModel --> UpdatingDefaultModel : api.set_default_model()
    UpdatingDefaultModel --> Prompting : Return to prompt
}

state ConversationManagement <<Command>> {
    ConversationCommand --> ListingConversations : /conversations
    ConversationCommand --> CreatingNewConversation : /new
    ConversationCommand --> DeletingConversation : /delete
    ConversationCommand --> CompactingConversation : /compact
    ConversationCommand --> DumpingConversation : /dump

    ListingConversations --> DisplayingList : Show conversation list
    DisplayingList --> Prompting

    CreatingNewConversation --> GeneratingNewID : ConversationId::generate()
    GeneratingNewID --> Prompting

    DeletingConversation --> ConfirmingDelete : User confirmation
    ConfirmingDelete --> RemovingConversation : api.delete_conversation()
    RemovingConversation --> Prompting

    CompactingConversation --> RunningCompaction : api.compact_conversation()
    RunningCompaction --> DisplayingMetrics : Show reduction stats
    DisplayingMetrics --> Prompting

    DumpingConversation --> ExportingJSON : Export as JSON
    DumpingConversation --> ExportingHTML : Export as HTML
    ExportingJSON --> Prompting
    ExportingHTML --> Prompting
}

state ToolCommands <<Command>> {
    ToolCommand --> FetchingTools : api.get_tools()
    FetchingTools --> DisplayingTools : Show tool list
    DisplayingTools --> Prompting
}

state ConfigCommands <<Command>> {
    ConfigCommand --> GettingConfig : /config get
    ConfigCommand --> SettingConfig : /config set
    ConfigCommand --> ListingConfig : /config list

    GettingConfig --> DisplayingValue : Show config value
    SettingConfig --> UpdatingValue : Update config
    ListingConfig --> DisplayingAllConfig : Show all config
    DisplayingValue --> Prompting
    UpdatingValue --> Prompting
    DisplayingAllConfig --> Prompting
}

ExitCommand --> [*] : Cleanup and exit

@enduml
```

## ASCII Art State Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           Paws UI & Event Flow                        │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────┐     ┌──────────────┐     ┌─────────────────┐
│   Startup   │────▶│   UI Init    │────▶│    Prompting    │
└─────────────┘     └──────────────┘     └────────┬────────┘
                                                   │
                                                   ▼
                                          ┌──────────────────┐
                                          │ Command Parsing │
                                          └───────┬────────┘
                                                  │
          ┌───────────────────────────────────────────┼─────────────────────────┐
          │                                   │                     │                 │
          ▼                                   ▼                     ▼                 ▼
    ┌──────────┐                      ┌────────────┐      ┌──────────┐    ┌──────────┐
    │ Message  │                      │   Agent    │      │ Provider │    │   Model  │
    └────┬─────┘                      └─────┬──────┘      └────┬─────┘    └────┬─────┘
         │                                   │                     │                 │
         ▼                                   ▼                     ▼                 ▼
    ┌──────────────┐                  ┌──────────┐         ┌──────────┐     ┌──────────┐
    │ Conversation  │                  │ Select   │         │ Configure │     │ Select   │
    │   Init       │                  │ Agent    │         │ Provider  │     │ Model    │
    └──────┬───────┘                  └────┬─────┘         └────┬─────┘     └────┬─────┘
           │                               │                     │                 │
           ▼                               ▼                     ▼                 ▼
    ┌──────────────┐                  ┌──────────────────────────────────────────────┐
    │  Create      │                  │              Update State              │
    │  ChatRequest  │                  │         (conversation_id, model, etc)     │
    └──────┬───────┘                  └────────────────────────┬─────────────────┘
           │                                            │
           ▼                                            ▼
    ┌──────────────┐                            ┌─────────────────┐
    │   Send to    │                            │  Return to     │
    │   API        │───────────────────────────────▶│   Prompting     │
    └──────┬───────┘                            └─────────────────┘
           │
           ▼
    ┌──────────────────────────────────────────────────────────────────────┐
    │                      PawsApp::chat()                          │
    └────────────────────────────┬─────────────────────────────────────┘
                             │
                             ▼
    ┌──────────────────────────────────────────────────────────────────────┐
    │                    Orchestrator::run()                         │
    │                                                              │
    │  ┌──────────────────────────────────────────────────────────┐   │
    │  │           Build Context                               │   │
    │  │  • SystemPrompt::new()                             │   │
    │  │  • UserPromptGenerator::new()                       │   │
    │  │  • ChangedFiles::new()                              │   │
    │  │  • ApplyTunableParameters::new()                     │   │
    │  └──────────────────────────┬───────────────────────────────┘   │
    │                         │                                      │
    │                         ▼                                      │
    │  ┌──────────────────────────────────────────────────────────┐   │
    │  │           Send to LLM                               │   │
    │  │  • execute_chat_turn()                             │   │
    │  └──────────────────────────┬───────────────────────────────┘   │
    │                         │                                      │
    │                         ▼                                      │
    │  ┌──────────────────────────────────────────────────────────┐   │
    │  │           Stream Responses                          │   │
    │  │  ┌────────────────────────────────────────────┐    │   │
    │  │  │ TaskReasoning    Display Reasoning      │    │   │
    │  │  │ TaskMessage      Display Content       │    │   │
    │  │  │ ToolCallStart   Execute Tool         │    │   │
    │  │  │ ToolCallEnd     Show Result          │    │   │
    │  │  │ RetryAttempt    Retry w/ Backoff     │    │   │
    │  │  │ Interrupt       Show Error          │    │   │
    │  │  └────────────────────────────────────────────┘    │   │
    │  └──────────────────────────┬───────────────────────────────┘   │
    │                         │                                      │
    │                         ▼                                      │
    │  ┌──────────────────────────────────────────────────────────┐   │
    │  │           Update Context                            │   │
    │  │  • context.append_message()                       │   │
    │  │  • Check compaction                              │   │
    │  │  • Check completion                              │   │
    │  └──────────────────────────┬───────────────────────────────┘   │
    │                         │                                      │
    │           ┌─────────────┴────────────┐                        │
    │           │                         │                        │
    │           ▼                         ▼                        │
    │    ┌─────────────┐         ┌─────────────┐                │
    │    │ Complete?   │         │ Continue?    │                │
    │    │   Yes       │         │   Yes        │                │
    │    └─────┬─────┘         └──────┬──────┘                │
    │          │                        │                         │
    │          ▼                        │                         │
    │    ┌─────────────┐             │                         │
    │    │ Save        │             │                         │
    │    │ Conversation │             │                         │
    │    └─────┬─────┘             │                         │
    │          │                    │                         │
    │          ▼                    │                         │
    │    ┌─────────────┐             │                         │
    │    │ Return to   │◀──────────┘                         │
    │    │  Prompting  │                                     │
    │    └─────────────┘                                     │
    │                                                         │
    └─────────────────────────────────────────────────────────────────┘
```

## Key Components and Their Roles

### UI Layer (`paws_main/src/ui.rs`)
- **UI**: Main orchestrator for user interface
- **UIState**: Maintains conversation_id and working directory
- **Console**: Handles user input via prompts
- **MarkdownWriter**: Renders markdown content to terminal

### Event Flow (`paws_domain/src/event.rs`)
- **Event**: Wraps user input (Text or Command)
- **EventValue**: Enum for Text(UserPrompt) or Command(UserCommand)
- **ChatRequest**: Contains Event + ConversationId

### Response Stream (`paws_domain/src/chat_response.rs`)
- **ChatResponse**: Streaming response enum
  - `TaskMessage`: Content (Title/PlainText/Markdown)
  - `TaskReasoning`: Agent reasoning output
  - `TaskComplete`: Turn completion signal
  - `ToolCallStart/End`: Tool execution notifications
  - `RetryAttempt`: Retry with backoff
  - `Interrupt`: Interruption (max requests/tool failures)

### Core Processing (`paws_app/src/app.rs`, `orch.rs`)
- **PawsApp**: Main application orchestrator
- **Orchestrator**: Manages agent execution loop
- **Context**: Conversation state with messages, tools, and configuration

### State Management
- **Conversation** (`paws_domain/src/conversation.rs`): Persistent conversation state
- **Context** (`paws_domain/src/context.rs`): Transient request/response state
- **ContextMessage**: Individual messages (Text, Tool, Image)

## Critical State Transitions

1. **User Input → Command Parsing**: Raw input parsed to `SlashCommand`
2. **Command → Event**: Commands converted to `Event` with conversation_id
3. **Event → API**: `ChatRequest` sent to `PawsApp::chat()`
4. **API → Orchestrator**: Stream spawned with `Orchestrator::run()`
5. **Orchestrator Loop**: Build context → Send to LLM → Process response → Repeat
6. **Response Stream**: Multiple `ChatResponse` events streamed back to UI
7. **State Persistence**: Conversation saved after each turn

## Error Handling States
- **RetryAttempt**: Automatic retry with exponential backoff
- **Interrupt**: Manual intervention required (max failures/requests)
- **ToolCallEnd**: Tool execution result (success or error)

## File References

- `crates/paws_main/src/ui.rs:89-102` - UI struct and initialization
- `crates/paws_main/src/ui.rs:292-404` - Main run loop and command handling
- `crates/paws_main/src/ui.rs:2327-2375` - Message handling and chat flow
- `crates/paws_main/src/ui.rs:2423-2501` - Chat response handling
- `crates/paws_main/src/state.rs:8-19` - UIState definition
- `crates/paws_domain/src/event.rs:44-61` - Event and EventValue types
- `crates/paws_domain/src/chat_request.rs:6-17` - ChatRequest structure
- `crates/paws_domain/src/chat_response.rs:47-56` - ChatResponse enum
- `crates/paws_app/src/app.rs:31-45` - PawsApp structure
- `crates/paws_app/src/app.rs:49-180` - Main chat orchestration
- `crates/paws_app/src/orch.rs:18-51` - Orchestrator structure
- `crates/paws_app/src/orch.rs:190-394` - Main execution loop
- `crates/paws_domain/src/conversation.rs:41-49` - Conversation structure
- `crates/paws_domain/src/context.rs:358-385` - Context structure
- `crates/paws_domain/src/context.rs:28-34` - ContextMessage enum

## State Data Structures

### UIState
```
- cwd: PathBuf
- conversation_id: Option<ConversationId>
```

### Conversation
```
- id: ConversationId
- title: Option<String>
- context: Option<Context>
- metrics: Metrics
- metadata: MetaData
```

### Context
```
- conversation_id: Option<ConversationId>
- messages: Vec<MessageEntry>
- tools: Vec<ToolDefinition>
- tool_choice: Option<ToolChoice>
- max_tokens: Option<usize>
- temperature: Option<Temperature>
- stream: Option<bool>
```

### Event Types
```
- EventValue::Text(UserPrompt)
- EventValue::Command(UserCommand)
```

### ChatResponse Types
```
- TaskMessage { content }
- TaskReasoning { content }
- TaskComplete
- ToolCallStart(ToolCallFull)
- ToolCallEnd(ToolResult)
- RetryAttempt { cause, duration }
- Interrupt { reason }
```
