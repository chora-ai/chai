#!/bin/sh
# Post-process script: filter wikilink targets to only broken (nonexistent) ones.
# Receives wikilink targets on stdin (one per line, from grep -oP).
# Checks each target against the KB directory for existence.
# Outputs only targets that don't resolve to an existing file.
#
# Resolution strategy:
#   1. Exact path: <kb_dir>/<target>.md or <kb_dir>/<target>
#   2. Recursive search: find <target>.md anywhere under <kb_dir>
#      (handles bare wikilinks like [[AI Assistant]] in a nested KB)
#
# Usage: check-broken-links.sh [kb_root]
#
# The kb_root parameter may be:
# - Empty/omitted: KB directory defaults to the sandbox root
# - A relative path: resolved relative to the sandbox root
# - An absolute path: used as-is (canonical path from the executor)

kb_root="$1"

sandbox_root="$HOME/.chai/active/sandbox"

# If the input is already an absolute path, use it directly.
case "$kb_root" in
    /*) kb_dir="$kb_root" ;;
    *)
        if [ -z "$kb_root" ]; then
            kb_dir="$sandbox_root"
        else
            kb_dir="$sandbox_root/$kb_root"
        fi
        ;;
esac

sort -u | while IFS= read -r target; do
    [ -z "$target" ] && continue

    # Strip trailing backslash from escaped pipes (e.g. [[path\|alias]] -> path).
    target=$(printf '%s' "$target" | sed 's/\\$//')

    # Check 1: exact path at KB directory.
    if [ -f "$kb_dir/$target.md" ] || [ -f "$kb_dir/$target" ] || [ -d "$kb_dir/$target" ]; then
        continue
    fi

    # Check 2: recursive search for bare wikilinks.
    # Extract just the filename portion if the target contains a path.
    basename=$(printf '%s' "$target" | sed 's|.*/||')
    if [ -n "$basename" ]; then
        found=$(find "$kb_dir" -name "$basename.md" -type f 2>/dev/null | head -1)
        if [ -n "$found" ]; then
            continue
        fi
    fi

    echo "$target"
done
