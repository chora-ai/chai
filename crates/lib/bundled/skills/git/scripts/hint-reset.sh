#!/bin/sh
# Post-process hint for git_reset: detect the reset type and guide the agent.
# Appends a diagnostic hint to guide the agent's next step.
# Receives git reset output on stdin.
# Usage: hint-reset.sh <ref>

ref="$1"

output=$(cat)

echo "$output"

# After a reset, the changes are staged (mixed reset). Hint the agent to check
# status or re-commit as appropriate.
if [ -n "$output" ]; then
    echo ""
fi
echo "hint: reset to $ref — use git_status to inspect the current state, or git_commit to re-commit staged changes"
