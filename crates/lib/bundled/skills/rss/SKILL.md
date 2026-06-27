---
description: Monitor RSS and Atom feeds for new content.
capability_tier: moderate
metadata:
  requires:
    bins: ["curl", "cat"]
---

## Skill Guidelines

- Call `rss_list_feeds` first to see what feeds are configured.
- Use feed names from the configured list when available — they are more robust than direct URLs.

Feeds are configured in `rss-feeds.txt` in the active profile's sandbox directory with one entry per line in `name|url` format:

```
arxiv-cs-ai|https://rss.arxiv.org/rss/cs.AI
arxiv-cs-cr|https://rss.arxiv.org/rss/cs.CR
```
