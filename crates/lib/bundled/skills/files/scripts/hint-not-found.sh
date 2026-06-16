#!/bin/sh
# Post-process script: detect "not found" errors and append a hint.
# Receives the command's output on stdin. When the output contains a
# "not found" error message, appends a hint suggesting files_list_dir.
# Passes through non-matching output unchanged.
#
# Handles two error patterns:
# - cat: "No such file or directory"
# - chai file read-lines: "file does not exist"

if grep -q "No such file or directory\|file does not exist" 2>/dev/null; then
    cat
    echo ""
    echo "hint: file not found — use files_list_dir to browse available files"
else
    cat
fi
