//! Goal-evaluator fallbacks that must not live in the session model.
//!
//! Hidden `/goal` roles (evaluator, and any other structured-output judge)
//! inherit the pane's model today. Cheap models such as DeepSeek flash can
//! call tools but will not emit a strict JSON object in `assistant_text`.
//! The session model still goes first; these two ids are the ordered backups.

/// First backup when the session model cannot produce a verdict.
pub const BACKUP_MODEL: &str = "grok-4.6";

/// Second backup, after Grok 4.6.
pub const SECOND_FALLBACK_MODEL: &str = "claude-sonnet-5";

/// Session model, then Grok 4.6, then Claude Sonnet. Drops blanks and
/// duplicates so a session already on a backup does not waste an attempt.
pub fn evaluator_models(session_model: &str) -> Vec<String> {
    let mut models = Vec::with_capacity(3);
    for candidate in [session_model, BACKUP_MODEL, SECOND_FALLBACK_MODEL] {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            continue;
        }
        if models.iter().any(|existing| existing == trimmed) {
            continue;
        }
        models.push(trimmed.to_string());
    }
    models
}

/// First JSON object in `raw`: the whole string, a ` ```json ` fence, or the
/// first balanced `{...}`. None when there is no object.
pub fn extract_json_object(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(object) = balanced_object(trimmed) {
        return Some(object);
    }
    if let Some(fenced) = json_fence_body(trimmed)
        && let Some(object) = balanced_object(fenced.trim())
    {
        return Some(object);
    }
    let start = trimmed.find('{')?;
    balanced_object(&trimmed[start..])
}

fn json_fence_body(raw: &str) -> Option<&str> {
    let rest = raw
        .strip_prefix("```json")
        .or_else(|| raw.strip_prefix("```JSON"))
        .or_else(|| raw.strip_prefix("```"))?;
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    let end = rest.find("```")?;
    Some(&rest[..end])
}

fn balanced_object(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    if bytes.first().copied() != Some(b'{') {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (index, &byte) in bytes.iter().enumerate() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match byte {
                b'\\' => escape = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&raw[..=index]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluator_models_session_then_grok_then_sonnet() {
        assert_eq!(
            evaluator_models("opencode-deepseek-v4-flash"),
            ["opencode-deepseek-v4-flash", "grok-4.6", "claude-sonnet-5"]
        );
    }

    #[test]
    fn evaluator_models_skips_session_when_it_is_already_a_backup() {
        assert_eq!(
            evaluator_models("grok-4.6"),
            ["grok-4.6", "claude-sonnet-5"]
        );
        assert_eq!(
            evaluator_models("claude-sonnet-5"),
            ["claude-sonnet-5", "grok-4.6"]
        );
    }

    #[test]
    fn evaluator_models_skips_blank_session() {
        assert_eq!(evaluator_models("  "), ["grok-4.6", "claude-sonnet-5"]);
    }

    #[test]
    fn extract_json_object_accepts_bare_object() {
        let raw = r#"{"decision":"continue","evidence":"x","next_step":"y","blocker_key":""}"#;
        assert_eq!(extract_json_object(raw), Some(raw));
    }

    #[test]
    fn extract_json_object_unwraps_fence_and_review_prose() {
        let inner = r#"{"decision":"continue","evidence":"x","next_step":"y","blocker_key":""}"#;
        let fenced = format!("```json\n{inner}\n```");
        assert_eq!(extract_json_object(&fenced), Some(inner));

        let leaked = format!("<review>Doing.\n{inner}\n");
        assert_eq!(extract_json_object(&leaked), Some(inner));
    }

    #[test]
    fn extract_json_object_rejects_empty_and_prose() {
        assert_eq!(extract_json_object(""), None);
        assert_eq!(extract_json_object("<review>Doing."), None);
        assert_eq!(extract_json_object("expected value"), None);
    }
}
