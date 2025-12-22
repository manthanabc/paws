#!/usr/bin/env zsh

# Authentication action handlers

# Action handler: Login to provider
function _paws_action_login() {
    echo
    local selected
    selected=$(_paws_select_provider)
    if [[ -n "$selected" ]]; then
        # Extract the second field (provider ID)
        local provider=$(echo "$selected" | awk '{print $2}')
        _paws_exec provider login "$provider"
    fi
    _paws_reset
}

# Action handler: Logout from provider
function _paws_action_logout() {
    echo
    local selected
    selected=$(_paws_select_provider "\[yes\]")
    if [[ -n "$selected" ]]; then
        # Extract the second field (provider ID)
        local provider=$(echo "$selected" | awk '{print $2}')
        _paws_exec provider logout "$provider"
    fi
    _paws_reset
}
