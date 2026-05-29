# Updating A Skill

This guide describes how skill **revisions** are stored today and how to publish a new revision—preferably with the **`chai`** CLI, and optionally by hand if you need full control.

## Where Skills Live

- Skill tree: **`~/.chai/skills/`** (shared by all profiles). The CLI, gateway, and desktop all use this path.
- Each skill is a **directory name** you chose when the skill was created (for example `notesmd-daily`). That name is the stable **package id**; it is not a version label.

Layout for a **versioned** skill (the usual case after `chai init` or `chai skill init`):

```text
~/.chai/skills/<skill-name>/
  active -> versions/<12-hex-chars>/    # symlink: which revision is “live”
  versions/
    <hash-a>/                           # immutable snapshot (directory name = content hash)
      SKILL.md
      tools.json
      scripts/
        ...
    <hash-b>/
      ...
```

The gateway loads files from the directory **`active`** resolves to (`resolve_active_dir` in the library). Older snapshots under **`versions/`** stay on disk unless you delete them; they are the revision history for that package.

## What The Version Directory Name Means

You **cannot** pick an arbitrary folder name such as `next` or `v2` for a revision. Under **`versions/`**, each directory **must** be named exactly **`12` lowercase hexadecimal characters**: the **truncated SHA-256** of the skill’s **canonical content** (see below). The name is both the address and the integrity check: if the bytes match the hash, the tree is self-consistent.

If the name does not match the content, tools that compare the path to a computed hash will disagree with what you expect.

## How The Content Hash Is Defined

Implementation reference: `crates/lib/src/skills/versioning.rs` (`compute_content_hash`, `compute_hash_from_entries`).

- **Included:** every **regular file** under the directory being hashed, with paths relative to that directory, **`/`** separators, sorted lexicographically by path.
- **Excluded at the skill package root only:** the top-level **`versions/`** directory and **`active`** (so hashing the package root does not mix in old snapshots or metadata).
- **Excluded everywhere:** symlinks (only regular files count).
- **Algorithm:** for each path in order, update SHA-256 with `path_bytes`, then `NUL`, then raw file bytes. Take the first **12** hex characters of the hex digest.

Details that affect the hash: exact bytes (including newlines), path spellings, and which files exist. Permissions are **not** part of the hash; scripts are still given mode `755` when written via the CLI.

## Recommended: Update Using The CLI

The **`chai skill`** commands that **write** files do not edit the current snapshot in place. They copy the **current active** tree to a staging area, apply your change, compute the new hash, move the result to **`versions/<new-hash>/`**, and repoint **`active`**. The command prints the new **`version <hash>`**.

| Command | What it updates |
|--------|------------------|
| `chai skill write-skill-md --skill-name <name> --content '...'` | `SKILL.md` |
| `chai skill write-tools-json --skill-name <name> --content '...'` | `tools.json` (JSON is validated before write) |
| `chai skill write-script --skill-name <name> --script-name <base> --content '...'` | `scripts/<base>.sh` (no `..`, `/`, or `\` in `script_name`) |

**Multi-file changes:** run one command **per file** you change. Each run builds a **new** revision from whatever **`active`** was at the start of that command. For example, updating `SKILL.md` and then `tools.json` creates **two** new hashes (two snapshots). That is normal; you do not need to invent a staging directory name yourself.

**Inspect before/after:**

- `chai skill list` — which skills exist and rough status.
- `chai skill read --skill-name <name> --file skill_md` or `--file tools_json` — content from the **active** revision.

**Validate tools:**

```bash
chai skill validate --skill-name <name>
```

There is **no** `chai skill hash <path>` (or similar) in the current CLI: you do not get a standalone “hash this folder” command. The hash appears when you use **`write-*`** or when you implement the algorithm yourself (see below).

## Lockfiles And Profiles

Skills are shared across profiles; each profile can pin hashes in **`~/.chai/profiles/<profile>/skills.lock`**.

- **`chai skill lock`** — record the **current active** hash for each discovered skill and bump the lock **generation**.
- **`chai skill generations`** — list stored generations.
- **`chai skill rollback <generation>`** — restore a saved lock generation and repoint **`active`** symlinks for skills that still have the matching **`versions/<hash>/`** on disk.

After changing skills, whether you need **`lock`** depends on how you use strict lock checking for the gateway; see your profile’s configuration. This guide only documents where revisions live and how to create them.

## What Not To Do

- **Do not** edit files in place under **`versions/<hash>/`** to “move forward.” Those directories are meant to be **immutable**; changing bytes without changing the directory name breaks the content-addressed model.
- **Do not** add a new directory under **`versions/`** with a made-up name. The name must equal the hash of **that directory’s files** (same rules as above).

## Manual Workflow (Without A Dedicated `chai hash` Command)

Use this when you want to edit several files in an editor and produce **one** new revision, or when you are scripting outside the **`write-*`** commands.

1. **Start from the active tree**  
   Resolve **`active`** (or use **`chai skill read`** / copy from **`versions/<current>/`** if you are sure which hash is active).

2. **Work in a temporary folder**  
   Copy **only** the skill payload (for example `SKILL.md`, `tools.json`, `scripts/`) into an empty working directory. Do **not** copy **`versions/`** or **`active`** into this tree; the hash must be computed over the same file set the runtime would hash for a snapshot.

3. **Edit** your files there.

4. **Compute the 12-character hash** using the same rules as the library (sorted paths, `path + NUL + bytes`, SHA-256, first 12 hex chars). There is no **`chai`** subcommand for this today; use a small script in your preferred language, or reproduce the algorithm from **`versioning.rs`**.

5. **Install the snapshot** under the skill package:
   - `mkdir -p ~/.chai/skills/<name>/versions/<hash>`
   - Copy your working tree into **`versions/<hash>/`** (preserving layout).
   - Point **`active`** at **`versions/<hash>`** (relative symlink **`active` → `versions/<hash>`** is what the library creates).

6. **Optional:** `chai skill validate` and **`chai skill lock`** if you use lockfiles.

If you copy an existing **`versions/<old-hash>/`** tree to **`versions/<wrong-guess>/`** without recomputing the hash, the directory name will not match the content; downstream checks that compare hash to content can fail or behave inconsistently.

## Summary

| Question | Answer |
|----------|--------|
| Can I name a new version directory `next` or arbitrary text? | **No.** Under **`versions/`**, the name must be the **12-hex content hash**. |
| Does **`chai`** print the hash? | **`write-skill-md`**, **`write-tools-json`**, and **`write-script`** print the new version hash. There is no standalone **`chai skill hash <dir>`** today. |
| How do I change multiple files? | Either run **`write-*`** once per file (each step creates a new revision), or prepare a full tree, compute the hash yourself, and create **`versions/<hash>/`** + update **`active`**. |
| Where is the skill id? | The directory name **`~/.chai/skills/<this-part>/`**; it is unrelated to the content hash. |
