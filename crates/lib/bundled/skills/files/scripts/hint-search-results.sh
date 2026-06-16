#!/bin/sh
# Post-process script: append a hint to use files_read_lines when search
# returns results. Detects results by checking if stdout has content
# (grep exit 0 means matches found; exit 1 means no matches).
# The script receives output on stdin; if non-empty, appends hint.

input=$(cat)

if [ -n "$input" ]; then
    printf '%s' "$input"
    echo ""
    echo "hint: use files_read_lines with these line numbers for surrounding context"
fi
