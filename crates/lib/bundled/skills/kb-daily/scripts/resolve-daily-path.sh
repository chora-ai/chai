#!/bin/sh
# Resolve a date to a daily note path within the knowledge base.
# Usage: resolve-daily-path.sh <date> [kb_root]
#
# If the input is already an absolute path, returns it unchanged. This makes
# the script idempotent — when the generic executor substitutes a canonical
# path into args and build_argv re-resolves it through this script, the
# absolute path passes through without doubling the sandbox root prefix
# or appending a second .md extension.
#
# The daily notes folder is resolved in order:
#   1. <kb_dir>/.kb-daily.conf (format: folder=<relative-path>)
#   2. <kb_dir>/.obsidian/daily-notes.json (format: {"folder": "<relative-path>"})
#   3. Default: "daily"
#
# If no date is provided, uses today's date (YYYY-MM-DD).
#
# The kb_root parameter is a path relative to the sandbox root. When omitted,
# the KB directory defaults to the sandbox root.
#
# Output: <kb_dir>/<folder>/<date>.md

date="$1"
kb_root_rel="$2"

# If the input is already an absolute path, return it as-is.
case "$date" in
    /*) echo "$date"; exit 0 ;;
esac

sandbox_root="$HOME/.chai/active/sandbox"

# Resolve the KB directory from kb_root parameter.
if [ -z "$kb_root_rel" ]; then
    kb_dir="$sandbox_root"
else
    kb_dir="$sandbox_root/$kb_root_rel"
fi

# Default to today if no date provided.
if [ -z "$date" ]; then
    date=$(date +%Y-%m-%d)
fi

# Resolve the daily notes folder.
# 1. Check .kb-daily.conf in the KB directory.
folder=""
daily_config="$kb_dir/.kb-daily.conf"
if [ -f "$daily_config" ]; then
    val=$(sed -n 's/^folder=//p' "$daily_config" 2>/dev/null | head -1 | tr -d '\n')
    if [ -n "$val" ]; then
        folder="$val"
    fi
fi

# 2. Fallback: check .obsidian/daily-notes.json in the KB directory.
if [ -z "$folder" ]; then
    obsidian_config="$kb_dir/.obsidian/daily-notes.json"
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

echo "$kb_dir/$folder/$date.md"
