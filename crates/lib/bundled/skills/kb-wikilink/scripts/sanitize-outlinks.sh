#!/bin/sh
# Post-process script: sanitize outlink output and detect broken links.
# 1. Strip trailing backslashes from escaped pipes in wikilinks inside
#    table cells (e.g., [[conventions/general\|General]] -> conventions/general).
# 2. Count links that don't resolve to existing files and append a hint.
#
# Receives wikilink targets on stdin (one per line, from grep -oP).
# Usage: sanitize-outlinks.sh [kb_root]
#
# The kb_root parameter may be:
# - Empty/omitted: KB directory defaults to the sandbox root
# - A relative path: resolved relative to the sandbox root
# - An absolute path: used as-is (canonical path from the executor)

kb_root="$1"

sandbox_root="$HOME/.chai/active/sandbox"

# Resolve kb_root to an absolute path.
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

# First pass: sanitize and collect output.
sanitized=""
broken=0
total=0

while IFS= read -r line; do
    [ -z "$line" ] && continue

    # Strip trailing backslash from escaped pipes.
    target=$(printf '%s' "$line" | sed 's/\\$//')

    if [ -n "$sanitized" ]; then
        sanitized="${sanitized}
${target}"
    else
        sanitized="$target"
    fi

    total=$((total + 1))

    # Check if the link resolves.
    if [ -f "$kb_dir/$target.md" ] || [ -f "$kb_dir/$target" ] || [ -d "$kb_dir/$target" ]; then
        continue
    fi

    # Recursive search for bare wikilinks.
    basename=$(printf '%s' "$target" | sed 's|.*/||')
    if [ -n "$basename" ]; then
        found=$(find "$kb_dir" -name "$basename.md" -type f 2>/dev/null | head -1)
        if [ -n "$found" ]; then
            continue
        fi
    fi

    broken=$((broken + 1))
done

# Output the sanitized links.
if [ -n "$sanitized" ]; then
    printf '%s' "$sanitized"
fi

# Append broken-link hint if any were found.
if [ "$broken" -gt 0 ]; then
    echo ""
    echo "hint: ${broken} broken link(s) — use kb_wikilink_broken for details"
fi
