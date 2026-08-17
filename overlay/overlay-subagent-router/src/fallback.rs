use crate::RouterConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderRetryDecision {
    Stop,
    Exhausted,
    Retry { provider: String, model: String },
}

pub fn next_provider_retry(
    retryable_provider_failure: bool,
    fallbacks: &mut Vec<(String, String)>,
) -> ProviderRetryDecision {
    if !retryable_provider_failure {
        return ProviderRetryDecision::Stop;
    }
    if fallbacks.is_empty() {
        return ProviderRetryDecision::Exhausted;
    }
    let (provider, model) = fallbacks.remove(0);
    ProviderRetryDecision::Retry { provider, model }
}

/// The caller of `provider_retry_error` (`run_one_turn_attempt` in
/// `xai-grok-shell`) already prefixes turn failures with "Session error: ",
/// so strip it here instead of doubling the prefix.
fn strip_session_error_prefix(error: &str) -> &str {
    error.strip_prefix("Session error: ").unwrap_or(error)
}

pub fn provider_retry_error(
    error: &str,
    attempted_providers: &[String],
    exhausted: bool,
) -> String {
    if !exhausted || attempted_providers.is_empty() {
        return error.to_string();
    }
    format!(
        "Session error after trying providers [{}]: {}",
        attempted_providers.join(", "),
        strip_session_error_prefix(error)
    )
}

pub fn configured_provider_retry_plan(
    primary_model: Option<&str>,
    task_type: &str,
    complexity: &str,
    requires_vision: bool,
) -> (Option<String>, Vec<(String, String)>) {
    crate::load_config(&crate::resolve_config_path())
        .map(|config| {
            let primary_provider =
                primary_model.and_then(|model| provider_for_model(&config, model));
            let fallback_models = config.provider_fallback_models(
                primary_model,
                task_type,
                complexity,
                requires_vision,
            );
            (primary_provider, fallback_models)
        })
        .unwrap_or_default()
}

pub fn provider_for_model(config: &RouterConfig, model: &str) -> Option<String> {
    config.models.get(model).map(|meta| meta.provider.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn providers() -> Vec<String> {
        ["gemini".to_string(), "claude".to_string()].to_vec()
    }

    /// Counts the session-error prefix phrase in either form the wrapper
    /// produces: the plain "Session error: ..." or the exhaustion banner
    /// "Session error after trying providers [...]".
    fn count_session_error_prefix(value: &str) -> usize {
        value.matches("Session error").count()
    }

    #[test]
    fn no_fallback_path_leaves_error_unchanged() {
        let providers = providers();
        let result = provider_retry_error(
            "Session error: Internal error: surge caused a crash",
            &providers,
            false,
        );
        assert_eq!(
            result, "Session error: Internal error: surge caused a crash",
            "caller already prefixes turn failures, overlay must pass them through"
        );
        assert_eq!(count_session_error_prefix(&result), 1);
    }

    #[test]
    fn exhausted_path_has_exactly_one_prefix_stripping_callers() {
        let providers = providers();
        let already_prefixed =
            provider_retry_error("Session error: Internal error: boom", &providers, true);
        assert_eq!(
            already_prefixed,
            "Session error after trying providers [gemini, claude]: \
             Internal error: boom"
        );
        assert_eq!(count_session_error_prefix(&already_prefixed), 1);

        let unprefixed = provider_retry_error("timeout", &providers, true);
        assert_eq!(
            unprefixed,
            "Session error after trying providers [gemini, claude]: timeout"
        );
        assert_eq!(count_session_error_prefix(&unprefixed), 1);
    }

    #[test]
    fn only_leading_prefix_is_stripped() {
        let providers = providers();
        // A message that legitimately mentions the phrase mid-string keeps it.
        let result = provider_retry_error(
            "Session error: Internal error: titled \"Session error: in docs\"",
            &providers,
            true,
        );
        assert_eq!(
            result,
            "Session error after trying providers [gemini, claude]: \
             Internal error: titled \"Session error: in docs\""
        );
        // First occurrence is the wrapper's own prefix; the mid-string one survives.
        assert_eq!(count_session_error_prefix(&result), 2);
    }

    #[test]
    fn exhausted_with_no_providers_degrades_to_passthrough() {
        let result = provider_retry_error("Session error: Internal error: x", &[], true);
        assert_eq!(result, "Session error: Internal error: x");
        assert_eq!(count_session_error_prefix(&result), 1);
    }
}
