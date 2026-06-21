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

# Try the normal path first (works during cherry-pick and normal operations).
branch=$(git branch --show-current 2>/dev/null)

# During rebase, git enters detached HEAD. Fall back to the rebase head-name file.
if [ -z "$branch" ]; then
    head_name_file="$(git rev-parse --git-dir 2>/dev/null)/rebase-merge/head-name"
    if [ -f "$head_name_file" ]; then
        ref=$(cat "$head_name_file")
        # Strip the refs/heads/ prefix to get the branch name.
        branch="${ref#refs/heads/}"
    fi
fi

if [ -z "$branch" ]; then
    echo "error: not on a branch (detached HEAD)" >&2
    exit 1
fi

echo "$branch"
