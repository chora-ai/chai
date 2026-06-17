#!/bin/sh
# Build a grep pattern to find wikilinks to a given note name.
# Usage: build-backlink-pattern.sh <note_name>
#
# Input:  "Conventions"
# Output: "\[\[Conventions"
#
# Matches [[Conventions]] and [[Conventions|display text]].

note="$1"

if [ -z "$note" ]; then
  echo "error: note name is required" >&2
  exit 1
fi

# Escape BRE specials in the note name. Do `[` / `]` in separate passes so `.[` is not parsed as a
# collating element on strict POSIX sed, and the class does not rely on an ambiguous `[` inside `[]`.
escaped=$(printf '%s' "$note" | sed 's/]/\\]/g; s/\[/\\[/g; s/[.^$*+?(){}|]/\\&/g')

printf '\\[\\[%s' "$escaped"
