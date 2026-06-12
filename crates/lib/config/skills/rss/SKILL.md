---
description: Monitor RSS and Atom feeds for new content.
capability_tier: moderate
metadata:
  requires:
    bins: ["curl", "cat"]
---

Feeds are configured in `rss-feeds.txt` in the active profile's sandbox directory with one entry per line in `name|url` format:

```
arxiv-cs-ai|https://rss.arxiv.org/rss/cs.AI
arxiv-cs-cr|https://rss.arxiv.org/rss/cs.CR
```

## Skill Directives

- always call `rss_list_feeds` first to see what feeds are configured
- always use feed names from the configured list when available
- always summarize feed entries rather than returning the raw table
- never follow external links from feed entries without evaluating relevance
