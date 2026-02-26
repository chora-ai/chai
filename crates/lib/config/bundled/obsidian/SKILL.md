---
name: obsidian
description: Obsidian vaults via the official Obsidian CLI (early access). Binary `obsidian`. Use notesmd-cli skill if unavailable.
homepage: https://help.obsidian.md/cli
metadata:
  requires:
    bins: ["obsidian"]
---

# Obsidian

Binary: `obsidian`
Vault: directory where notes are stored
Note Format: `*.md`

**Commands**

- `obsidian search "query"` — search note names
- `obsidian search:context "query"` — search inside note content
- `obsidian create "Folder/Note"` — create note (optional `--content "..."`)
