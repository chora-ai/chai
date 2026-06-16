#!/bin/sh
# Post-process script: detect "not found" errors for daily note reads
# and append a hint suggesting kb_daily_write.
# Receives the command's output on stdin.

if grep -q "No such file or directory" 2>/dev/null; then
    cat
    echo ""
    echo "hint: no daily note found for this date — use kb_daily_write to create one"
else
    cat
fi
