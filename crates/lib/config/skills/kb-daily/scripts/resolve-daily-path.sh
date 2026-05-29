#!/bin/sh
# Resolve a date to a daily note path within the knowledge base.
# Usage: resolve-daily-path.sh <date>
#
# If the input is already an absolute path, returns it unchanged. This makes
# the script idempotent — when the generic executor substitutes a canonical
# path into args and build_argv re-resolves it through this script, the
# absolute path passes through without doubling the sandbox root prefix
# or appending a second .md extension.
#
# Otherwise, reads the daily notes folder from the convention file:
#   $HOME/.chai/active/sandbox/.kb-daily.conf
#
# Convention file format (simple key=value):
#   folder=00-daily
#
# If no convention file exists, defaults to "00-daily".
# If no date is provided, uses today's date (YYYY-MM-DD).
#
# Output: <kb_root>/<folder>/<date>.md

date="$1"

# If the input is already an absolute path, return it as-is.
case "$date" in
    /*) echo "$date"; exit 0 ;;
esac

kb_root="$HOME/.chai/active/sandbox"

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
