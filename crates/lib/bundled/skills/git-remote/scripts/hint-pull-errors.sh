#!/bin/sh
# Post-process hint for git_pull: detect common error conditions.
# Appends a diagnostic hint when the pull fails due to:
# - no tracking information for the current branch
# - remote not found
# Receives git pull output on stdin.

output=$(cat)

if echo "$output" | grep -q "no tracking information\|no upstream branch\|There is no tracking information"; then
    printf '%s\n' "$output"
    echo ""
    echo "hint: no tracking branch set — specify remote and branch explicitly"
elif echo "$output" | grep -q "Could not resolve\|does not appear to be a git repository\|not found"; then
    printf '%s\n' "$output"
    echo ""
    echo "hint: remote not found — use git_remote to list configured remotes"
else
    printf '%s\n' "$output"
fi
