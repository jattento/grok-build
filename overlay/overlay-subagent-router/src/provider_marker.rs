//! Provider-failover retry marker for ACP error `data`.
//!
//! Operates on raw JSON values so the shell can keep thin `acp::Error` wrappers
//! while this crate owns merge/lookup policy. Public client-facing shapes stay
//! close to upstream: overload keeps `data = None`, rate limits keep a plain
//! string, and object payloads only gain an extra bool field.

pub const RETRYABLE_PROVIDER_FAILURE_KEY: &str = "retryable_provider_failure";

/// Merge a retryability marker into ACP error `data`, preserving message /
/// http_status fields when `data` is already an object.
pub fn merge_retryable_provider_failure(
    data: Option<serde_json::Value>,
    fallback_message: &str,
    retryable_provider_failure: bool,
) -> serde_json::Value {
    match data {
        Some(serde_json::Value::Object(mut map)) => {
            map.insert(
                RETRYABLE_PROVIDER_FAILURE_KEY.to_string(),
                retryable_provider_failure.into(),
            );
            serde_json::Value::Object(map)
        }
        Some(serde_json::Value::String(message)) => serde_json::json!({
            "message": message,
            "retryable_provider_failure": retryable_provider_failure,
        }),
        Some(other) => serde_json::json!({
            "detail": other,
            "retryable_provider_failure": retryable_provider_failure,
        }),
        None => serde_json::json!({
            "message": fallback_message,
            "retryable_provider_failure": retryable_provider_failure,
        }),
    }
}

/// Read an explicit retry marker from ACP error `data`, if present.
pub fn retryable_provider_failure_from_data(data: Option<&serde_json::Value>) -> Option<bool> {
    data.and_then(|data| data.get(RETRYABLE_PROVIDER_FAILURE_KEY))
        .and_then(serde_json::Value::as_bool)
}

/// Whether an ACP error should trigger provider failover.
///
/// Explicit data marker wins. Otherwise: stable overload copy is retryable,
/// and the dedicated rate-limit ACP code is retryable — both without needing
/// object-shaped `data`.
pub fn is_retryable_provider_failure(
    data: Option<&serde_json::Value>,
    message: &str,
    code: i32,
    rate_limited_error_code: i32,
    overloaded_user_message: &str,
) -> bool {
    if let Some(flag) = retryable_provider_failure_from_data(data) {
        return flag;
    }
    if message == overloaded_user_message {
        return true;
    }
    code == rate_limited_error_code
}

/// Apply the marker after upstream ACP mapping while preferring upstream
/// public `data` shapes.
///
/// - Retryable overload: leave `data` unset (Display-safe copy).
/// - Vetoed overload: stamp explicit `false` so the message heuristic does not fire.
/// - Retryable rate-limit: leave plain-string `data`.
/// - Vetoed rate-limit: stamp explicit `false`.
/// - Retryable objects: insert the bool (keeps `message` / `http_status`).
/// - Non-retryable objects/strings and retryable plain strings: leave unchanged
///   so client-facing Display stays upstream-shaped; the private subagent path
///   calls [`merge_retryable_provider_failure`] when it needs a wire marker.
pub fn apply_retryable_provider_failure_marker(
    data: Option<serde_json::Value>,
    message: &str,
    code: i32,
    rate_limited_error_code: i32,
    overloaded_user_message: &str,
    retryable_provider_failure: bool,
) -> Option<serde_json::Value> {
    if message == overloaded_user_message {
        return if retryable_provider_failure {
            data
        } else {
            Some(merge_retryable_provider_failure(data, message, false))
        };
    }
    if code == rate_limited_error_code {
        return if retryable_provider_failure {
            data
        } else {
            Some(merge_retryable_provider_failure(data, message, false))
        };
    }
    match data {
        Some(serde_json::Value::Object(mut map)) if retryable_provider_failure => {
            map.insert(RETRYABLE_PROVIDER_FAILURE_KEY.to_string(), true.into());
            Some(serde_json::Value::Object(map))
        }
        // Non-retryable string/object/None keep the upstream public shape.
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OVERLOAD: &str = "Model is temporarily overloaded. Try again in a moment.";
    const RATE_LIMIT_CODE: i32 = -32003;

    #[test]
    fn merge_preserves_object_fields() {
        let data = serde_json::json!({ "message": "upstream unavailable", "http_status": 503 });
        let marked = merge_retryable_provider_failure(Some(data), "fallback", true);
        assert_eq!(marked["message"], "upstream unavailable");
        assert_eq!(marked["http_status"], 503);
        assert_eq!(marked["retryable_provider_failure"], true);
    }

    #[test]
    fn merge_wraps_string_data() {
        let marked = merge_retryable_provider_failure(
            Some(serde_json::Value::String("timeout".into())),
            "fallback",
            true,
        );
        assert_eq!(marked["message"], "timeout");
        assert_eq!(marked["retryable_provider_failure"], true);
    }

    #[test]
    fn apply_keeps_retryable_overload_data_unset() {
        let out = apply_retryable_provider_failure_marker(
            None,
            OVERLOAD,
            -32603,
            RATE_LIMIT_CODE,
            OVERLOAD,
            true,
        );
        assert_eq!(out, None);
        assert!(is_retryable_provider_failure(
            None,
            OVERLOAD,
            -32603,
            RATE_LIMIT_CODE,
            OVERLOAD
        ));
    }

    #[test]
    fn apply_stamps_false_for_vetoed_overload() {
        let out = apply_retryable_provider_failure_marker(
            None,
            OVERLOAD,
            -32603,
            RATE_LIMIT_CODE,
            OVERLOAD,
            false,
        );
        assert_eq!(
            retryable_provider_failure_from_data(out.as_ref()),
            Some(false)
        );
        assert!(!is_retryable_provider_failure(
            out.as_ref(),
            OVERLOAD,
            -32603,
            RATE_LIMIT_CODE,
            OVERLOAD
        ));
    }

    #[test]
    fn apply_keeps_rate_limit_string_data() {
        let data = Some(serde_json::Value::String("Rate limit exceeded".into()));
        let out = apply_retryable_provider_failure_marker(
            data.clone(),
            "Rate limited",
            RATE_LIMIT_CODE,
            RATE_LIMIT_CODE,
            OVERLOAD,
            true,
        );
        assert_eq!(out, data);
        assert!(is_retryable_provider_failure(
            out.as_ref(),
            "Rate limited",
            RATE_LIMIT_CODE,
            RATE_LIMIT_CODE,
            OVERLOAD
        ));
    }

    #[test]
    fn apply_inserts_marker_on_status_objects_only() {
        let data = serde_json::json!({ "message": "at capacity", "http_status": 503 });
        let out = apply_retryable_provider_failure_marker(
            Some(data),
            "Internal error",
            -32603,
            RATE_LIMIT_CODE,
            OVERLOAD,
            true,
        )
        .expect("object data");
        assert_eq!(out["message"], "at capacity");
        assert_eq!(out["http_status"], 503);
        assert_eq!(out["retryable_provider_failure"], true);

        let plain = Some(serde_json::Value::String(
            "empty response from model".into(),
        ));
        let kept = apply_retryable_provider_failure_marker(
            plain.clone(),
            "Internal error",
            -32603,
            RATE_LIMIT_CODE,
            OVERLOAD,
            true,
        );
        assert_eq!(kept, plain);
    }

    #[test]
    fn unmarked_string_errors_are_not_retryable() {
        let data = Some(serde_json::Value::String("idle timeout".into()));
        assert!(!is_retryable_provider_failure(
            data.as_ref(),
            "Internal error",
            -32603,
            RATE_LIMIT_CODE,
            OVERLOAD
        ));
    }
}
