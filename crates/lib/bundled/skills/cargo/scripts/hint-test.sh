#!/bin/sh
# Post-process hint for cargo_test: detect test failures, warnings, and
# summarize results. Filters out progress lines and individual passing test
# lines so the agent only sees diagnostics, failures, and summaries.
# Receives cargo test output on stdin.
# Args: $1 = package (may be empty)

output=$(cat)

if [ -z "$output" ]; then
    if [ -n "$1" ]; then
        echo "cargo test -p $1 succeeded with no warnings"
    else
        echo "cargo test succeeded with no warnings"
    fi
    exit 0
fi

# Filter: remove progress lines, individual passing test lines, and
# "running N tests" lines, then collapse consecutive blank lines.
# Everything else is diagnostic content, failure details, or summaries.
filtered=$(printf '%s\n' "$output" | grep -v '^[[:space:]]*\(Checking\|Compiling\|Finished\|Running\)[[:space:]]' | grep -v '^test .* \.\.\. ok$' | grep -v '^running [0-9][0-9]* tests\{0,1\}$' | awk 'NF || !blank++ { print; if(NF) blank=0 }')

if [ -z "$filtered" ]; then
    # No diagnostic content — only progress/passing lines were present
    if [ -n "$1" ]; then
        echo "cargo test -p $1 succeeded with no warnings"
    else
        echo "cargo test succeeded with no warnings"
    fi
elif printf '%s\n' "$filtered" | grep -q '^error'; then
    # Compilation error (appears before any test results)
    printf '%s\n' "$filtered"
    echo ""
    echo "hint: compilation error prevented tests from running — fix the errors above"
elif printf '%s\n' "$filtered" | grep -q '^test result:.*FAILED'; then
    # Test failures
    printf '%s\n' "$filtered"
    echo ""
    echo "hint: some tests failed — review the failures above"
elif printf '%s\n' "$filtered" | grep -q 'warning'; then
    # Warnings but no failures
    printf '%s\n' "$filtered"
    echo ""
    echo "hint: warnings detected — consider fixing them before proceeding"
else
    # Clean: no errors, no warnings, no failures — show summary only
    prefix="cargo test"
    if [ -n "$1" ]; then
        prefix="cargo test -p $1"
    fi
    printf '%s\n' "$filtered" | grep "^test result:" | while IFS= read -r line; do
        echo "$prefix: $line"
    done
fi
