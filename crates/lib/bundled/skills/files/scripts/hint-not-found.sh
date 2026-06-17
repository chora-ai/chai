#!/bin/sh
# Post-process script: detect "not found" errors and append a hint.
# Receives the command's output on stdin. When the main command exited
# with a non-zero code (CHAI_EXIT_CODE != 0), appends a hint suggesting
# files_list_dir. Passes through output unchanged when the command
# succeeded (exit code 0).

input=$(cat)

if [ "${CHAI_EXIT_CODE:-0}" != "0" ]; then
    printf '%s' "$input"
    echo ""
    echo "hint: file not found — use files_list_dir to browse available files"
else
    printf '%s' "$input"
fi
