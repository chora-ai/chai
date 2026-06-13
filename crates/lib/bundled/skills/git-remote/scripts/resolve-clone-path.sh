#!/bin/sh
# Resolve a clone path to an absolute sandbox path.
# If a path is provided, passes it through unchanged.
# If empty, defaults to the sandbox root directory so the agent
# only needs to provide a relative directory name.
#
# Note: The resolveCommand architecture only passes $param (current param
# value), so this script cannot extract the repo name from the URL.
# The model must provide at least a directory name; this script ensures
# it resolves to the sandbox.
#
# Args: $1 = path value (may be empty)
# Usage: resolve-clone-path.sh "$path"

path="$1"
sandbox="$HOME/.chai/active/sandbox"

# If path is provided and already absolute, use as-is
if [ -n "$path" ]; then
    case "$path" in
        /*) printf '%s' "$path" ;;
        *)  printf '%s/%s' "$sandbox" "$path" ;;
    esac
    exit 0
fi

# Default to sandbox root (model should provide the directory name)
printf '%s' "$sandbox"
