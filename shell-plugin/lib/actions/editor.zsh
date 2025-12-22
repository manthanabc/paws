#!/usr/bin/env zsh

# Editor and command suggestion action handlers

# Action handler: Open external editor for command composition
function _paws_action_editor() {
    local initial_text="$1"
    echo
    
    # Determine editor in order of preference: PAWS_EDITOR > EDITOR > nano
    local editor_cmd="${PAWS_EDITOR:-${EDITOR:-nano}}"
    
    # Validate editor exists
    if ! command -v "${editor_cmd%% *}" &>/dev/null; then
        _paws_log error "Editor not found: $editor_cmd (set PAWS_EDITOR or EDITOR)"
        _paws_reset
        return 1
    fi
    
    # Create .paws directory if it doesn't exist
    local paws_dir=".paws"
    if [[ ! -d "$paws_dir" ]]; then
        mkdir -p "$paws_dir" || {
            _paws_log error "Failed to create .paws directory"
            _paws_reset
            return 1
        }
    fi
    
    # Create temporary file with git-like naming: PAWS_EDITMSG.md
    local temp_file="${paws_dir}/PAWS_EDITMSG.md"
    touch "$temp_file" || {
        _paws_log error "Failed to create temporary file"
        _paws_reset
        return 1
    }
    
    # Ensure cleanup on exit
    trap "rm -f '$temp_file'" EXIT INT TERM
    
    # Pre-populate with initial text if provided
    if [[ -n "$initial_text" ]]; then
        echo "$initial_text" > "$temp_file"
    fi
    
    # Open editor
    eval "$editor_cmd '$temp_file'"
    local editor_exit_code=$?
    
    if [ $editor_exit_code -ne 0 ]; then
        _paws_log error "Editor exited with error code $editor_exit_code"
        _paws_reset
        return 1
    fi
    
    # Read and process content
    local content
    content=$(cat "$temp_file" | tr -d '\r')
    
    if [ -z "$content" ]; then
        _paws_log info "Editor closed with no content"
        _paws_reset
        return 0
    fi
    
    # Insert into buffer with : prefix
    BUFFER=": $content"
    CURSOR=${#BUFFER}
    
    _paws_log info "Command ready - press Enter to execute"
    zle reset-prompt
}

# Action handler: Generate shell command from natural language
# Usage: :? <description>
function _paws_action_suggest() {
    local description="$1"
    
    if [[ -z "$description" ]]; then
        _paws_log error "Please provide a command description"
        _paws_reset
        return 0
    fi
    
    echo
    # Generate the command
    local generated_command
    generated_command=$(FORCE_COLOR=true CLICOLOR_FORCE=1 _paws_exec suggest "$description")
    
    if [[ -n "$generated_command" ]]; then
        # Replace the buffer with the generated command
        BUFFER="$generated_command"
        CURSOR=${#BUFFER}
        zle reset-prompt
    else
        _paws_log error "Failed to generate command"
        _paws_reset
    fi
}
