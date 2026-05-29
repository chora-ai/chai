# Improvement: Better Skill Instructions for `devtools_search_content`

## Status

Open

## Summary

The `devtools_search_content` tool description and skill instructions could be improved to set better expectations about behavior and help agents use the tool correctly.

## Suggested Improvements

1. **No-match behavior**: The tool description should note that searching with no matches currently returns an error string (exit status 1). Agents should expect this and not treat it as a fatal failure. Once BUG_GREP_EXIT_1 is fixed, this note should be removed.

2. **Regex flavor**: The tool description says "basic regex supported" but does not clarify that only BRE (basic regular expressions) is supported. In BRE, `|`, `+`, `?` are literal characters, not operators. Once BUG_GREP_NO_EXTENDED_REGEX is fixed by adding `-E`, the description should say "extended regex supported" instead.

3. **Recursive default**: The skill instructions say to "always set `recursive` to true when searching directories" but it is easy to forget. Consider making recursive the default in the tool description's parameter schema or adding a stronger note.

## Related Files

- `crates/lib/config/skills/devtools/tools.json` — tool definitions and descriptions
- Whichever file contains the skill instructions text rendered in the system prompt
