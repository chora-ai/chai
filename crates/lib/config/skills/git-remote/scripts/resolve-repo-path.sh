#!/bin/sh
# Resolve a repository path to an absolute sandbox path.
# If a path is provided, passes it through unchanged.
# If empty, defaults to the sandbox root directory so that
# git commands run against the sandbox root by default.
#
# Note: The sandbox validator and working_dir resolver handle
# symlink resolution and canonical path mapping. This script
# only needs to turn an empty param into the sandbox root.
#
# Args: $1 = path value (may be empty)
# Usage: resolve-repo-path.sh "$path"

path="$1"
sandbox="$HOME/.chai/active/sandbox"

# If path is provided, use as-is (sandbox validator resolves it)
if [ -n "$path" ]; then
    printf '%s' "$path"
    exit 0
fi

# Default to sandbox root
printf '%s' "$sandbox"
