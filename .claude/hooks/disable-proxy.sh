#!/bin/bash

# Claude Code Hook: Disable proxy for curl commands
# This hook automatically sets environment variables to bypass proxy for local connections

# Check if the command contains curl
if echo "$CLAUDE_TOOL_PARAMS" | grep -q "curl"; then
    export NO_PROXY="*"
    export no_proxy="*"
    unset http_proxy
    unset https_proxy
    unset HTTP_PROXY
    unset HTTPS_PROXY
    unset ALL_PROXY
    unset all_proxy
    echo "🔧 Proxy disabled for curl command"
fi

exit 0