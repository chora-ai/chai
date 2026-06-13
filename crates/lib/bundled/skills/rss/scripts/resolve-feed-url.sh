#!/bin/sh
# Resolve a feed name to its URL. If the argument looks like a URL, pass it
# through. Otherwise look it up in the feeds file.
feed="$1"

# Pass through URLs directly.
case "$feed" in
    http://*|https://*) echo "$feed"; exit 0 ;;
esac

# Look up the feed name in the feeds file.
feeds_file="$HOME/.chai/active/sandbox/rss-feeds.txt"
if [ ! -f "$feeds_file" ]; then
    echo "$feed"
    exit 0
fi

url=$(grep "^${feed}|" "$feeds_file" | head -1 | cut -d'|' -f2)
if [ -n "$url" ]; then
    echo "$url"
else
    echo "$feed"
fi
