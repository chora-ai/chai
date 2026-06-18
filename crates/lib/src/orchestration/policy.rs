//! Delegation policy: session and per-turn caps (see `base/adr/ORCHESTRATION.md`).

use crate::config::AgentsConfig;
use crate::session::SessionStore;

/// Match `[workerId]` at the start of the instruction, inject `workerId`, and strip the bracketed prefix.
///
/// Every worker with a non-empty ID gets an automatic delegation prefix `[workerId]`. The system
/// matches `[` + worker ID + `]` at the start of the `instruction` (full bracket form from opening
/// to closing bracket), injects `workerId`, and strips the matched prefix from the instruction
/// string. This avoids prefix subsumption: workers named `code` and `code-review` produce
/// prefixes `[code]` and `[code-review]`, which are unambiguous because the matcher requires the
/// closing bracket.
pub fn apply_delegation_bracket_match(agents: &AgentsConfig, args: &serde_json::Value) -> serde_json::Value {
    let Some(workers) = agents.workers.as_ref() else {
        return args.clone();
    };
    if workers.is_empty() {
        return args.clone();
    }
    let instruction = args
        .get("instruction")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let trimmed = instruction.trim();

    // Find the first worker whose bracket prefix matches the start of the instruction.
    // Full bracket match: `[workerId]` — the closing bracket prevents prefix subsumption
    // (e.g. `[code]` does not match `[code-review]...`).
    let mut matched_worker_id: Option<&str> = None;
    for w in workers {
        let id = w.id.trim();
        if id.is_empty() {
            continue;
        }
        let bracket = format!("[{}]", id);
        if trimmed.starts_with(&bracket) {
            matched_worker_id = Some(id);
            break;
        }
    }

    let Some(worker_id) = matched_worker_id else {
        return args.clone();
    };

    let mut v = args.clone();
    let Some(obj) = v.as_object_mut() else {
        return args.clone();
    };

    // Inject workerId.
    obj.insert("workerId".to_string(), serde_json::json!(worker_id));

    // Strip the bracket prefix from the instruction.
    if let Some(instr) = obj.get_mut("instruction") {
        if let Some(s) = instr.as_str() {
            let bracket = format!("[{}]", worker_id);
            let stripped = s.trim().strip_prefix(&bracket).unwrap_or(s.trim()).trim();
            *instr = serde_json::Value::String(stripped.to_string());
        }
    }

    v
}

/// Enforces **`maxDelegationsPerSession`** and **`maxDelegationsPerWorker`** before a delegation runs.
pub async fn assert_session_delegation_limits(
    store: &SessionStore,
    session_id: &str,
    agents: &AgentsConfig,
    worker_id: &str,
) -> Result<(), String> {
    let session = store
        .get(session_id)
        .await
        .ok_or_else(|| "session not found".to_string())?;

    if let Some(max) = agents.max_delegations_per_session {
        if session.delegation_count >= max {
            return Err(format!(
                "max delegations per session reached (maxDelegationsPerSession={})",
                max
            ));
        }
    }

    if let Some(ref map) = agents.max_delegations_per_worker {
        if let Some(&limit) = map.get(worker_id) {
            let n = session
                .delegation_by_worker
                .get(worker_id)
                .copied()
                .unwrap_or(0);
            if n >= limit {
                return Err(format!(
                    "max delegations to worker {} for this session reached (configure maxDelegationsPerWorker.{})",
                    worker_id, worker_id
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkerConfig;
    use serde_json::json;

    #[test]
    fn bracket_match_injects_worker_id() {
        let agents = AgentsConfig {
            workers: Some(vec![WorkerConfig {
                id: "read-only".to_string(),
                default_provider: None,
                default_model: None,
                enabled_skills: None,
                context_mode: None,
            }]),
            ..AgentsConfig::default()
        };
        let args = json!({ "instruction": "[read-only] search the files" });
        let merged = apply_delegation_bracket_match(&agents, &args);
        assert_eq!(merged["workerId"], "read-only");
        // The bracket prefix should be stripped from the instruction.
        assert_eq!(merged["instruction"], "search the files");
    }

    #[test]
    fn bracket_match_no_match_returns_args_unchanged() {
        let agents = AgentsConfig {
            workers: Some(vec![WorkerConfig {
                id: "read-only".to_string(),
                default_provider: None,
                default_model: None,
                enabled_skills: None,
                context_mode: None,
            }]),
            ..AgentsConfig::default()
        };
        let args = json!({ "instruction": "search the files" });
        let merged = apply_delegation_bracket_match(&agents, &args);
        assert!(merged.get("workerId").is_none());
        assert_eq!(merged["instruction"], "search the files");
    }

    #[test]
    fn bracket_match_no_subsumption() {
        // `[code]` should not match when instruction starts with `[code-review]`.
        let agents = AgentsConfig {
            workers: Some(vec![
                WorkerConfig {
                    id: "code".to_string(),
                    default_provider: None,
                    default_model: None,
                    enabled_skills: None,
                    context_mode: None,
                },
                WorkerConfig {
                    id: "code-review".to_string(),
                    default_provider: None,
                    default_model: None,
                    enabled_skills: None,
                    context_mode: None,
                },
            ]),
            ..AgentsConfig::default()
        };
        let args = json!({ "instruction": "[code-review] check this" });
        let merged = apply_delegation_bracket_match(&agents, &args);
        // Should match `code-review`, not `code`.
        assert_eq!(merged["workerId"], "code-review");
        assert_eq!(merged["instruction"], "check this");
    }

    #[test]
    fn bracket_match_strips_prefix_and_trims() {
        let agents = AgentsConfig {
            workers: Some(vec![WorkerConfig {
                id: "w".to_string(),
                default_provider: None,
                default_model: None,
                enabled_skills: None,
                context_mode: None,
            }]),
            ..AgentsConfig::default()
        };
        let args = json!({ "instruction": "[w]   do thing  " });
        let merged = apply_delegation_bracket_match(&agents, &args);
        assert_eq!(merged["workerId"], "w");
        assert_eq!(merged["instruction"], "do thing");
    }
}
