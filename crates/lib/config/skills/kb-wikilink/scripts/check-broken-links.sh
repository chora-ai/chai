#!/bin/sh
# Post-process script: filter wikilink targets to only broken (nonexistent) ones.
# Receives wikilink targets on stdin (one per line, from grep -oP).
# Checks each target against the KB root for existence.
# Outputs only targets that don't resolve to an existing file.
#
# A target resolves if any of these exist:
#   <kb_root>/<target>.md
#   <kb_root>/<target>
#
# Usage: pipe grep output | check-broken-links.sh

kb_root="$HOME/.chai/active/sandbox"
found_broken=0

sort -u | while IFS= read -r target; do
    [ -z "$target" ] && continue
    if [ ! -f "$kb_root/$target.md" ] && [ ! -f "$kb_root/$target" ] && [ ! -d "$kb_root/$target" ]; then
        echo "$target"
        found_broken=1
    fi
done
