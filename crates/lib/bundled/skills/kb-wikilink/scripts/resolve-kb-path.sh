#!/bin/sh
# Resolve a path to an absolute path in the sandbox root.
# Usage: resolve-kb-path.sh [relative-path]
#
# All kb paths are resolved from the sandbox root, matching the files skill.
# No KB root configuration file is needed.
#
# If no path or empty path, returns the sandbox root.
# If the path is already absolute, returns it unchanged. This makes the script
# idempotent — when the generic executor substitutes a canonical path into args
# and build_argv re-resolves it through this script, the absolute path passes
# through without doubling the sandbox root prefix.

path="$1"

# If the path is already absolute, return it as-is.
case "$path" in
    /*) echo "$path"; exit 0 ;;
esac

sandbox_root="$HOME/.chai/active/sandbox"

if [ -z "$path" ]; then
    echo "$sandbox_root"
else
    echo "$sandbox_root/$path"
fi
