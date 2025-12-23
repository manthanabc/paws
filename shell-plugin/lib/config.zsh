#!/usr/bin/env zsh

# Configuration variables for paws plugin
# Using typeset to keep variables local to plugin scope and prevent public exposure

typeset -h _PAWS_BIN="${PAWS_BIN:-paws}"
typeset -h _PAWS_CONVERSATION_PATTERN=":"
typeset -h _PAWS_MAX_COMMIT_DIFF="${PAWS_MAX_COMMIT_DIFF:-100000}"
typeset -h _PAWS_DELIMITER='\s\s+'
typeset -h _PAWS_PREVIEW_WINDOW="--preview-window=top:75%:wrap:border-sharp"

# Detect fd command - Ubuntu/Debian use 'fdfind', others use 'fd'
typeset -h _PAWS_FD_CMD="$(command -v fdfind 2>/dev/null || command -v fd 2>/dev/null || echo 'fd')"

# Detect bat command - use bat if available, otherwise fall back to cat
if command -v bat &>/dev/null; then
    typeset -h _PAWS_CAT_CMD="bat --color=always --style=numbers,changes --line-range=:500"
else
    typeset -h _PAWS_CAT_CMD="cat"
fi

# Commands cache - loaded lazily on first use
typeset -h _PAWS_COMMANDS=""

# Store active agent ID in a local variable (session-scoped)
# Default to "paws" agent
export _PAWS_ACTIVE_AGENT=paws

# Store conversation ID in a temporary variable (local to plugin)
export _PAWS_CONVERSATION_ID=""
