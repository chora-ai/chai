#!/bin/sh
# Normalize a tag for grep search.
# Usage: normalize-tag.sh <tag>
#
# Strips leading # if present, escapes BRE special characters,
# and builds a pattern that matches the tag in frontmatter or body.
#
# Input:  "#agentic-systems"  or  "agentic-systems"
# Output: "agentic-systems"  (escaped for grep BRE)

tag="$1"

if [ -z "$tag" ]; then
    echo "error: tag is required" >&2
    exit 1
fi

# Strip leading # if present.
tag=$(printf '%s' "$tag" | sed 's/^#//')

# Escape BRE special characters.
printf '%s' "$tag" | sed 's/[].^$*+?(){}|[]/\\&/g'
