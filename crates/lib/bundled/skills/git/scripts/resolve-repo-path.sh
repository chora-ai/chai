#!/bin/sh
# Resolve a repository path for the sandbox validator.
# If a path is provided, passes it through unchanged.
# If empty, defaults to the sandbox root directory so that
# git commands run against the sandbox root by default.
#
# After resolving the working directory, verifies that git would
# find its repository root (.git directory) inside the sandbox.
# This prevents git's upward traversal from escaping the sandbox
# when the working directory does not contain its own .git.
#
# The sandbox may contain symlinked directories whose physical
# paths are outside the sandbox root. These are granted access
# because the user placed them in the sandbox. The script checks
# both the physical sandbox root and symlink targets within it.
#
# Note: The sandbox validator and working_dir resolver handle
# symlink resolution and canonical path mapping. This script
# only needs to turn an empty param into the sandbox root and
# perform the repository root validation.
#
# Args: $1 = path value (may be empty)
# Usage: resolve-repo-path.sh "$path"

path="$1"
sandbox_raw="$HOME/.chai/active/sandbox"

# Resolve the sandbox to its physical (canonical) path.
# git rev-parse --git-dir may return absolute paths, and when
# resolved via cd+pwd the result may be physical — so the prefix
# comparison must use the physical sandbox path to match reliably.
sandbox="$(cd "$sandbox_raw" 2>/dev/null && pwd -P)" || sandbox="$sandbox_raw"

# Resolve the working directory.
if [ -n "$path" ]; then
    work_dir="$sandbox_raw/$path"
else
    work_dir="$sandbox_raw"
fi

# Check whether a physical path is inside the sandbox.
# Matches against both the physical sandbox root and any
# symlinked entries at the top level of the sandbox directory.
# Symlinked entries are granted access because the user placed
# them in the sandbox.
is_inside_sandbox() {
    case "$1" in
        "$sandbox"/*) return 0 ;;
        "$sandbox") return 0 ;;
    esac
    # Check symlinked entries in the sandbox root.
    for entry in "$sandbox_raw"/*; do
        [ -L "$entry" ] || continue
        target=$(cd "$entry" 2>/dev/null && pwd -P) || continue
        case "$1" in
            "$target"/*) return 0 ;;
            "$target") return 0 ;;
        esac
    done
    return 1
}

# Verify that git would resolve its repository inside the sandbox.
# git rev-parse --git-dir returns the path to the .git directory
# relative to the working directory (or absolute if outside).
git_dir=$(cd "$work_dir" 2>/dev/null && git rev-parse --git-dir 2>/dev/null)
if [ -n "$git_dir" ]; then
    # Resolve git_dir to an absolute physical path for comparison.
    case "$git_dir" in
        /*) abs_git_dir="$git_dir" ;;
        *)  abs_git_dir="$(cd "$work_dir" && cd "$git_dir" && pwd -P)" ;;
    esac

    # Check that the .git directory is inside the sandbox.
    if ! is_inside_sandbox "$abs_git_dir"; then
        echo "error: git repository at $abs_git_dir is outside the sandbox" >&2; exit 1
    fi
fi

# Output the same value as before (relative path or sandbox root),
# so the downstream sandbox validator and working directory resolver
# work unchanged.
if [ -n "$path" ]; then
    printf '%s' "$path"
else
    printf '%s' "$sandbox_raw"
fi
