#!/usr/bin/env zsh

# Core action handlers for basic paws operations

# Action handler: Start a new conversation
function _paws_action_new() {
    _PAWS_CONVERSATION_ID=""
    _PAWS_ACTIVE_AGENT="paws"
    
    echo
    _paws_exec banner
    _paws_reset
}

# Action handler: Show session info
function _paws_action_info() {
    echo
    if [[ -n "$_PAWS_CONVERSATION_ID" ]]; then
        _paws_exec info --cid "$_PAWS_CONVERSATION_ID"
    else
        _paws_exec info
    fi
    _paws_reset
}

# Action handler: Show environment info
function _paws_action_env() {
    echo
    _paws_exec env
    _paws_reset
}

# Action handler: Dump conversation
function _paws_action_dump() {
    local input_text="$1"
    if [[ "$input_text" == "html" ]]; then
        _paws_handle_conversation_command "dump" "--html"
    else
        _paws_handle_conversation_command "dump"
    fi
}

# Action handler: Compact conversation
function _paws_action_compact() {
    _paws_handle_conversation_command "compact"
}

# Action handler: Retry last message
function _paws_action_retry() {
    _paws_handle_conversation_command "retry"
}

# Helper function to handle conversation commands that require an active conversation
function _paws_handle_conversation_command() {
    local subcommand="$1"
    shift  # Remove first argument, remaining args become extra parameters
    
    echo
    
    # Check if PAWS_CONVERSATION_ID is set
    if [[ -z "$_PAWS_CONVERSATION_ID" ]]; then
        _paws_log error "No active conversation. Start a conversation first or use :list to see existing ones"
        _paws_reset
        return 0
    fi
    
    # Execute the conversation command with conversation ID and any extra arguments
    _paws_exec conversation "$subcommand" "$_PAWS_CONVERSATION_ID" "$@"
    
    _paws_reset
    return 0
}
