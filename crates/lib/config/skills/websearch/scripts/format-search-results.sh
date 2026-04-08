#!/bin/sh
# Post-process script: transform SearXNG JSON into structured text.
# Receives raw JSON on stdin (from curl).
# Outputs top results as: TITLE | URL | SNIPPET
# Limits to 10 results.
#
# Requires: jq
# Usage: curl <searxng_url> | format-search-results.sh

if ! command -v jq >/dev/null 2>&1; then
    # Fallback: return raw input if jq is not available
    cat
    exit 0
fi

input=$(cat)

# Check if input is valid JSON with results
results=$(printf '%s' "$input" | jq -r '.results // empty' 2>/dev/null)
if [ -z "$results" ]; then
    echo "No search results found."
    exit 0
fi

printf '%s\n' "TITLE | URL | SNIPPET"
printf '%s\n' "------|-----|--------"

printf '%s' "$input" | jq -r '
    .results[:10][] |
    # Strip HTML tags from content
    (.content // "" | gsub("<[^>]*>"; "") | gsub("\\s+"; " ") | ltrimstr(" ") | rtrimstr(" ")) as $snippet |
    # Truncate snippet to 200 chars
    (if ($snippet | length) > 200 then ($snippet[:197] + "...") else $snippet end) as $truncated |
    "\(.title // "untitled") | \(.url // "") | \($truncated)"
'

count=$(printf '%s' "$input" | jq '.results | length' 2>/dev/null)
printf '\n(%s results total, showing up to 10)\n' "$count"
