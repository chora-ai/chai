---
description: Inspect Git repository state, history, and diffs. Stage files, create commits, manage branches, merge, rebase, cherry-pick, and reset.
capability_tier: moderate
metadata:
  requires:
    bins: ["git"]
---

## Skill Directives

- always check `git_status` before interpreting diffs to understand the current state
- always use specific refs (commit hashes, branch names) rather than ambiguous references
- always write clear, concise, conventional commit messages that describe the change
- always ask the user to revert commits to `main` or `release/*` - the branches are protected
- only use `force: true` on `git_branch_delete` after verifying the branch was squash-merged
- when `git_diff` output is truncated, use `git_diff_lines` with the `start_line` shown in the truncation notice to read the remaining lines
- when `git_show` output is truncated, use `git_show_lines` with the `start_line` shown in the truncation notice to read the remaining lines
