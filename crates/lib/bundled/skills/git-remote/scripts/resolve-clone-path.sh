#!/bin/sh
# Resolve a clone path to an absolute sandbox path.
# If a path is provided, passes it through unchanged (relative paths
# are prefixed with the sandbox root; absolute paths are validated
# against the sandbox boundary).
# If empty, defaults to the sandbox root directory so the agent
# only needs to provide a relative directory name.
#
# Absolute paths are validated to ensure they are inside the sandbox.
# This prevents clone targets from escaping the sandbox boundary.
#
# The sandbox may contain symlinked directories whose physical
# paths are outside the sandbox root. These are granted access
# because the user placed them in the sandbox. The script checks
# both the physical sandbox root and symlink targets within it.
#
# Note: The resolveCommand architecture only passes $param (current param
# value), so this script cannot extract the repo name from the URL.
# The model must provide at least a directory name; this script ensures
# it resolves to the sandbox.
#
# Args: $1 = path value (may be empty)
# Usage: resolve-clone-path.sh "$path"

path="$1"
sandbox_raw="$HOME/.chai/active/sandbox"

# Resolve the sandbox to its physical (canonical) path.
# Absolute paths provided by the agent may be canonical, so the
# prefix comparison must use the physical sandbox path to match.
sandbox="$(cd "$sandbox_raw" 2>/dev/null && pwd -P)" || sandbox="$sandbox_raw"

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

if [ -n "$path" ]; then
    case "$path" in
        /*)
            # Absolute path — validate it is inside the sandbox.
            if is_inside_sandbox "$path"; then
                printf '%s' "$path"
            else
                echo "error: clone target $path is outside the sandbox" >&2; exit 1
            fi
            ;;
        *)  printf '%s/%s' "$sandbox_raw" "$path" ;;
    esac
    exit 0
fi

# Default to sandbox root (model should provide the directory name)
printf '%s' "$sandbox_raw"
