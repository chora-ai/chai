#!/bin/sh
# Post-process hint for skills_write_tools_json: auto-validate after write.
# Receives the chai skill write-tools-json output on stdin.
# Extracts the skill name from the output and runs validation.
# Appends validation summary as a hint.

input=$(cat)

# Extract skill name from output: "wrote tools.json for 'SKILL_NAME' (N bytes, version HASH)"
skill_name=$(echo "$input" | sed -n "s/.*wrote tools.json for '\([^']*\)'.*/\1/p" | head -1)

if [ -z "$skill_name" ]; then
    printf '%s' "$input"
    exit 0
fi

# Run validation and capture output
val_output=$(chai skill validate "$skill_name" 2>&1) || true

# Count errors and warnings
errors=$(echo "$val_output" | grep -c "^ERROR:" 2>/dev/null) || errors=0
warnings=$(echo "$val_output" | grep -c "^WARNING:" 2>/dev/null) || warnings=0
pass=$(echo "$val_output" | grep -c "^PASS" 2>/dev/null) || pass=0

if [ "$errors" -gt 0 ]; then
    printf '%s\n' "$input"
    echo ""
    echo "hint: tools.json written — validation: $errors ERROR(s), $warnings WARNING(s)"
elif [ "$warnings" -gt 0 ]; then
    printf '%s\n' "$input"
    echo ""
    echo "hint: tools.json written — validation: PASS with $warnings WARNING(s)"
else
    printf '%s' "$input"
fi
