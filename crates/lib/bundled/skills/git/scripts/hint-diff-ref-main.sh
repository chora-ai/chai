#!/bin/sh
# Post-process hint for git_diff: clarify ref="main" behavior.
# When ref=main is used, append a note explaining what the output shows.
# Receives the ref value as $1 and git diff output on stdin.
#
# Args: $1 = ref parameter value

output=$(cat)
ref="$1"

if [ "$ref" = "main" ]; then
    echo "$output"
    echo ""
    echo "hint: showing changes since diverging from main"
else
    echo "$output"
fi
