#!/bin/sh
# Post-process hint for git_cherry_pick: detect conflicts or successful no-commit pick.
# Appends a diagnostic hint to guide the agent's next step.
# Receives git cherry-pick output on stdin.
# Usage: hint-cherry-pick.sh <no_commit>

no_commit="$1"

output=$(cat)

if echo "$output" | grep -q "CONFLICT"; then
    printf '%s\n' "$output"
    echo ""
    echo "hint: cherry-pick conflicts detected — resolve conflicts, stage them with git_add, then use git_cherry_pick_continue to proceed (or git_cherry_pick_abort to cancel)"
elif [ "$no_commit" = "true" ]; then
    # With --no-commit, git may produce no output on silent success.
    # Always append the hint since the only successful outcome is staged changes
    # that need a commit.
    printf '%s\n' "$output"
    echo ""
    echo "hint: cherry-pick staged — use git_commit to finalize with a conventional commit message"
else
    printf '%s\n' "$output"
fi
