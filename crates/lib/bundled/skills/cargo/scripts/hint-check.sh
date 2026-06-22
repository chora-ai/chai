#!/bin/sh
# Post-process hint for cargo_check: detect clean check, warnings, or errors.
# Receives cargo check output on stdin.
# Args: $1 = package (may be empty)

output=$(cat)

if [ -z "$output" ]; then
    if [ -n "$1" ]; then
        echo "cargo check -p $1 succeeded with no warnings"
    else
        echo "cargo check succeeded with no warnings"
    fi
elif echo "$output" | grep -q "^error"; then
    echo "$output"
    echo ""
    echo "hint: compilation errors detected — fix the errors above before proceeding"
elif echo "$output" | grep -q "warning"; then
    echo "$output"
    echo ""
    echo "hint: warnings detected — consider fixing them before proceeding"
else
    echo "$output"
fi
