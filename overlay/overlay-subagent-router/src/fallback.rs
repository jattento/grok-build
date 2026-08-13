use crate::RouterConfig;

pub const RETRYABLE_PROVIDER_ERROR_NEEDLES: &[&str] = &[
    "rate limit",
    "rate_limit",
    "too many requests",
    "429",
    "quota exceeded",
    "resource exhausted",
    "overloaded",
    "provider unavailable",
    "service unavailable",
    "temporarily unavailable",
    "bad gateway",
    "gateway timeout",
    "upstream connect",
    "upstream reset",
    "upstream timeout",
    "connection reset",
    "connection refused",
    "network error",
    "timed out",
    "timeout",
    "http status 500",
    "http_status\":500",
    "http status 502",
    "http_status\":502",
    "http status 503",
    "http_status\":503",
    "http status 504",
    "http_status\":504",
    "http status 529",
    "http_status\":529",
];

pub fn is_retryable_provider_failure(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    RETRYABLE_PROVIDER_ERROR_NEEDLES
        .iter()
        .any(|needle| lower.contains(needle))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderRetryDecision {
    Stop,
    Retry { provider: String, model: String },
}

pub fn next_provider_retry(
    error: &str,
    fallbacks: &mut Vec<(String, String)>,
) -> ProviderRetryDecision {
    if !is_retryable_provider_failure(error) || fallbacks.is_empty() {
        return ProviderRetryDecision::Stop;
    }
    let (provider, model) = fallbacks.remove(0);
    ProviderRetryDecision::Retry { provider, model }
}

pub fn provider_retry_exhausted_error(error: &str, attempted_providers: &[String]) -> String {
    if attempted_providers.is_empty() {
        return format!("Session error: {error}");
    }
    format!(
        "Session error after trying providers [{}]: {error}",
        attempted_providers.join(", ")
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
