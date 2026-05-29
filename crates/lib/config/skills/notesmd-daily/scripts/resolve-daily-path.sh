#!/bin/sh
# Resolve a bare date (YYYY-MM-DD) to the vault daily-note path.
# Uses notesmd-cli print-default --path-only and .obsidian/daily-notes.json.
# Usage: resolve-daily-path.sh <date>
# Output: folder/date or date on stdout; on error echoes the input date.
#
# If the input is already an absolute path, returns it unchanged. This makes
# the script idempotent — when the generic executor substitutes a canonical
# path into args and build_argv re-resolves it, the absolute path passes
# through without doubling or re-processing.

date="${1:-}"

# If the input is already an absolute path, return it as-is.
case "$date" in
    /*) echo "$date"; exit 0 ;;
esac

if [ -z "$date" ]; then
  exit 1
fi

vault_path=$(notesmd-cli print-default --path-only 2>/dev/null) || { echo "$date"; exit 0; }
vault_path=$(echo "$vault_path" | tr -d '\n')
if [ -z "$vault_path" ]; then
  echo "$date"
  exit 0
fi

config_file="$vault_path/.obsidian/daily-notes.json"
if [ ! -f "$config_file" ]; then
  echo "$date"
  exit 0
fi

folder=$(sed -n 's/.*"folder"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$config_file" 2>/dev/null | head -1 | tr -d '\n')
if [ -z "$folder" ]; then
  echo "$date"
  exit 0
fi

echo "$folder/$date"
