#!/bin/sh
# Post-process script: detect "not found" errors and append a hint.
# Receives the command's output on stdin. When the main command exited
# with a non-zero code (CHAI_EXIT_CODE != 0), appends a hint suggesting
# notes_list. Passes through output unchanged when the command succeeded
# (exit code 0).

input=$(cat)

if [ "${CHAI_EXIT_CODE:-0}" != "0" ]; then
    printf '%s' "$input"
    echo ""
    echo "hint: note not found — use notes_list to browse available notes"
else
    printf '%s' "$input"
fi
