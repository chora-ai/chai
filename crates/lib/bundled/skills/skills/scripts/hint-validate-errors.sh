#!/bin/sh
# Post-process hint for skills_validate: when errors are found, suggest
# using skills_read to examine the tools.json content.
# Receives the chai skill validate output on stdin.

input=$(cat)

# Check for validation errors
if echo "$input" | grep -q "^ERROR:"; then
    printf '%s\n' "$input"
    echo ""
    echo "hint: use skills_read with file: 'tools_json' to examine the content"
else
    printf '%s\n' "$input"
fi
