#!/bin/sh
# Post-process hint for logs_search: detect many matches.
# Appends a diagnostic hint when a large number of matches are returned,
# suggesting the agent narrow the pattern.
# Receives logs search output on stdin.

output=$(cat)

# Count matching lines (lines starting with > or having context markers)
# The search output ends with "N match(es) for pattern"
match_count=$(echo "$output" | grep -o '[0-9]\+ match(es)' | grep -o '[0-9]\+')

if [ -n "$match_count" ] && [ "$match_count" -gt 15 ]; then
    echo "$output"
    echo ""
    echo "hint: many matches — consider narrowing the pattern"
else
    echo "$output"
fi
