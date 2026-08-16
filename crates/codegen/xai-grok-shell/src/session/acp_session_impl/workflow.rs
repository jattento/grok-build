use std::sync::Arc;

use super::super::acp_session::SessionActor;

impl SessionActor {
    pub(crate) fn named_workflow_snapshot(
        &self,
    ) -> (
        crate::session::workflow::registry::WorkflowRegistry,
        Vec<crate::session::workflow::registry::WorkflowListing>,
    ) {
        crate::session::workflow::registry::workflow_snapshot(Some(std::path::Path::new(
            self.session_info.cwd.as_str(),
        )))
    }

    pub(crate) async fn launch_named_workflow(
        self: &Arc<Self>,
        registry: &crate::session::workflow::registry::WorkflowRegistry,
        name: &str,
        input: &str,
    ) -> String {
        let resolved = match registry.resolve_by_name(name) {
            Ok(r) => r,
            Err(e) => return format!("Workflow '{name}' unavailable: {e}"),
        };
        let (args, objective) = parse_named_workflow_args(input, &resolved.meta.description);
        let spec = crate::session::workflow::manager::LaunchSpec {
            objective,
            args,
            agent_budget: None,
            resume_run_id: None,
        };
        let launched = self.workflow_manager.lock().await.launch(resolved, spec);
        match launched {
            Ok((run_id, outcome_rx)) => {
                let (display, objective) = self
                    .workflow_tracker()
                    .await
                    .lock()
                    .get(&run_id)
                    .map(|r| (r.name.clone(), r.objective.clone()))
                    .unwrap_or_else(|| (name.to_string(), String::new()));
                let command_line = if input.trim().is_empty() {
                    format!("/{name}")
                } else {
                    format!("/{name} {}", input.trim())
                };
                self.push_workflow_launch_reminder(
                    &display,
                    &run_id,
                    &objective,
                    &command_line,
                    false,
                );
                tokio::spawn(async move {
                    if let Ok(outcome) = outcome_rx.await {
                        tracing::info!(run_id, ?outcome, "named workflow finished");
                    }
                });
                format!(
                    "Workflow '{display}' started in the background. Watch it in /workflows; \
                     the result lands here when it finishes."
                )
            }
            Err(e) => format!("Could not start workflow '{name}': {e}"),
        }
    }

    pub(crate) async fn manage_workflow_run(self: &Arc<Self>, run_id: &str, op: &str) -> String {
        use crate::session::workflow::tracker::WorkflowRunStatus;

        const USAGE: &str = "Usage: /workflow <name> [args] to launch a saved workflow, or \
                             /workflow <op> [name] (also `/workflow <name> <op>`) to manage \
                             a run — ops: pause, resume, stop, save.";
        if op.is_empty() {
            return USAGE.to_string();
        }

        let matches: Vec<(String, WorkflowRunStatus, String)> = {
            let tracker = self.workflow_tracker().await;
            let tracker = tracker.lock();
            let all: Vec<_> = tracker
                .list()
                .iter()
                .filter(|r| r.run_id.starts_with(run_id) || r.name.starts_with(run_id))
                .map(|r| (r.run_id.clone(), r.status, r.name.clone()))
                .collect();
            narrow_run_matches(all, run_id, op)
        };
        let (full_id, status, name) = match matches.as_slice() {
            [] if run_id.is_empty() => {
                return "No workflow runs in this session yet.".to_string();
            }
            [] => return format!("No workflow run matches '{run_id}'."),
            [one] => one.clone(),
            many => {
                let rows: Vec<String> = many
                    .iter()
                    .map(|(_, status, name)| format!("  {name} ({})", status.as_str()))
                    .collect();
                return format!(
                    "Several runs could be '{op}' — pick one by name:\n{}\n(/workflow {op} <name>)",
                    rows.join("\n")
                );
            }
        };
        let id_suffix = format!(" {name}");

        match op {
            "pause" => {
                if status != WorkflowRunStatus::Active {
                    return format!("Run '{name}' is not active (status: {}).", status.as_str());
                }
                self.workflow_manager.lock().await.pause(&full_id);
                format!("Paused {name}. /workflow resume{id_suffix} to continue.")
            }
            "stop" => {
                if status.is_terminal() {
                    return format!(
                        "Run '{name}' is already finished (status: {}).",
                        status.as_str()
                    );
                }
                self.workflow_manager.lock().await.cancel(&full_id);
                format!("Stopped {name}.")
            }
            "resume" => {
                if status == WorkflowRunStatus::Active {
                    return format!("Run '{name}' is already running.");
                }
                if !status.is_resumable() {
                    return format!(
                        "Run '{name}' cannot be resumed (status: {}). Start a new run instead.",
                        status.as_str()
                    );
                }
                if status == WorkflowRunStatus::BudgetLimited {
                    let (used, limit) = {
                        let tracker = self.workflow_tracker().await;
                        let tracker = tracker.lock();
                        let run = tracker.get(&full_id);
                        (
                            run.as_ref().map_or(0, |r| r.agents_used),
                            run.as_ref().and_then(|r| r.agent_budget),
                        )
                    };
                    let limit = limit.map_or_else(String::new, |l| format!("/{l}"));
                    if used >= xai_workflow::MAX_AGENT_BUDGET {
                        return format!(
                            "Run '{name}' exhausted the maximum agent budget ({used}{limit} agents) \
                             and cannot be resumed. Start a new run instead."
                        );
                    }
                    let suggested = used.saturating_add(64).min(xai_workflow::MAX_AGENT_BUDGET);
                    return format!(
                        "Run '{name}' exhausted its agent budget ({used}{limit} agents). \
                         Resuming keeps all finished work but needs a higher absolute cap — \
                         ask the agent to resume it with an agent budget above {used}, e.g. \
                         \"resume {name} with an agent budget of {suggested}\"."
                    );
                }
                let (script, args) = {
                    let manager = self.workflow_manager.lock().await;
                    (
                        manager.script_copy_for(&full_id),
                        manager.args_copy_for(&full_id),
                    )
                };
                let Some(script) = script else {
                    return format!("No persisted script for '{name}'; cannot resume.");
                };
                let resolved = match crate::session::workflow::registry::resolve_inline(script) {
                    Ok(r) => r,
                    Err(e) => return format!("Persisted script invalid: {e}"),
                };
                let objective = {
                    let tracker = self.workflow_tracker().await;
                    tracker
                        .lock()
                        .get(&full_id)
                        .map(|r| r.objective.clone())
                        .unwrap_or_default()
                };
                let agent_budget = {
                    let tracker = self.workflow_tracker().await;
                    tracker
                        .lock()
                        .get(&full_id)
                        .and_then(|run| run.agent_budget)
                };
                let objective_echo = objective.clone();
                let spec = crate::session::workflow::manager::LaunchSpec {
                    objective,
                    args,
                    agent_budget,
                    resume_run_id: Some(full_id.clone()),
                };
                match self
                    .workflow_manager
                    .lock()
                    .await
                    .launch_resuming(resolved, spec)
                    .await
                {
                    Ok((rid, outcome_rx)) => {
                        tokio::spawn(async move {
                            if let Ok(outcome) = outcome_rx.await {
                                tracing::info!(run_id = rid, ?outcome, "resumed workflow finished");
                            }
                        });
                        self.push_workflow_launch_reminder(
                            &name,
                            &full_id,
                            &objective_echo,
                            &format!("/workflow resume {name}"),
                            true,
                        );
                        format!("Resumed {name} from its journal.")
                    }
                    Err(e) => format!("Could not resume '{name}': {e}"),
                }
            }
            "save" => {
                let Some(script) = self.workflow_manager.lock().await.script_copy_for(&full_id)
                else {
                    return format!("No persisted script for '{name}'; nothing to save.");
                };
                let definition_name =
                    match crate::session::workflow::registry::resolve_inline(script.clone()) {
                        Ok(resolved) => resolved.meta.name,
                        Err(error) => return format!("Could not save workflow '{name}': {error}"),
                    };
                if definition_name != name {
                    return format!(
                        "Save is disabled for run '{name}': it is a duplicate-run display handle, \
                         while the script is still named '{definition_name}'. Choose a new unique \
                         meta.name and save the script under that name instead."
                    );
                }
                if crate::session::workflow::registry::BUILTIN_WORKFLOWS
                    .iter()
                    .any(|builtin| builtin.name == definition_name)
                {
                    return format!(
                        "Save is disabled for built-in workflow '{definition_name}', which is \
                         already runnable. To customize it, create a copy with a new unique \
                         meta.name."
                    );
                }
                match crate::session::workflow::registry::save_project_workflow(
                    std::path::Path::new(self.session_info.cwd.as_str()),
                    &definition_name,
                    &script,
                ) {
                    Ok(path) => format!(
                        "Saved workflow '{definition_name}' to {} — runnable by name from now on.",
                        path.display()
                    ),
                    Err(e) => format!("Could not save workflow '{definition_name}': {e}"),
                }
            }
            other => format!("Unknown op '{other}'. {USAGE}"),
        }
    }

    pub(crate) async fn launch_named_workflow_structured(
        self: &Arc<Self>,
        name: String,
        args: serde_json::Value,
        agent_budget: Option<u64>,
    ) -> Result<crate::session::WorkflowLaunchResult, crate::session::WorkflowCommandError> {
        if !self.background_workflows_enabled {
            return Err(crate::session::WorkflowCommandError::new(
                "workflows_disabled",
                "background workflows are disabled for this session",
            ));
        }
        launch_registered_workflow(
            &self.workflow_manager,
            std::path::Path::new(self.session_info.cwd.as_str()),
            name,
            args,
            agent_budget,
        )
        .await
    }

    pub(crate) async fn workflow_snapshot_structured(
        &self,
    ) -> Result<Vec<serde_json::Value>, crate::session::WorkflowCommandError> {
        let tracker = self.workflow_tracker().await;
        workflow_snapshot_payloads(&tracker.lock())
    }

    pub(crate) async fn control_workflow_structured(
        self: &Arc<Self>,
        run_id: String,
        operation: crate::session::WorkflowControlOperation,
        agent_budget: Option<u64>,
    ) -> Result<serde_json::Value, crate::session::WorkflowCommandError> {
        if operation != crate::session::WorkflowControlOperation::Resume && agent_budget.is_some() {
            return Err(crate::session::WorkflowCommandError::new(
                "workflow_agent_budget_not_applicable",
                "agentBudget is only valid for resume",
            ));
        }
        if operation != crate::session::WorkflowControlOperation::Resume {
            validate_agent_budget(agent_budget)?;
        }

        let tracker = self.workflow_tracker().await;
        let state = tracker.lock().get(&run_id).ok_or_else(|| {
            crate::session::WorkflowCommandError::new(
                "workflow_unknown_run",
                format!("workflow run not found: {run_id}"),
            )
        })?;
        validate_control_operation(&state, operation)?;
        use crate::session::workflow::tracker::WorkflowRunStatus;
        match operation {
            crate::session::WorkflowControlOperation::Pause => {
                if !self.workflow_manager.lock().await.pause(&run_id) {
                    let current = tracker.lock().get(&run_id).unwrap_or(state);
                    return Err(invalid_control_state("pause", &current));
                }
            }
            crate::session::WorkflowControlOperation::Stop => {
                if !self.workflow_manager.lock().await.cancel(&run_id) {
                    let current = tracker.lock().get(&run_id).unwrap_or(state);
                    return Err(invalid_control_state("stop", &current));
                }
            }
            crate::session::WorkflowControlOperation::Resume => {
                if state.status == WorkflowRunStatus::BudgetLimited {
                    validate_budget_limited_resume(&state, agent_budget)?;
                } else {
                    validate_agent_budget(agent_budget)?;
                }
                let (script, args) = {
                    let manager = self.workflow_manager.lock().await;
                    (
                        manager.script_copy_for(&run_id),
                        manager.args_copy_for(&run_id),
                    )
                };
                let script = script.ok_or_else(|| {
                    crate::session::WorkflowCommandError::new(
                        "workflow_resume_source_missing",
                        format!("persisted workflow script is missing for run {run_id}"),
                    )
                })?;
                let resolved = crate::session::workflow::registry::resolve_inline(script)
                    .map_err(map_resolve_error)?;
                let spec = crate::session::workflow::manager::LaunchSpec {
                    objective: state.objective.clone(),
                    args,
                    agent_budget,
                    resume_run_id: Some(run_id.clone()),
                };
                let (_, outcome_rx) = self
                    .workflow_manager
                    .lock()
                    .await
                    .launch_without_completion_turn(resolved, spec)
                    .await
                    .map_err(map_launch_error)?;
                let logged_run_id = run_id.clone();
                tokio::spawn(async move {
                    if let Ok(outcome) = outcome_rx.await {
                        tracing::info!(
                            run_id = logged_run_id,
                            ?outcome,
                            "structured workflow resume finished"
                        );
                    }
                });
            }
        }
        workflow_run_payload(&tracker, &run_id)
    }
}

async fn launch_registered_workflow(
    manager: &Arc<tokio::sync::Mutex<crate::session::workflow::manager::WorkflowManager>>,
    cwd: &std::path::Path,
    name: String,
    args: serde_json::Value,
    agent_budget: Option<u64>,
) -> Result<crate::session::WorkflowLaunchResult, crate::session::WorkflowCommandError> {
    validate_agent_budget(agent_budget)?;
    let registry = crate::session::workflow::registry::WorkflowRegistry::scan(Some(cwd));
    let resolved = registry.resolve_by_name(&name).map_err(map_resolve_error)?;
    let definition_name = resolved.meta.name.clone();
    let objective = structured_workflow_objective(&args, &resolved.meta.description);
    let spec = crate::session::workflow::manager::LaunchSpec {
        objective,
        args,
        agent_budget,
        resume_run_id: None,
    };
    let (run_id, outcome_rx, tracker) = {
        let mut manager = manager.lock().await;
        let tracker = manager.tracker();
        let (run_id, outcome_rx) = manager
            .launch_without_completion_turn(resolved, spec)
            .await
            .map_err(map_launch_error)?;
        (run_id, outcome_rx, tracker)
    };
    let display_name = tracker
        .lock()
        .get(&run_id)
        .map(|run| run.name)
        .unwrap_or(definition_name);
    let logged_run_id = run_id.clone();
    tokio::spawn(async move {
        if let Ok(outcome) = outcome_rx.await {
            tracing::info!(
                run_id = logged_run_id,
                ?outcome,
                "structured workflow finished"
            );
        }
    });
    Ok(crate::session::WorkflowLaunchResult {
        run_id,
        name: display_name,
    })
}

fn structured_workflow_objective(args: &serde_json::Value, description: &str) -> String {
    args.get("objective")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| description.to_string())
}

fn validate_agent_budget(
    agent_budget: Option<u64>,
) -> Result<(), crate::session::WorkflowCommandError> {
    let Some(agent_budget) = agent_budget else {
        return Ok(());
    };
    if (1..=xai_workflow::MAX_AGENT_BUDGET).contains(&agent_budget) {
        return Ok(());
    }
    Err(crate::session::WorkflowCommandError::with_data(
        "workflow_invalid_agent_budget",
        format!(
            "agentBudget must be between 1 and {}",
            xai_workflow::MAX_AGENT_BUDGET
        ),
        serde_json::json!({
            "minimum": 1,
            "maximum": xai_workflow::MAX_AGENT_BUDGET,
            "requested": agent_budget,
        }),
    ))
}

fn validate_budget_limited_resume(
    state: &crate::session::workflow::tracker::WorkflowRunState,
    agent_budget: Option<u64>,
) -> Result<(), crate::session::WorkflowCommandError> {
    let limit = state.agent_budget.unwrap_or(0);
    let Some(requested) = agent_budget else {
        return Err(budget_resume_error(
            state.agents_used,
            limit,
            None,
            "budget-limited runs require an explicitly higher absolute agentBudget",
        ));
    };
    if requested > xai_workflow::MAX_AGENT_BUDGET {
        return Err(budget_resume_error(
            state.agents_used,
            limit,
            Some(requested),
            format!(
                "agentBudget must be at most {}",
                xai_workflow::MAX_AGENT_BUDGET
            ),
        ));
    }
    if requested <= limit || requested <= state.agents_used {
        return Err(budget_resume_error(
            state.agents_used,
            limit,
            Some(requested),
            format!(
                "agentBudget must be higher than both the current limit ({limit}) and agents used ({})",
                state.agents_used
            ),
        ));
    }
    Ok(())
}

fn budget_resume_error(
    used: u64,
    limit: u64,
    requested: Option<u64>,
    message: impl Into<String>,
) -> crate::session::WorkflowCommandError {
    crate::session::WorkflowCommandError::with_data(
        "workflow_budget_not_raised",
        message,
        serde_json::json!({
            "used": used,
            "limit": limit,
            "requested": requested,
            "maximum": xai_workflow::MAX_AGENT_BUDGET,
        }),
    )
}

fn validate_control_operation(
    state: &crate::session::workflow::tracker::WorkflowRunState,
    operation: crate::session::WorkflowControlOperation,
) -> Result<(), crate::session::WorkflowCommandError> {
    use crate::session::workflow::tracker::WorkflowRunStatus;
    let valid = match operation {
        crate::session::WorkflowControlOperation::Pause => {
            state.status == WorkflowRunStatus::Active
        }
        crate::session::WorkflowControlOperation::Resume => state.status.is_resumable(),
        crate::session::WorkflowControlOperation::Stop => !state.status.is_terminal(),
    };
    if valid {
        Ok(())
    } else {
        let operation = match operation {
            crate::session::WorkflowControlOperation::Pause => "pause",
            crate::session::WorkflowControlOperation::Resume => "resume",
            crate::session::WorkflowControlOperation::Stop => "stop",
        };
        Err(invalid_control_state(operation, state))
    }
}

fn invalid_control_state(
    operation: &str,
    state: &crate::session::workflow::tracker::WorkflowRunState,
) -> crate::session::WorkflowCommandError {
    crate::session::WorkflowCommandError::with_data(
        "workflow_invalid_state",
        format!(
            "cannot {operation} workflow run {} while status is {}",
            state.run_id,
            state.status.as_str()
        ),
        serde_json::json!({
            "runId": state.run_id,
            "operation": operation,
            "status": state.status.as_str(),
        }),
    )
}

fn map_resolve_error(
    error: crate::session::workflow::registry::ResolveError,
) -> crate::session::WorkflowCommandError {
    crate::session::WorkflowCommandError::new("workflow_resolve_failed", error.to_string())
}

fn map_launch_error(
    error: crate::session::workflow::manager::LaunchError,
) -> crate::session::WorkflowCommandError {
    match error {
        crate::session::workflow::manager::LaunchError::UnknownRun(run_id) => {
            crate::session::WorkflowCommandError::new(
                "workflow_unknown_run",
                format!("workflow run not found: {run_id}"),
            )
        }
        crate::session::workflow::manager::LaunchError::NotResumable(status) => {
            crate::session::WorkflowCommandError::with_data(
                "workflow_invalid_state",
                format!("workflow run is not resumable while status is {status}"),
                serde_json::json!({ "status": status }),
            )
        }
        crate::session::workflow::manager::LaunchError::BudgetNotRaised { used, limit } => {
            budget_resume_error(
                used,
                limit,
                None,
                "agentBudget was not raised enough to resume",
            )
        }
        other => {
            crate::session::WorkflowCommandError::new("workflow_launch_failed", other.to_string())
        }
    }
}

fn workflow_snapshot_payloads(
    tracker: &crate::session::workflow::tracker::WorkflowTracker,
) -> Result<Vec<serde_json::Value>, crate::session::WorkflowCommandError> {
    tracker
        .list()
        .iter()
        .map(|state| workflow_update_payload(state, tracker.elapsed_ms(&state.run_id)))
        .collect()
}

fn workflow_run_payload(
    tracker: &Arc<parking_lot::Mutex<crate::session::workflow::tracker::WorkflowTracker>>,
    run_id: &str,
) -> Result<serde_json::Value, crate::session::WorkflowCommandError> {
    let tracker = tracker.lock();
    let state = tracker.get(run_id).ok_or_else(|| {
        crate::session::WorkflowCommandError::new(
            "workflow_unknown_run",
            format!("workflow run not found: {run_id}"),
        )
    })?;
    workflow_update_payload(&state, tracker.elapsed_ms(run_id))
}

/// Structured responses omit only the live notification's `sessionUpdate`
/// discriminator; every remaining field comes from `build_workflow_updated`.
fn workflow_update_payload(
    state: &crate::session::workflow::tracker::WorkflowRunState,
    elapsed_ms: u64,
) -> Result<serde_json::Value, crate::session::WorkflowCommandError> {
    let update = crate::session::workflow::notify::build_workflow_updated(state, elapsed_ms, 0);
    let mut value = serde_json::to_value(update).map_err(|error| {
        crate::session::WorkflowCommandError::new(
            "workflow_snapshot_failed",
            format!("failed to serialize workflow update: {error}"),
        )
    })?;
    let object = value.as_object_mut().ok_or_else(|| {
        crate::session::WorkflowCommandError::new(
            "workflow_snapshot_failed",
            "workflow update did not serialize as an object",
        )
    })?;
    match object.remove("sessionUpdate") {
        Some(serde_json::Value::String(kind)) if kind == "workflow_updated" => Ok(value),
        _ => Err(crate::session::WorkflowCommandError::new(
            "workflow_snapshot_failed",
            "workflow update serialized with an unexpected discriminator",
        )),
    }
}

pub(crate) fn parse_named_workflow_args(
    input: &str,
    description: &str,
) -> (serde_json::Value, String) {
    let input = input.trim();
    if input.is_empty() {
        return (serde_json::Value::Null, description.to_string());
    }
    if let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(input) {
        let objective = map
            .get("objective")
            .or_else(|| map.get("query"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| input.to_string());
        return (serde_json::Value::Object(map), objective);
    }
    (
        serde_json::json!({ "query": input, "objective": input }),
        input.to_string(),
    )
}

type RunMatch = (
    String,
    crate::session::workflow::tracker::WorkflowRunStatus,
    String,
);

fn narrow_run_matches(mut all: Vec<RunMatch>, selector: &str, op: &str) -> Vec<RunMatch> {
    use crate::session::workflow::tracker::WorkflowRunStatus;
    if !selector.is_empty() {
        let exact: Vec<_> = all
            .iter()
            .filter(|(id, _, name)| id.as_str() == selector || name.as_str() == selector)
            .cloned()
            .collect();
        if !exact.is_empty() {
            all = exact;
        }
    }
    if all.len() > 1 {
        let applicable: Vec<_> = all
            .iter()
            .filter(|(_, status, ..)| match op {
                "pause" => *status == WorkflowRunStatus::Active,
                "resume" => status.is_resumable(),
                "stop" => !status.is_terminal(),
                _ => true,
            })
            .cloned()
            .collect();
        if applicable.len() == 1 {
            return applicable;
        }
    }
    all
}

#[cfg(test)]
mod structured_workflow_tests {
    use super::*;
    use crate::session::workflow::tracker::{WorkflowAgentRow, WorkflowRunStatus, WorkflowTracker};
    use xai_workflow::{PhaseMeta, WorkflowOutcome};

    fn start_run(
        tracker: &mut WorkflowTracker,
        run_id: &str,
        budget: Option<u64>,
    ) -> crate::session::workflow::tracker::WorkflowRunState {
        tracker.start_run(
            run_id.into(),
            "fixture".into(),
            "fixture objective".into(),
            vec![
                PhaseMeta {
                    title: "Plan".into(),
                    detail: None,
                },
                PhaseMeta {
                    title: "Execute".into(),
                    detail: None,
                },
            ],
            budget,
            None,
        )
    }

    #[test]
    fn snapshot_uses_exact_workflow_updated_payload_without_discriminator() {
        let mut tracker = WorkflowTracker::default();
        start_run(&mut tracker, "wf_snapshot", Some(5));
        tracker.set_phase("wf_snapshot", "Plan");
        tracker.reserve_agents("wf_snapshot", 1).unwrap();
        tracker.agent_started(
            "wf_snapshot",
            WorkflowAgentRow {
                agent_id: "agent-1".into(),
                label: "planner".into(),
                phase: Some("Plan".into()),
                model: None,
                state: "running".into(),
                tokens_used: 0,
                duration_ms: 0,
            },
        );
        std::thread::sleep(std::time::Duration::from_millis(2));

        let runs = workflow_snapshot_payloads(&tracker).unwrap();
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert!(run.get("sessionUpdate").is_none());
        let keys: std::collections::BTreeSet<&str> = run
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            std::collections::BTreeSet::from([
                "active_agents",
                "agent_budget",
                "agent_usage_incomplete",
                "agents",
                "agents_remaining",
                "agents_reserved",
                "agents_used",
                "current_agent_label",
                "current_phase",
                "elapsed_ms",
                "foreground",
                "last_event",
                "last_event_detail",
                "last_event_timestamp",
                "name",
                "objective",
                "phases",
                "revision",
                "run_id",
                "status",
            ])
        );
        assert_eq!(run["run_id"], "wf_snapshot");
        assert_eq!(run["status"], "active");
        assert_eq!(run["current_phase"], "Plan");
        assert_eq!(run["agents_used"], 1);
        assert_eq!(run["agents_remaining"], 4);
        assert_eq!(run["active_agents"], 1);
        assert_eq!(run["current_agent_label"], "planner");
        assert!(run["elapsed_ms"].as_u64().unwrap() > 0);
        assert_eq!(run["phases"][0]["state"], "active");
        assert_eq!(run["phases"][1]["state"], "pending");

        let state = tracker.get("wf_snapshot").unwrap();
        let mut live =
            serde_json::to_value(crate::session::workflow::notify::build_workflow_updated(
                &state,
                run["elapsed_ms"].as_u64().unwrap(),
                0,
            ))
            .unwrap();
        live.as_object_mut().unwrap().remove("sessionUpdate");
        assert_eq!(*run, live);
    }

    #[test]
    fn launch_objective_uses_string_field_otherwise_description() {
        assert_eq!(
            structured_workflow_objective(
                &serde_json::json!({ "objective": "target" }),
                "description"
            ),
            "target"
        );
        assert_eq!(
            structured_workflow_objective(&serde_json::json!({ "objective": 42 }), "description"),
            "description"
        );
        assert_eq!(
            structured_workflow_objective(&serde_json::Value::Null, "description"),
            "description"
        );
    }

    #[test]
    fn invalid_and_restored_stale_control_states_are_rejected() {
        let mut tracker = WorkflowTracker::default();
        let active = start_run(&mut tracker, "wf_active", Some(16));
        assert!(
            validate_control_operation(&active, crate::session::WorkflowControlOperation::Pause)
                .is_ok()
        );
        assert!(
            validate_control_operation(&active, crate::session::WorkflowControlOperation::Stop)
                .is_ok()
        );
        assert_eq!(
            validate_control_operation(&active, crate::session::WorkflowControlOperation::Resume)
                .unwrap_err()
                .code,
            "workflow_invalid_state"
        );

        let stale = WorkflowTracker::from_snapshot(vec![active])
            .get("wf_active")
            .unwrap();
        assert_eq!(stale.status, WorkflowRunStatus::Interrupted);
        for operation in [
            crate::session::WorkflowControlOperation::Pause,
            crate::session::WorkflowControlOperation::Resume,
            crate::session::WorkflowControlOperation::Stop,
        ] {
            assert_eq!(
                validate_control_operation(&stale, operation)
                    .unwrap_err()
                    .code,
                "workflow_invalid_state"
            );
        }
    }

    #[tokio::test]
    async fn registered_named_launch_uses_objective_and_returns_display_name() {
        let (manager, tracker) = crate::session::workflow::manager::WorkflowManager::test_bundle();
        let launched = launch_registered_workflow(
            &manager,
            std::path::Path::new("."),
            "deep-research".into(),
            serde_json::json!({ "objective": "find the regression" }),
            Some(8),
        )
        .await
        .unwrap();
        assert!(launched.run_id.starts_with("wf_"));
        assert_eq!(launched.name, "deep-research");
        let state = tracker.lock().get(&launched.run_id).unwrap();
        assert_eq!(state.objective, "find the regression");
        assert_eq!(state.agent_budget, Some(8));
        let _ = manager.lock().await.cancel(&launched.run_id);
    }

    #[tokio::test]
    async fn pause_and_stop_apply_immediate_states() {
        let (manager, tracker) = crate::session::workflow::manager::WorkflowManager::test_bundle();
        start_run(&mut tracker.lock(), "wf_pause", Some(16));
        let (_done_tx, done_rx) = tokio::sync::oneshot::channel();
        manager
            .lock()
            .await
            .test_insert_active_run("wf_pause".into(), done_rx);
        assert!(manager.lock().await.pause("wf_pause"));
        assert_eq!(
            tracker.lock().get("wf_pause").unwrap().status,
            WorkflowRunStatus::UserPaused
        );

        start_run(&mut tracker.lock(), "wf_stop", Some(16));
        assert!(manager.lock().await.cancel("wf_stop"));
        assert_eq!(
            tracker.lock().get("wf_stop").unwrap().status,
            WorkflowRunStatus::Cancelled
        );
    }

    #[test]
    fn budget_limited_resume_requires_explicit_higher_bounded_cap() {
        let mut tracker = WorkflowTracker::default();
        start_run(&mut tracker, "wf_budget", Some(128));
        tracker.reserve_agents("wf_budget", 128).unwrap();
        tracker.apply_outcome(
            "wf_budget",
            &WorkflowOutcome::BudgetExceeded {
                message: "spent".into(),
            },
        );
        let state = tracker.get("wf_budget").unwrap();
        assert_eq!(state.status, WorkflowRunStatus::BudgetLimited);

        for requested in [None, Some(128), Some(1_025)] {
            let error = validate_budget_limited_resume(&state, requested).unwrap_err();
            assert_eq!(error.code, "workflow_budget_not_raised");
            let data = error.data.unwrap();
            assert_eq!(data["used"], 128);
            assert_eq!(data["limit"], 128);
            assert_eq!(data["maximum"], 1_024);
        }
        validate_budget_limited_resume(&state, Some(256)).unwrap();
    }
}

#[cfg(test)]
mod run_match_tests {
    use super::narrow_run_matches;
    use crate::session::workflow::tracker::WorkflowRunStatus;

    fn run(id: &str, name: &str, status: WorkflowRunStatus) -> super::RunMatch {
        (id.to_string(), status, name.to_string())
    }

    #[test]
    fn exact_name_beats_prefix_of_uniquified_sibling() {
        let all = vec![
            run("wf_1", "deep-research", WorkflowRunStatus::Active),
            run("wf_2", "deep-research-2", WorkflowRunStatus::Active),
        ];
        let picked = narrow_run_matches(all, "deep-research", "stop");
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].2, "deep-research");
    }

    #[test]
    fn prefix_still_narrows_by_op_applicability() {
        let all = vec![
            run("wf_1", "deep-research", WorkflowRunStatus::Complete),
            run("wf_2", "deep-research-2", WorkflowRunStatus::Active),
        ];
        let picked = narrow_run_matches(all, "deep", "stop");
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].2, "deep-research-2");
    }

    #[test]
    fn empty_selector_with_single_applicable_run_resolves() {
        let all = vec![
            run("wf_1", "a", WorkflowRunStatus::Complete),
            run("wf_2", "b", WorkflowRunStatus::UserPaused),
        ];
        let picked = narrow_run_matches(all, "", "resume");
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].2, "b");
    }

    #[test]
    fn failed_run_is_applicable_for_resume_narrowing() {
        let all = vec![
            run("wf_1", "a", WorkflowRunStatus::Complete),
            run("wf_2", "b", WorkflowRunStatus::Failed),
        ];
        let picked = narrow_run_matches(all, "", "resume");
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].2, "b");
    }

    #[test]
    fn ambiguous_stays_ambiguous() {
        let all = vec![
            run("wf_1", "a", WorkflowRunStatus::Active),
            run("wf_2", "b", WorkflowRunStatus::Active),
        ];
        assert_eq!(narrow_run_matches(all, "", "stop").len(), 2);
    }
}
