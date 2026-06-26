#!/bin/sh
# Post-process hint for cargo_check: detect clean check, warnings, or errors.
# Filters out progress lines (Checking, Compiling, Finished, Running) so the
# agent only sees diagnostics (errors, warnings) and summaries.
# Receives cargo check output on stdin.
# Args: $1 = package (may be empty)

output=$(cat)

if [ -z "$output" ]; then
    if [ -n "$1" ]; then
        echo "cargo check -p $1 succeeded with no warnings"
    else
        echo "cargo check succeeded with no warnings"
    fi
    exit 0
fi

# Filter: remove progress lines, then collapse consecutive blank lines.
# Progress lines are the only noise — everything else is diagnostic content
# (warnings, errors, file references, code snippets, crate-level summaries).
filtered=$(printf '%s\n' "$output" | grep -v '^[[:space:]]*\(Checking\|Compiling\|Finished\|Running\)[[:space:]]' | awk 'NF || !blank++ { print; if(NF) blank=0 }')

if [ -z "$filtered" ]; then
    # No diagnostic content — only progress lines were present
    if [ -n "$1" ]; then
        echo "cargo check -p $1 succeeded with no warnings"
    else
        echo "cargo check succeeded with no warnings"
    fi
elif printf '%s\n' "$filtered" | grep -q '^error'; then
    printf '%s\n' "$filtered"
    echo ""
    echo "hint: compilation errors detected — fix the errors above before proceeding"
elif printf '%s\n' "$filtered" | grep -q 'warning'; then
    printf '%s\n' "$filtered"
    echo ""
    echo "hint: warnings detected — consider fixing them before proceeding"
else
    printf '%s\n' "$filtered"
fi
