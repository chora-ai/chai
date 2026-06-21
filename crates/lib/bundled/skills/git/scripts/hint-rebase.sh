#!/bin/sh
# Post-process hint for git_rebase: detect conflicts or already-up-to-date state.
# Appends a diagnostic hint to guide the agent's next step.
# Receives git rebase output on stdin.

output=$(cat)

if echo "$output" | grep -q "CONFLICT"; then
    echo "$output"
    echo ""
    echo "hint: rebase conflicts detected — resolve conflicts, stage them with git_add, then use git_rebase_continue to proceed (or git_rebase_abort to cancel)"
elif echo "$output" | grep -q "is up to date"; then
    echo "$output"
    echo ""
    echo "hint: current branch is already up to date with the target"
else
    echo "$output"
fi
