#!/bin/sh
# Sanitize ls output for agent consumption.
# Removes total line and permission patterns from ls -l output.
# Passes through plain ls output unchanged.

# Read input
input=$(cat)

# If the input starts with "total" (ls -l header), remove that line
echo "$input" | sed '/^total/d'
