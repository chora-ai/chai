#!/bin/sh
# Build a SearXNG search URL from a query string.
# Usage: build-search-url.sh <query>
#
# Environment:
#   SEARXNG_URL - Base URL of the SearXNG instance (default: http://localhost:8888)

query="$1"
base_url="${SEARXNG_URL:-http://localhost:8888}"

if [ -z "$query" ]; then
  echo "error: query is required" >&2
  exit 1
fi

# URL-encode common characters
encoded=$(printf '%s' "$query" | sed 's/ /+/g; s/&/%26/g; s/=/%3D/g; s/?/%3F/g; s/#/%23/g')

echo "${base_url}/search?q=${encoded}&format=json&safesearch=1"
