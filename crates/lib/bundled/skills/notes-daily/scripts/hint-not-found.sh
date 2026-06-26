#!/bin/sh
# Post-process script: detect "not found" errors for daily note reads
# and append a hint suggesting notes_daily_write.
# Receives the command's output on stdin. When the main command exited
# with a non-zero code (CHAI_EXIT_CODE != 0), appends a hint suggesting
# notes_daily_write. Passes through output unchanged when the command
# succeeded (exit code 0).

input=$(cat)

if [ "${CHAI_EXIT_CODE:-0}" != "0" ]; then
    printf '%s\n' "$input"
    echo ""
    echo "hint: no daily note found for this date — use notes_daily_write to create one"
else
    printf '%s\n' "$input"
fi
