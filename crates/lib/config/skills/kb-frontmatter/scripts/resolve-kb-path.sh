#!/bin/sh
# Resolve a KB-relative path to an absolute path in the active profile sandbox.
# Usage: resolve-kb-path.sh [relative-path]
# If no path or empty path, returns the KB root (sandbox root).

kb_root="$HOME/.chai/active/sandbox"
path="$1"

if [ -z "$path" ]; then
    echo "$kb_root"
else
    echo "$kb_root/$path"
fi
