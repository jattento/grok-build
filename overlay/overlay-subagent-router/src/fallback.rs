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

pub fn provider_retry_error(
    error: &str,
    attempted_providers: &[String],
    exhausted: bool,
) -> String {
    if !exhausted || attempted_providers.is_empty() {
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
