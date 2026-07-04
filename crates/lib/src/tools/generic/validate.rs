//! Schema validation for tool call arguments.
//!
//! Validates that tool call parameters conform to the tool's JSON schema
//! (from `tools.json`). The schema is the contract: undeclared parameters
//! and type mismatches are rejected before execution.

/// Check a JSON value against its schema's `type` constraint.
/// Returns `None` if the value matches, or `Some(description)` if it doesn't.
/// Null values are treated as absent — type checking is skipped.
pub(crate) fn check_type(
    param_schema: &serde_json::Value,
    value: &serde_json::Value,
) -> Option<String> {
    // null is treated as absent for type checking.
    if value.is_null() {
        return None;
    }
    let type_decl = param_schema.get("type")?.as_str()?;
    match type_decl {
        "string" => {
            if !value.is_string() {
                return Some(format!("expected string, got {}", json_type_name(value)));
            }
        }
        "integer" => {
            if !value.is_i64() {
                // Accept numbers that are integers (e.g. 5.0 as i64).
                if value.is_f64() {
                    if let Some(f) = value.as_f64() {
                        if f.fract() != 0.0 {
                            return Some(format!(
                                "expected integer, got number with fractional part"
                            ));
                        }
                    }
                } else {
                    return Some(format!("expected integer, got {}", json_type_name(value)));
                }
            }
        }
        "number" => {
            if !value.is_number() {
                return Some(format!("expected number, got {}", json_type_name(value)));
            }
        }
        "boolean" => {
            if !value.is_boolean() {
                return Some(format!("expected boolean, got {}", json_type_name(value)));
            }
        }
        "array" => {
            if !value.is_array() {
                return Some(format!("expected array, got {}", json_type_name(value)));
            }
        }
        "object" => {
            if !value.is_object() {
                return Some(format!("expected object, got {}", json_type_name(value)));
            }
        }
        // Unknown type declaration — skip type checking (don't reject).
        _ => {}
    }
    None
}

/// Human-readable type name for a JSON value.
fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_type_string_match() {
        let schema = serde_json::json!({"type": "string"});
        let value = serde_json::json!("hello");
        assert!(check_type(&schema, &value).is_none());
    }

    #[test]
    fn check_type_string_mismatch() {
        let schema = serde_json::json!({"type": "string"});
        let value = serde_json::json!(42);
        assert!(check_type(&schema, &value).is_some());
    }

    #[test]
    fn check_type_integer_match() {
        let schema = serde_json::json!({"type": "integer"});
        let value = serde_json::json!(42);
        assert!(check_type(&schema, &value).is_none());
    }

    #[test]
    fn check_type_integer_mismatch_string() {
        let schema = serde_json::json!({"type": "integer"});
        let value = serde_json::json!("42");
        assert!(check_type(&schema, &value).is_some());
    }

    #[test]
    fn check_type_boolean_match() {
        let schema = serde_json::json!({"type": "boolean"});
        let value = serde_json::json!(true);
        assert!(check_type(&schema, &value).is_none());
    }

    #[test]
    fn check_type_boolean_mismatch() {
        let schema = serde_json::json!({"type": "boolean"});
        let value = serde_json::json!("true");
        assert!(check_type(&schema, &value).is_some());
    }

    #[test]
    fn check_type_null_skipped() {
        let schema = serde_json::json!({"type": "string"});
        let value = serde_json::Value::Null;
        // null is treated as absent — type check is skipped.
        assert!(check_type(&schema, &value).is_none());
    }

    #[test]
    fn check_type_no_type_decl() {
        let schema = serde_json::json!({"description": "no type"});
        let value = serde_json::json!(42);
        // No "type" field — skip type checking.
        assert!(check_type(&schema, &value).is_none());
    }
}
