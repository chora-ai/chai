---
name: rss
description: Monitor RSS and Atom feeds for new content.
metadata:
  requires:
    bins: ["curl", "cat"]
generated_from:
  spec_version: "1.0"
  generator_model: claude-opus-4
  capability_tier: moderate
---

# RSS Feed Monitor

Fetch and read RSS/Atom feeds for monitoring information sources. Feeds are
configured in `rss-feeds.txt` in the active profile's sandbox directory with
one entry per line in `name|url` format. The orchestrator can modify this file
via `devtools_write_file`; worker agents with only the `rss` skill can read
and fetch but not modify.

## Prerequisites

- Create `~/.chai/active/sandbox/rss-feeds.txt` with feed entries:
  ```
  arxiv-cs-ai|https://rss.arxiv.org/rss/cs.AI
  arxiv-cs-cr|https://rss.arxiv.org/rss/cs.CR
  ```

## Skill Directives

- always call `rss_list_feeds` first to see what feeds are configured
- always use feed names from the configured list when available
- always summarize feed entries rather than returning the raw table
- never follow external links from feed entries without evaluating relevance

## Available Tools

- `rss_check_feed`
- `rss_list_feeds`

## Tool Instructions

### List configured feeds

1. Call `rss_list_feeds` to see all configured feed names and URLs.

### Check a feed for new entries

1. Call `rss_check_feed` with `feed` set to the feed name or a direct URL.
2. The output is a structured table with TITLE, DATE, LINK, and SUMMARY columns (up to 20 entries).
3. Summarize the most relevant entries for the user.

### Monitor multiple feeds

1. Call `rss_list_feeds` to get the list of configured feeds.
2. For each feed of interest, call `rss_check_feed` with the feed name.
3. Summarize new entries across all feeds, grouped by source.

## Examples

### rss_list_feeds

{}

### rss_check_feed

{"feed": "arxiv-cs-ai"}
