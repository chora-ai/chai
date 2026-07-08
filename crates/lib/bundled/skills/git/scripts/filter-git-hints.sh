#!/bin/sh
# Strip git's own hint: lines from output. Git emits hints like:
#   hint: If you are sure you want to delete it, run 'git branch -D ...'
#   hint: Disable this message with "git config set advice.forceDeleteBranch false"
# These reference commands the agent cannot run (no direct git access, no
# git config). The executor's hintConditions append agent-relevant hints
# after postProcess, so stripping git's hints here leaves only the error
# message and the custom hint.
#
# Non-matching output (no hint: lines) passes through unchanged.

input=$(cat)
printf '%s\n' "$input" | grep -v '^hint:' | sed '/^$/d'
