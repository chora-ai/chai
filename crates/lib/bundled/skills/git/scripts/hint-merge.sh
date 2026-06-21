#!/bin/sh
# Post-process hint for git_merge: detect merge conflicts or successful squash merge.
# Appends a diagnostic hint to guide the agent's next step.
# Receives git merge output on stdin.
# Usage: hint-merge.sh <squash>

squash="$1"

output=$(cat)

if echo "$output" | grep -q "Merge conflict"; then
    echo "$output"
    echo ""
    echo "hint: merge conflicts detected — resolve conflicts, then git_add and git_commit"
elif [ "$squash" = "true" ]; then
    if echo "$output" | grep -q "Squash commit -- not updating HEAD"; then
        echo "$output"
        echo ""
        echo "hint: squash merge staged — use git_commit to finalize with a conventional commit message"
    else
        echo "$output"
    fi
else
    echo "$output"
fi
