#!/bin/sh
# Resolve the feeds file path. If a non-empty argument is given, use it.
# Otherwise use the active profile's sandbox.
if [ -n "$1" ]; then
    echo "$1"
else
    echo "${CHAI_HOME:-$HOME/.chai}/active/sandbox/rss-feeds.txt"
fi
