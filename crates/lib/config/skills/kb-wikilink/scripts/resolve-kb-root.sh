#!/bin/sh
# Output the knowledge base root path.
# Used to pass the KB root as --root to chai file rename-note.
#
# Usage: resolve-kb-root.sh [kb_root]
#
# Accepts an optional kb_root parameter (a path relative to the sandbox root).
# When provided, resolves to $sandbox_root/$kb_root.
# When omitted, defaults to the sandbox root.
#
# If the input is already an absolute path, returns it unchanged. This makes
# the script idempotent — when the generic executor substitutes a canonical
# path into args and build_argv re-resolves it through this script, the
# absolute path passes through without doubling the sandbox root prefix.

kb_root_rel="$1"

# If the input is already absolute, return it as-is.
case "$kb_root_rel" in
    /*) echo "$kb_root_rel"; exit 0 ;;
esac

sandbox_root="$HOME/.chai/active/sandbox"

if [ -z "$kb_root_rel" ]; then
    echo "$sandbox_root"
else
    echo "$sandbox_root/$kb_root_rel"
fi
