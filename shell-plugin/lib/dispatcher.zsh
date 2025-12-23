#!/usr/bin/env zsh

# Main command dispatcher and widget registration

# Action handler: Set active agent or execute command
# Flow:
# 1. Check if user_action is a CUSTOM command -> execute with `cmd` subcommand
# 2. If no input_text -> switch to agent (for AGENT type commands)
# 3. If input_text -> execute command with active agent context
function _paws_action_default() {
    local user_action="$1"
    local input_text="$2"
    
    # Validate that the command exists in show-commands (if user_action is provided)
    if [[ -n "$user_action" ]]; then
        local commands_list=$(_paws_get_commands)
        if [[ -n "$commands_list" ]]; then
            # Check if the user_action is in the list of valid commands and extract the row
            local command_row=$(echo "$commands_list" | grep "^${user_action}\b")
            if [[ -z "$command_row" ]]; then
                echo
                _paws_log error "Command '\033[1m${user_action}\033[0m' not found"
                _paws_reset
                return 0
            fi
            
            # Extract the command type from the second field (TYPE column)
            # Format: "COMMAND_NAME    TYPE    DESCRIPTION"
            local command_type=$(echo "$command_row" | awk '{print $2}')
            # Case-insensitive comparison using :l (lowercase) modifier
            if [[ "${command_type:l}" == "custom" ]]; then
                # Generate conversation ID if needed
                [[ -z "$_PAWS_CONVERSATION_ID" ]] && _PAWS_CONVERSATION_ID=$($_PAWS_BIN conversation new)
                
                echo
                # Execute custom command with run subcommand
                if [[ -n "$input_text" ]]; then
                    _paws_exec cmd --cid "$_PAWS_CONVERSATION_ID" "$user_action" "$input_text"
                else
                    _paws_exec cmd --cid "$_PAWS_CONVERSATION_ID" "$user_action"
                fi
                _paws_reset
                return 0
            fi
        fi
    fi
    
    # If input_text is empty, just set the active agent (only if user explicitly specified one)
    if [[ -z "$input_text" ]]; then
        if [[ -n "$user_action" ]]; then
            echo
            # Set the agent in the local variable
            _PAWS_ACTIVE_AGENT="$user_action"
            _paws_log info "\033[1;37m${_PAWS_ACTIVE_AGENT:u}\033[0m \033[90mis now the active agent\033[0m"
        fi
        _paws_reset
        return 0
    fi
    
    # Generate conversation ID if needed (in parent shell context)
    if [[ -z "$_PAWS_CONVERSATION_ID" ]]; then
        _PAWS_CONVERSATION_ID=$($_PAWS_BIN conversation new)
    fi
    
    echo
    
    # Only set the agent if user explicitly specified one
    if [[ -n "$user_action" ]]; then
        _PAWS_ACTIVE_AGENT="$user_action"
    fi
    
    # Execute the paws command directly with proper escaping
    _paws_exec -p "$input_text" --cid "$_PAWS_CONVERSATION_ID"
    
    # Reset the prompt
    _paws_reset
}

function paws-accept-line() {
    # Save the original command for history
    local original_buffer="$BUFFER"
    
    # Parse the buffer first in parent shell context to avoid subshell issues
    local user_action=""
    local input_text=""
    
    # Check if the line starts with any of the supported patterns
    if [[ "$BUFFER" =~ "^:([a-zA-Z][a-zA-Z0-9_-]*)( (.*))?$" ]]; then
        # Action with or without parameters: :foo or :foo bar baz
        user_action="${match[1]}"
        # Only use match[3] if the second group (space + params) was actually matched
        if [[ -n "${match[2]}" ]]; then
            input_text="${match[3]}"
        else
            input_text=""
        fi
    elif [[ "$BUFFER" =~ "^: (.*)$" ]]; then
        # Default action with parameters: : something
        user_action=""
        input_text="${match[1]}"
    else
        # For non-:commands, use normal accept-line
        zle accept-line
        return
    fi
    
    # Add the original command to history before transformation
    print -s -- "$original_buffer"
    
    # CRITICAL: For multiline buffers, move cursor to end so output doesn't overwrite
    # Don't clear BUFFER yet - let _paws_reset do that after action completes
    # This keeps buffer state consistent if Ctrl+C is pressed
    if [[ "$BUFFER" == *$'\n'* ]]; then
        CURSOR=${#BUFFER}
        zle redisplay
    fi
    
    # Handle aliases - convert to their actual agent names
    case "$user_action" in
        ask)
            user_action="sage"
        ;;
        plan)
            user_action="muse"
        ;;
    esac
    
    # ⚠️  IMPORTANT: When adding a new command here, you MUST also update:
    #     crates/paws_main/src/built_in_commands.json
    #     Add a new entry: {"command": "name", "description": "Description [alias: x]"}
    #
    # Dispatch to appropriate action handler using pattern matching
    case "$user_action" in
        new|n)
            _paws_action_new
        ;;
        info|i)
            _paws_action_info
        ;;
        env|e)
            _paws_action_env
        ;;
        dump|d)
            _paws_action_dump "$input_text"
        ;;
        compact)
            _paws_action_compact
        ;;
        retry|r)
            _paws_action_retry
        ;;
        agent|a)
            _paws_action_agent "$input_text"
        ;;
        conversation|c)
            _paws_action_conversation "$input_text"
        ;;
        provider|p)
            _paws_action_provider
        ;;
        model|m)
            _paws_action_model
        ;;
        tools|t)
            _paws_action_tools
        ;;
        skill)
            _paws_action_skill
        ;;
        edit|ed)
            _paws_action_editor "$input_text"
        ;;
        commit)
            _paws_action_commit "$input_text"
        ;;
        suggest|s)
            _paws_action_suggest "$input_text"
        ;;
        clone)
            _paws_action_clone "$input_text"
        ;;
        login)
            _paws_action_login
        ;;
        logout)
            _paws_action_logout
        ;;
        *)
            _paws_action_default "$user_action" "$input_text"
        ;;
    esac
}
