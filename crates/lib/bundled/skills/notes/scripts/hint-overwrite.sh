#!/bin/sh
# Post-process script: detect when notes_write overwrites an existing note
# and append a hint.
# The chai file write command outputs "overwriting existing N lines"
# when the file existed before.

input=$(cat)

if printf '%s' "$input" | grep -q "overwriting existing" 2>/dev/null; then
    printf '%s' "$input"
    echo ""
    echo "hint: overwrote existing note"
else
    printf '%s' "$input"
fi
