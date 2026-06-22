#!/bin/sh
# Post-process hint for cargo_test: detect test failures and summarize results.
# Receives cargo test output on stdin.
# Args: $1 = package (may be empty)

output=$(cat)

if echo "$output" | grep -q "^test result:.*FAILED"; then
    echo "$output"
    echo ""
    echo "hint: some tests failed — review the failures above"
elif echo "$output" | grep -q "^test result:.*ok"; then
    # Extract summary lines for clean output — prefix each line individually
    # so that multi-package workspace output is consistently formatted.
    prefix="cargo test"
    if [ -n "$1" ]; then
        prefix="cargo test -p $1"
    fi
    echo "$output" | grep "^test result:" | while IFS= read -r line; do
        echo "$prefix: $line"
    done
elif echo "$output" | grep -q "^error"; then
    echo "$output"
    echo ""
    echo "hint: compilation error prevented tests from running — fix the errors above"
else
    echo "$output"
fi
