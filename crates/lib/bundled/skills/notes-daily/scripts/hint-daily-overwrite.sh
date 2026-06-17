#!/bin/sh
# Post-process script: detect when notes_daily_append creates a new file
# (because the daily note didn't exist yet) and append a hint suggesting
# notes_daily_write instead. Also detect when notes_daily_write overwrites an
# existing daily note and append a hint suggesting notes_daily_append.
#
# For notes_daily_append: the chai file append command outputs
#   "appended N bytes to PATH (created new file)"
# when the file didn't exist.
#
# For notes_daily_write: the chai file write command outputs
#   "wrote PATH (N bytes, overwriting existing M lines)"
# when overwriting.
#
# This script handles both cases.

input=$(cat)

# notes_daily_append: new file created
if printf '%s' "$input" | grep -q "(created new file)" 2>/dev/null; then
    printf '%s' "$input"
    echo ""
    echo "hint: no daily note found for this date — use notes_daily_write to create one"
# notes_daily_write: overwriting existing
elif printf '%s' "$input" | grep -q "overwriting existing" 2>/dev/null; then
    printf '%s' "$input"
    echo ""
    echo "hint: daily note already exists — consider notes_daily_append to add content instead"
else
    printf '%s' "$input"
fi
