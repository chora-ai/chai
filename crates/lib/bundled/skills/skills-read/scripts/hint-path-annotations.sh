#!/bin/sh
# Post-process hint for skills_read (tools_json): check for parameters that
# look like filesystem paths but lack readPath/writePath annotations.
# Receives the chai skill read output on stdin.
# $1 is the 'file' parameter value ('skill_md' or 'tools_json').

input=$(cat)
file_type="${1:-}"

# Only check when reading tools_json
if [ "$file_type" != "tools_json" ]; then
    printf '%s\n' "$input"
    exit 0
fi

# Verify the input looks like tools.json content
if ! echo "$input" | grep -q '"tools"\|"args"\|"execution"'; then
    printf '%s\n' "$input"
    exit 0
fi

# Simple heuristic: check if any param named *path* or *dir* or *root* or *file*
# appears in args without a corresponding readPath/writePath annotation.
missing_path_hints=""

if echo "$input" | grep -q '"param".*\(path\|dir\|root\|file\)'; then
    # Count params with path-like names vs params with readPath/writePath
    path_params=$(echo "$input" | grep -c '"param".*\(path\|dir\|root\|file\)') || path_params=0
    annotated_params=$(echo "$input" | grep -c '"readPath"\|"writePath"') || annotated_params=0

    if [ "$path_params" -gt "$annotated_params" ]; then
        missing_path_hints="yes"
    fi
fi

printf '%s\n' "$input"

if [ -n "$missing_path_hints" ]; then
    echo ""
    echo "hint: some path-like parameters may lack readPath/writePath annotations — review args for sandbox security"
fi
