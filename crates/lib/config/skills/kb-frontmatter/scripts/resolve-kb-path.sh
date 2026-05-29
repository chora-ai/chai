#!/bin/sh
# Resolve a KB-relative path to an absolute path in the active profile sandbox.
# Usage: resolve-kb-path.sh [relative-path]
# If no path or empty path, returns the KB root (sandbox root).
# If the path is already absolute, returns it unchanged. This makes the script
# idempotent — when the generic executor substitutes a canonical path into args
# and build_argv re-resolves it through this script, the absolute path passes
# through without doubling the sandbox root prefix.

path="$1"

# If the path is already absolute, return it as-is.
case "$path" in
    /*) echo "$path"; exit 0 ;;
esac

kb_root="$HOME/.chai/active/sandbox"

if [ -z "$path" ]; then
    echo "$kb_root"
else
    echo "$kb_root/$path"
fi
