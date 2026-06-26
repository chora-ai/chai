#!/bin/sh
# Post-process hint for skills_write_skill_md: check for missing frontmatter
# fields and variant naming pattern.
# Receives the chai skill write-skill-md output on stdin.
# Uses the skill_name from args ($1) to read the written file and check
# frontmatter.

input=$(cat)
skill_name="${1:-}"

if [ -z "$skill_name" ]; then
    printf '%s\n' "$input"
    exit 0
fi

# Find the skill directory and active version
skill_dir="${HOME}/.chai/skills/${skill_name}"
if [ ! -d "$skill_dir" ]; then
    printf '%s\n' "$input"
    exit 0
fi

# Resolve the active version
active_link="$skill_dir/active"
if [ -L "$active_link" ]; then
    content_dir=$(readlink -f "$active_link" 2>/dev/null) || content_dir=""
else
    # Try versions directory
    versions_dir="$skill_dir/versions"
    if [ -d "$versions_dir" ]; then
        content_dir=$(ls -td "$versions_dir"/*/ 2>/dev/null | head -1)
        content_dir="${content_dir%/}"
    else
        content_dir="$skill_dir"
    fi
fi

if [ -z "$content_dir" ] || [ ! -f "$content_dir/SKILL.md" ]; then
    printf '%s\n' "$input"
    exit 0
fi

skill_md=$(cat "$content_dir/SKILL.md")

# Check for required frontmatter fields
missing=""
has_frontmatter=$(echo "$skill_md" | head -1 | grep -c "^---")

if [ "$has_frontmatter" -eq 0 ]; then
    missing="description, capability_tier, metadata.requires.bins"
else
    # Extract frontmatter block
    fm_end=$(echo "$skill_md" | awk '/^---/{count++; if(count==2) {print NR; exit}}')
    if [ -z "$fm_end" ]; then
        fm_end=$(echo "$skill_md" | wc -l)
    fi
    frontmatter=$(echo "$skill_md" | head -n "$fm_end")

    if ! echo "$frontmatter" | grep -q "capability_tier"; then
        missing="capability_tier"
    fi
    if ! echo "$frontmatter" | grep -q "metadata"; then
        if [ -n "$missing" ]; then
            missing="$missing, metadata.requires.bins"
        else
            missing="metadata.requires.bins"
        fi
    fi
fi

# Check for variant naming pattern (hyphenated name without variant_of)
variant_hint=""
if echo "$skill_name" | grep -q "-"; then
    # It's a variant name pattern
    if ! echo "$skill_md" | grep -q "variant_of"; then
        variant_hint="skill name '$skill_name' matches variant pattern — consider adding variant_of to frontmatter"
    fi
fi

# Build output
printf '%s\n' "$input"

if [ -n "$missing" ] || [ -n "$variant_hint" ]; then
    echo ""
    if [ -n "$missing" ]; then
        echo "hint: SKILL.md written — missing recommended frontmatter: $missing"
        if [ -n "$variant_hint" ]; then
            echo ""
        fi
    fi
    if [ -n "$variant_hint" ]; then
        echo "hint: $variant_hint"
    fi
fi
