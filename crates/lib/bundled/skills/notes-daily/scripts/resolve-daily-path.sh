#!/bin/sh
# Resolve a date to a daily note path.
# Usage: resolve-daily-path.sh <date> [scope]
#
# If the input is already an absolute path, returns it unchanged. This makes
# the script idempotent — when the generic executor substitutes a canonical
# path into args and build_argv re-resolves it through this script, the
# absolute path passes through without doubling the sandbox root prefix
# or appending a second .md extension.
#
# The daily notes folder is resolved in order:
#   1. <notes_dir>/.notes-daily.conf (format: folder=<relative-path>)
#   2. <notes_dir>/.obsidian/daily-notes.json (format: {"folder": "<relative-path>"})
#   3. Default: "daily"
#
# If no date is provided, uses today's date (YYYY-MM-DD).
#
# The scope parameter is a path relative to the sandbox root. When omitted,
# the notes directory defaults to the sandbox root.
#
# Output: <notes_dir>/<folder>/<date>.md

date="$1"
scope_rel="$2"

# Reject scope values containing path traversal.
case "$scope_rel" in
    *..*) echo "error: scope must not contain path traversal (..)" >&2; exit 1 ;;
esac

# If the input is already an absolute path, return it as-is.
case "$date" in
    /*) echo "$date"; exit 0 ;;
esac

sandbox_root="${CHAI_HOME:-$HOME/.chai}/active/sandbox"

# Resolve the notes directory from scope parameter.
if [ -z "$scope_rel" ]; then
    notes_dir="$sandbox_root"
else
    notes_dir="$sandbox_root/$scope_rel"
fi

# Default to today if no date provided.
if [ -z "$date" ]; then
    date=$(date +%Y-%m-%d)
fi

# Resolve the daily notes folder.
# 1. Check .notes-daily.conf in the notes directory.
folder=""
daily_config="$notes_dir/.notes-daily.conf"
if [ -f "$daily_config" ]; then
    val=$(sed -n 's/^folder=//p' "$daily_config" 2>/dev/null | head -1 | tr -d '\n')
    if [ -n "$val" ]; then
        folder="$val"
    fi
fi

# 2. Fallback: check .obsidian/daily-notes.json in the notes directory.
if [ -z "$folder" ]; then
    obsidian_config="$notes_dir/.obsidian/daily-notes.json"
    if [ -f "$obsidian_config" ]; then
        val=$(sed -n 's/.*"folder"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$obsidian_config" 2>/dev/null | head -1 | tr -d '\n')
        if [ -n "$val" ]; then
            folder="$val"
        fi
    fi
fi

# 3. Default folder.
if [ -z "$folder" ]; then
    folder="daily"
fi

echo "$notes_dir/$folder/$date.md"
