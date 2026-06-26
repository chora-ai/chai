#!/bin/sh
# Post-process hint for git_status: detect non-repo paths.
# If the output contains "not a git repository", append a hint.
# Receives git status output on stdin.

output=$(cat)

if echo "$output" | grep -q "not a git repository"; then
    printf '%s\n' "$output"
    echo ""
    echo "hint: not a git repository — specify a valid repo path"
else
    printf '%s\n' "$output"
fi
