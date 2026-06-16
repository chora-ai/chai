#!/bin/sh
# Post-process script: detect "not found" errors and append a hint.
# Receives the command's output on stdin. When the output contains a
# "not found" error message, appends a hint suggesting kb_list.
# Passes through non-matching output unchanged.
#
# Handles two error patterns:
# - cat: "No such file or directory"
# - chai file read-lines: "file does not exist"

if grep -q "No such file or directory\|file does not exist" 2>/dev/null; then
    cat
    echo ""
    echo "hint: note not found — use kb_list to browse available notes"
else
    cat
fi
