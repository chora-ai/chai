#!/bin/sh
# Post-process hint for git_commit: detect clean tree or unstaged changes.
# Appends a diagnostic hint when the commit fails due to working tree state.
# Receives git commit output on stdin.

output=$(cat)

if echo "$output" | grep -q "nothing to commit"; then
    printf '%s\n' "$output"
    echo ""
    echo "hint: nothing to commit — working tree clean"
elif echo "$output" | grep -q "no changes added to commit\|untracked files present"; then
    printf '%s\n' "$output"
    echo ""
    echo "hint: unstaged changes present — use git_add to stage them"
else
    printf '%s\n' "$output"
fi
