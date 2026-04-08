#!/bin/sh
# Resolve a date to a daily note path within the knowledge base.
# Usage: resolve-daily-path.sh <date>
#
# Reads the daily notes folder from the convention file:
#   $HOME/.chai/active/sandbox/.kb-daily.conf
#
# Convention file format (simple key=value):
#   folder=00-daily
#
# If no convention file exists, defaults to "00-daily".
# If no date is provided, uses today's date (YYYY-MM-DD).
#
# Output: <kb_root>/<folder>/<date>.md

kb_root="$HOME/.chai/active/sandbox"
date="$1"

# Default to today if no date provided.
if [ -z "$date" ]; then
    date=$(date +%Y-%m-%d)
fi

# Read folder from convention file, default to 00-daily.
folder="00-daily"
config="$kb_root/.kb-daily.conf"
if [ -f "$config" ]; then
    val=$(sed -n 's/^folder=//p' "$config" 2>/dev/null | head -1 | tr -d '\n')
    if [ -n "$val" ]; then
        folder="$val"
    fi
fi

echo "$kb_root/$folder/$date.md"
