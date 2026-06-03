# FEAT: Cargo Skill

Expose `cargo` commands (check, test, build) as a chai skill so the agent can verify code changes compile and pass tests during a session.

## Problem

The agent can read and write source code but cannot verify that changes compile or that tests pass. When making extensive changes across multiple files (e.g., `ollama.rs`, `openai_compat.rs`, `nim.rs`, `agent.rs`, `mod.rs`), the agent can't verify them against the compiler. This is a significant gap — the agent operates blind, unable to catch type errors, missing struct fields, or broken tests until a human rebuilds.

## Proposed Skill: `cargo`

### Tools

| Tool | Description |
|------|-------------|
| `cargo_check` | Run `cargo check` on the workspace or a specific package. Returns compilation errors/warnings. |
| `cargo_test` | Run `cargo test` on the workspace or a specific package. Returns test results. |
| `cargo_build` | Run `cargo build` on the workspace or a specific package. Returns build output. |

### Parameters

Each tool should accept:
- `package` (optional): Limit to a specific package (e.g. `"lib"`, `"cli"`, `"desktop"`)
- `target` (optional): Specific test target or binary
- `args` (optional): Additional cargo arguments (e.g. `--features`, `--release`)

### Security Considerations

- Cargo commands are **read-only with respect to source code** — they produce build artifacts but don't modify source files.
- Build artifacts go into `target/`, which is gitignored. No risk of source contamination.
- Long-running test suites could consume provider context with output. Consider truncating output to errors/summary only.
- `cargo test` executes arbitrary code from the crate's test functions. This is the same risk as any `cargo test` run.

### Implementation Notes

- The skill could wrap `cargo` via `chai` subcommands (similar to how `files` wraps `cat`/`ls`/`grep` via `chai file`).
- Output should be truncated to a reasonable size (e.g., last 100 lines) to avoid context overflow.
- A successful `cargo check` with no output should return a brief confirmation, not empty output.
- Test failures should include the test name and error message, not the full test output.

## Priority

High. This is the most impactful skill to add next because it enables the agent to verify all code changes immediately, catching errors that would otherwise persist until a human rebuilds.
