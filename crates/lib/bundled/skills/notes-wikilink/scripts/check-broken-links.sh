#!/bin/sh
# Post-process script: filter wikilink targets to only broken (nonexistent) ones.
# Receives wikilink targets on stdin (one per line, from grep -oP).
# Checks each target against the notes directory for existence.
# Outputs only targets that don't resolve to an existing file.
#
# Resolution strategy:
#   1. Exact path: <notes_dir>/<target>.md or <notes_dir>/<target>
#   2. Recursive search: find <target>.md anywhere under <notes_dir>
#      (handles bare wikilinks like [[AI Assistant]] in a nested notes directory)
#
# Usage: check-broken-links.sh [scope]
#
# The scope parameter may be:
# - Empty/omitted: notes directory defaults to the sandbox root
# - A relative path: resolved relative to the sandbox root
# - An absolute path: used as-is (canonical path from the executor)

scope="$1"

sandbox_root="$HOME/.chai/active/sandbox"

# If the input is already an absolute path, use it directly.
case "$scope" in
    /*) notes_dir="$scope" ;;
    *)
        if [ -z "$scope" ]; then
            notes_dir="$sandbox_root"
        else
            notes_dir="$sandbox_root/$scope"
        fi
        ;;
esac

sort -u | while IFS= read -r target; do
    [ -z "$target" ] && continue

    # Strip trailing backslash from escaped pipes (e.g. [[path\|alias]] -> path).
    target=$(printf '%s' "$target" | sed 's/\\$//')

    # Check 1: exact path at notes directory.
    if [ -f "$notes_dir/$target.md" ] || [ -f "$notes_dir/$target" ] || [ -d "$notes_dir/$target" ]; then
        continue
    fi

    # Check 2: recursive search for bare wikilinks.
    # Extract just the filename portion if the target contains a path.
    basename=$(printf '%s' "$target" | sed 's|.*/||')
    if [ -n "$basename" ]; then
        found=$(find "$notes_dir" -name "$basename.md" -type f 2>/dev/null | head -1)
        if [ -n "$found" ]; then
            continue
        fi
    fi

    echo "$target"
done