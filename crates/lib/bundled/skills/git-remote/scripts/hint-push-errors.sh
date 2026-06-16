#!/bin/sh
# Post-process hint for git_push: detect common error conditions.
# Appends a diagnostic hint when the push fails due to:
# - non-fast-forward rejection
# - no upstream branch configured
# - no remote configured or remote not found
# Receives git push output on stdin.

output=$(cat)

if echo "$output" | grep -q "non-fast-forward\|rejected\|fetch first"; then
    echo "$output"
    echo ""
    echo "hint: pull first to integrate remote changes, then retry"
elif echo "$output" | grep -q "no upstream branch\|no tracking information\|has no upstream branch"; then
    echo "$output"
    echo ""
    echo "hint: no upstream set — use set_upstream: true on first push"
elif echo "$output" | grep -q "No remote\|remote:.*not found\|Could not resolve\|does not appear to be a git repository"; then
    echo "$output"
    echo ""
    echo "hint: no remote configured — use git_remote to list configured remotes"
else
    echo "$output"
fi
