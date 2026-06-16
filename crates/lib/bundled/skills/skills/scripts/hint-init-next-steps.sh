#!/bin/sh
# Post-process hint for skills_init: suggest next steps after initialization.
# Receives the chai skill init output on stdin.

input=$(cat)

# Only add hint on successful init (output contains "initialized skill")
if echo "$input" | grep -q "initialized skill"; then
    printf '%s\n' "$input"
    echo ""
    echo "hint: skill initialized — next: design tools, write tools.json, then validate"
else
    printf '%s' "$input"
fi
