#!/bin/sh
# Post-process script: transform RSS/Atom XML into structured text.
# Receives raw XML on stdin (from curl).
# Outputs entries as: TITLE | DATE | LINK | SUMMARY
# Handles both RSS 2.0 (<item>) and Atom (<entry>) feeds.
# Limits output to the 20 most recent entries.
#
# Usage: curl <feed_url> | parse-rss.sh

tmpfile=$(mktemp)
trap 'rm -f "$tmpfile"' EXIT
cat > "$tmpfile"

# Detect feed type
is_atom=false
grep -q '<feed' "$tmpfile" && is_atom=true

# Extract text content between XML tags
# Handles both <tag>text</tag> and <tag><![CDATA[text]]></tag>
extract_tag() {
    tag="$1"
    text="$2"
    result=$(printf '%s' "$text" | sed -n "s/.*<${tag}[^>]*><!\[CDATA\[\(.*\)\]\]><\/${tag}>.*/\1/p" | head -1)
    if [ -z "$result" ]; then
        result=$(printf '%s' "$text" | sed -n "s/.*<${tag}[^>]*>\([^<]*\)<\/${tag}>.*/\1/p" | head -1)
    fi
    printf '%s' "$result"
}

# Extract Atom link (href attribute)
extract_atom_link() {
    result=$(printf '%s' "$1" | sed -n 's/.*<link[^>]*rel="alternate"[^>]*href="\([^"]*\)".*/\1/p' | head -1)
    if [ -z "$result" ]; then
        result=$(printf '%s' "$1" | sed -n 's/.*<link[^>]*href="\([^"]*\)".*/\1/p' | head -1)
    fi
    printf '%s' "$result"
}

# Strip HTML tags and normalize whitespace
strip_html() {
    printf '%s' "$1" | sed 's/<[^>]*>//g' | tr -s '[:space:]' ' ' | sed 's/^ *//;s/ *$//'
}

# Split XML into item/entry blocks using awk
if [ "$is_atom" = true ]; then
    delim_open="<entry"
    delim_close="</entry>"
else
    delim_open="<item"
    delim_close="</item>"
fi

# Extract blocks between open/close tags (collapse each block to one line)
blocks=$(awk -v otag="$delim_open" -v ctag="$delim_close" '
{
    line = $0
    while (line != "") {
        if (inside) {
            ci = index(line, ctag)
            if (ci > 0) {
                block = block " " substr(line, 1, ci - 1)
                print block
                inside = 0
                block = ""
                line = substr(line, ci + length(ctag))
            } else {
                block = block " " line
                break
            }
        } else {
            oi = index(line, otag)
            if (oi > 0) {
                inside = 1
                block = ""
                line = substr(line, oi + length(otag))
                # Skip to end of opening tag
                gi = index(line, ">")
                if (gi > 0) line = substr(line, gi + 1)
            } else {
                break
            }
        }
    }
}' "$tmpfile")

count=0
max=20
header_printed=false

printf '%s\n' "$blocks" | while IFS= read -r block; do
    [ -z "$block" ] && continue
    [ $count -ge $max ] && break

    title=$(extract_tag "title" "$block")
    [ -z "$title" ] && continue

    # Date: RSS uses pubDate, Atom uses published or updated
    date=$(extract_tag "pubDate" "$block")
    [ -z "$date" ] && date=$(extract_tag "published" "$block")
    [ -z "$date" ] && date=$(extract_tag "updated" "$block")

    # Link
    if [ "$is_atom" = true ]; then
        link=$(extract_atom_link "$block")
    else
        link=$(extract_tag "link" "$block")
    fi

    # Summary/description
    summary=$(extract_tag "description" "$block")
    [ -z "$summary" ] && summary=$(extract_tag "summary" "$block")
    summary=$(strip_html "$summary")
    # Truncate to 200 chars
    if [ ${#summary} -gt 200 ]; then
        summary="$(printf '%s' "$summary" | cut -c1-197)..."
    fi

    if [ "$header_printed" = false ]; then
        printf '%s\n' "TITLE | DATE | LINK | SUMMARY"
        printf '%s\n' "------|------|------|--------"
        header_printed=true
    fi

    printf '%s | %s | %s | %s\n' "$title" "$date" "$link" "$summary"
    count=$((count + 1))
done

# If the while loop (subshell) produced no output, print fallback
if ! printf '%s\n' "$blocks" | grep -q '<title\|<pubDate\|<published\|<link'; then
    echo "No entries found in feed."
fi
