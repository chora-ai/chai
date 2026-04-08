#!/bin/sh
# Post-process script: extract readable text from HTML.
# Receives raw HTML on stdin (from curl).
# Strips tags, decodes common entities, collapses whitespace.
# Extracts <title> and <body> content when present.
#
# Usage: curl <url> | strip-html.sh

input=$(cat)

# Extract title if present
title=$(printf '%s' "$input" | tr '\n' ' ' | sed -n 's/.*<title[^>]*>\([^<]*\)<\/title>.*/\1/Ip' | head -1)

# Extract body content (or use full input if no body tag)
body=$(printf '%s' "$input" | tr '\n' ' ' | sed -n 's/.*<body[^>]*>\(.*\)<\/body>.*/\1/Ip' | head -1)
if [ -z "$body" ]; then
    body="$input"
fi

# Remove script and style blocks
body=$(printf '%s' "$body" | sed 's/<script[^>]*>.*<\/script>//gI')
body=$(printf '%s' "$body" | sed 's/<style[^>]*>.*<\/style>//gI')

# Convert block elements to newlines
body=$(printf '%s' "$body" | sed 's/<\/\(p\|div\|h[1-6]\|li\|tr\|br\)[^>]*>/\n/gI')
body=$(printf '%s' "$body" | sed 's/<\(p\|div\|h[1-6]\|li\|tr\)[^>]*>/\n/gI')

# Strip remaining HTML tags
body=$(printf '%s' "$body" | sed 's/<[^>]*>//g')

# Decode common HTML entities
body=$(printf '%s' "$body" | sed 's/&amp;/\&/g; s/&lt;/</g; s/&gt;/>/g; s/&quot;/"/g; s/&#39;/'"'"'/g; s/&nbsp;/ /g')

# Collapse whitespace (preserve paragraph breaks)
body=$(printf '%s' "$body" | sed 's/[[:blank:]]\+/ /g' | sed 's/^ //;s/ $//' | sed '/^$/d' | head -200)

# Output
if [ -n "$title" ]; then
    printf '=== %s ===\n\n' "$title"
fi

printf '%s\n' "$body"
