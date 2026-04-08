//! Startup validation of skill composition against agent configuration.
//!
//! Two checks run per agent:
//! - **Variant overlap** — warns when two enabled skills share a `model_variant_of` relationship
//!   (e.g. `git` and `git-read` both enabled), creating redundant tool surfaces.
//! - **Tier–model mismatch** — warns when a skill's `capability_tier` exceeds the likely capability
//!   of the agent's configured model (e.g. `full`-tier skill with a 7B local model).

use super::SkillEntry;

/// Validate skill composition for a single agent and log warnings.
///
/// `agent_label` is a human-readable identifier (e.g. "orchestrator", "worker:fast").
/// `enabled_entries` is the filtered set of skills for this agent.
/// `default_model` is the agent's configured default model string (if any).
pub fn validate_skill_composition(
    agent_label: &str,
    enabled_entries: &[SkillEntry],
    default_model: Option<&str>,
) {
    check_variant_overlap(agent_label, enabled_entries);
    if let Some(model) = default_model {
        check_tier_model_mismatch(agent_label, enabled_entries, model);
    }
}

/// Warn when two enabled skills share a variant relationship:
/// - Both have the same `model_variant_of` parent
/// - One is the parent and the other is its variant
fn check_variant_overlap(agent_label: &str, entries: &[SkillEntry]) {
    let len = entries.len();
    for i in 0..len {
        for j in (i + 1)..len {
            let a = &entries[i];
            let b = &entries[j];
            if skills_overlap(a, b) {
                log::warn!(
                    "{}: skill '{}' and '{}' overlap (variant relationship) — \
                     consider enabling only one",
                    agent_label,
                    a.name,
                    b.name,
                );
            }
        }
    }
}

/// Two skills overlap if:
/// - a.model_variant_of == Some(b.name) or b.model_variant_of == Some(a.name)
/// - both have model_variant_of pointing to the same parent
fn skills_overlap(a: &SkillEntry, b: &SkillEntry) -> bool {
    // One is the parent of the other
    if let Some(ref av) = a.model_variant_of {
        if av == &b.name {
            return true;
        }
    }
    if let Some(ref bv) = b.model_variant_of {
        if bv == &a.name {
            return true;
        }
    }
    // Both are variants of the same parent
    if let (Some(ref av), Some(ref bv)) = (&a.model_variant_of, &b.model_variant_of) {
        if av == bv {
            return true;
        }
    }
    false
}

/// Warn when a skill's capability_tier is likely too high for the agent's model.
///
/// Uses a simple heuristic: extract parameter count from the model name string
/// (e.g. "3b" from "llama3.2:3b") and compare against tier thresholds.
/// This is intentionally conservative — it warns, never blocks.
fn check_tier_model_mismatch(agent_label: &str, entries: &[SkillEntry], model: &str) {
    let param_count = extract_param_billions(model);
    for entry in entries {
        let tier = match entry.capability_tier.as_deref() {
            Some(t) => t,
            None => continue,
        };
        let warning = match (tier, param_count) {
            // full-tier skills assume cloud-grade or 70B+ models
            ("full", Some(b)) if b <= 30.0 => Some(format!(
                "capability_tier 'full' may exceed {:.0}B model capability",
                b,
            )),
            // moderate-tier skills target 13B-30B
            ("moderate", Some(b)) if b < 7.0 => Some(format!(
                "capability_tier 'moderate' may exceed {:.0}B model capability",
                b,
            )),
            _ => None,
        };
        if let Some(reason) = warning {
            log::warn!(
                "{}: skill '{}' — {} (model: {})",
                agent_label,
                entry.name,
                reason,
                model,
            );
        }
    }
}

/// Extract parameter count in billions from a model name string.
///
/// Looks for patterns like "3b", "7B", "13b", "70b", "3.2b" in the model name.
/// Returns None if no recognizable parameter count is found (e.g. cloud API model names
/// like "gpt-4" or "claude-opus" that don't embed parameter counts).
fn extract_param_billions(model: &str) -> Option<f64> {
    let lower = model.to_ascii_lowercase();
    // Match patterns: digits (optional decimal) followed by 'b'
    // Common formats: "3b", "7b", "13b", "70b", "1.5b", "3.2b"
    // Preceded by non-alphanumeric or start of string, followed by non-alphanumeric or end
    let bytes = lower.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        // Find a digit that starts a potential parameter count
        if bytes[i].is_ascii_digit() {
            // Check preceding char is not a letter (avoid matching "llama3" as "3b")
            let preceded_by_letter = i > 0 && bytes[i - 1].is_ascii_alphabetic();
            if !preceded_by_letter {
                // Scan the number (digits + optional one decimal point)
                let start = i;
                let mut seen_dot = false;
                while i < len && (bytes[i].is_ascii_digit() || (bytes[i] == b'.' && !seen_dot)) {
                    if bytes[i] == b'.' {
                        seen_dot = true;
                    }
                    i += 1;
                }
                // Check if followed by 'b' (and not another letter after that)
                if i < len && bytes[i] == b'b' {
                    let after_b = i + 1;
                    let followed_by_letter = after_b < len && bytes[after_b].is_ascii_alphabetic();
                    if !followed_by_letter {
                        if let Ok(val) = lower[start..i].parse::<f64>() {
                            return Some(val);
                        }
                    }
                }
                continue;
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_param_billions_common_formats() {
        assert_eq!(extract_param_billions("llama3.2:3b"), Some(3.0));
        assert_eq!(extract_param_billions("llama-3.2-3B-instruct"), Some(3.0));
        assert_eq!(extract_param_billions("mistral-7b-instruct"), Some(7.0));
        assert_eq!(extract_param_billions("codellama:13b"), Some(13.0));
        assert_eq!(extract_param_billions("llama3.1:70b"), Some(70.0));
        assert_eq!(extract_param_billions("qwen2.5:1.5b"), Some(1.5));
    }

    #[test]
    fn extract_param_billions_no_match() {
        // Cloud model names without parameter counts
        assert_eq!(extract_param_billions("gpt-4"), None);
        assert_eq!(extract_param_billions("claude-opus"), None);
        // Version numbers should not match (preceded by letter)
        assert_eq!(extract_param_billions("llama3"), None);
    }

    #[test]
    fn skills_overlap_variant_and_parent() {
        let parent = make_entry("git", None, None);
        let variant = make_entry("git-read", None, Some("git"));
        assert!(skills_overlap(&parent, &variant));
        assert!(skills_overlap(&variant, &parent));
    }

    #[test]
    fn skills_overlap_two_variants_same_parent() {
        let a = make_entry("git-read", None, Some("git"));
        let b = make_entry("git-remote", None, Some("git"));
        assert!(skills_overlap(&a, &b));
    }

    #[test]
    fn skills_no_overlap_unrelated() {
        let a = make_entry("git", None, None);
        let b = make_entry("devtools", None, None);
        assert!(!skills_overlap(&a, &b));
    }

    fn make_entry(
        name: &str,
        capability_tier: Option<&str>,
        model_variant_of: Option<&str>,
    ) -> SkillEntry {
        SkillEntry {
            name: name.to_string(),
            description: String::new(),
            path: std::path::PathBuf::new(),
            content: String::new(),
            tool_descriptor: None,
            capability_tier: capability_tier.map(|s| s.to_string()),
            model_variant_of: model_variant_of.map(|s| s.to_string()),
        }
    }
}
