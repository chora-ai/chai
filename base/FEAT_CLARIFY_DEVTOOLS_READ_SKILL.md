# Improvement: Clarify Purpose of `devtools-read` Skill Variant

## Status

Open

## Summary

There are two devtools skill directories under `crates/lib/config/skills/`:

- `devtools/` — the active skill with all five tools (read_file, list_dir, search_content, write_file, delete_file)
- `devtools-read/` — a variant that appears to be a read-only subset

Both reference `AGENTS.md` in their `sideRead` specs. The purpose and relationship between these two skills is unclear. This could lead to confusion about which skill is active, what tools are available, and whether the read-only variant is intended to be used in certain profiles or contexts.

## Suggested Improvements

1. Add a `README.md` or comment in `tools.json` explaining the purpose of `devtools-read` vs `devtools`.
2. If `devtools-read` is a legacy/unused variant, consider removing it or moving it to an archive.
3. If it's intentionally kept (e.g. for restricted profiles), document which profiles use which variant and why.

## Related Files

- `crates/lib/config/skills/devtools/tools.json`
- `crates/lib/config/skills/devtools-read/tools.json`
