#!/bin/sh
# Resolve the current git branch name for deny pattern checking.
# Receives the working directory as an argument.
# Usage: resolve-current-branch.sh <working-dir>

working_dir="$1"

if [ -z "$working_dir" ]; then
    echo "error: no working directory provided" >&2
    exit 1
fi

cd "$working_dir" 2>/dev/null || {
    echo "error: cannot access $working_dir" >&2
    exit 1
}

branch=$(git branch --show-current 2>/dev/null)

if [ -z "$branch" ]; then
    echo "error: not on a branch (detached HEAD)" >&2
    exit 1
fi

echo "$branch"
