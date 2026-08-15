use agent_client_protocol as acp;
use serde::{Deserialize, Serialize};

use super::{ExtResult, parse_params, to_raw_response};
use crate::agent::mvp_agent::MvpAgent;
use crate::session::{WorkflowCommandError, WorkflowControlOperation};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowsListRequest {
    session_id: acp::SessionId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowLaunchRequest {
    session_id: acp::SessionId,
    name: String,
    #[serde(default)]
    args: Option<serde_json::Value>,
    #[serde(default)]
    agent_budget: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowLaunchResponse {
    run_id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowSnapshotRequest {
    session_id: acp::SessionId,
}

#[derive(Debug, Serialize)]
struct WorkflowSnapshotResponse {
    runs: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum WorkflowControlOperationRequest {
    Pause,
    Resume,
    Stop,
}

impl From<WorkflowControlOperationRequest> for WorkflowControlOperation {
    fn from(value: WorkflowControlOperationRequest) -> Self {
        match value {
            WorkflowControlOperationRequest::Pause => Self::Pause,
            WorkflowControlOperationRequest::Resume => Self::Resume,
            WorkflowControlOperationRequest::Stop => Self::Stop,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowControlRequest {
    session_id: acp::SessionId,
    run_id: String,
    operation: WorkflowControlOperationRequest,
    #[serde(default)]
    agent_budget: Option<u64>,
}

#[derive(Debug, Serialize)]
struct WorkflowControlResponse {
    run: serde_json::Value,
}

pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    match args.method.as_ref() {
        "x.ai/workflows/list" => handle_list(agent, args).await,
        "x.ai/workflows/launch" => handle_launch(agent, args).await,
        "x.ai/workflows/snapshot" => handle_snapshot(agent, args).await,
        "x.ai/workflows/control" => handle_control(agent, args).await,
        _ => Err(acp::Error::method_not_found()),
    }
}

async fn handle_list(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let req: WorkflowsListRequest = serde_json::from_str(args.params.get())?;
    let Some(handle) = agent.session_handle_waiting_for_load(&req.session_id).await else {
        return super::to_ext_response(Err::<serde_json::Value, _>(anyhow::anyhow!(
            "unknown session id: {}",
            req.session_id.0
        )));
    };
    let (launches_enabled, _management_available) = handle.workflow_catalog_state().await;
    let workflows = if launches_enabled {
        crate::session::workflow::registry::list_workflows(Some(handle.tool_context.cwd.as_path()))
    } else {
        Vec::new()
    };
    super::to_ext_response(Ok(serde_json::json!({ "workflows": workflows })))
}

async fn handle_launch(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let req: WorkflowLaunchRequest = parse_params(args)?;
    let handle = resident_session(agent, &req.session_id)?;
    let launched = handle
        .launch_named_workflow_structured(
            req.name,
            req.args.unwrap_or(serde_json::Value::Null),
            req.agent_budget,
        )
        .await
        .map_err(workflow_error)?;
    to_raw_response(&WorkflowLaunchResponse {
        run_id: launched.run_id,
        name: launched.name,
    })
}

async fn handle_snapshot(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let req: WorkflowSnapshotRequest = parse_params(args)?;
    let handle = resident_session(agent, &req.session_id)?;
    let runs = handle.workflow_snapshot().await.map_err(workflow_error)?;
    to_raw_response(&WorkflowSnapshotResponse { runs })
}

async fn handle_control(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let req: WorkflowControlRequest = parse_params(args)?;
    let handle = resident_session(agent, &req.session_id)?;
    let run = handle
        .control_workflow(req.run_id, req.operation.into(), req.agent_budget)
        .await
        .map_err(workflow_error)?;
    to_raw_response(&WorkflowControlResponse { run })
}

fn resident_session(
    agent: &MvpAgent,
    session_id: &acp::SessionId,
) -> Result<crate::session::SessionHandle, acp::Error> {
    agent.resident_handle(session_id).ok_or_else(|| {
        workflow_error(WorkflowCommandError::new(
            "workflow_session_not_resident",
            format!("session is not resident: {}", session_id.0),
        ))
    })
}

fn workflow_error(error: WorkflowCommandError) -> acp::Error {
    let mut data = serde_json::Map::from_iter([
        (
            "code".to_string(),
            serde_json::Value::String(error.code.to_string()),
        ),
        (
            "message".to_string(),
            serde_json::Value::String(error.message),
        ),
    ]);
    if let Some(extra) = error.data {
        data.insert("data".to_string(), extra);
    }
    acp::Error::invalid_request().data(serde_json::Value::Object(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_deserialize_camel_case_contract() {
        let launch: WorkflowLaunchRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "session-1",
            "name": "deep-research",
            "args": { "objective": "trace the regression" },
            "agentBudget": 42
        }))
        .unwrap();
        assert_eq!(launch.session_id.0.as_ref(), "session-1");
        assert_eq!(launch.name, "deep-research");
        assert_eq!(launch.args.unwrap()["objective"], "trace the regression");
        assert_eq!(launch.agent_budget, Some(42));

        let snapshot: WorkflowSnapshotRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "session-2"
        }))
        .unwrap();
        assert_eq!(snapshot.session_id.0.as_ref(), "session-2");

        let control: WorkflowControlRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "session-3",
            "runId": "wf_123",
            "operation": "resume",
            "agentBudget": 256
        }))
        .unwrap();
        assert_eq!(control.session_id.0.as_ref(), "session-3");
        assert_eq!(control.run_id, "wf_123");
        assert_eq!(control.operation, WorkflowControlOperationRequest::Resume);
        assert_eq!(control.agent_budget, Some(256));
    }

    #[test]
    fn responses_serialize_exact_contract() {
        assert_eq!(
            serde_json::to_value(WorkflowLaunchResponse {
                run_id: "wf_123".into(),
                name: "deep-research".into(),
            })
            .unwrap(),
            serde_json::json!({
                "runId": "wf_123",
                "name": "deep-research"
            })
        );
        assert_eq!(
            serde_json::to_value(WorkflowSnapshotResponse {
                runs: vec![serde_json::json!({ "run_id": "wf_123" })],
            })
            .unwrap(),
            serde_json::json!({
                "runs": [{ "run_id": "wf_123" }]
            })
        );
        assert_eq!(
            serde_json::to_value(WorkflowControlResponse {
                run: serde_json::json!({ "run_id": "wf_123" }),
            })
            .unwrap(),
            serde_json::json!({
                "run": { "run_id": "wf_123" }
            })
        );
    }

    #[test]
    fn control_rejects_unknown_operation_during_deserialization() {
        let error = serde_json::from_value::<WorkflowControlRequest>(serde_json::json!({
            "sessionId": "session-3",
            "runId": "wf_123",
            "operation": "restart"
        }))
        .unwrap_err();
        assert!(error.to_string().contains("unknown variant"));
    }

    #[test]
    fn workflow_error_preserves_stable_code_and_structured_budget_data() {
        let error = workflow_error(WorkflowCommandError::with_data(
            "workflow_budget_not_raised",
            "raise agentBudget",
            serde_json::json!({ "used": 128, "limit": 128 }),
        ));
        let data = error.data.unwrap();
        assert_eq!(data["code"], "workflow_budget_not_raised");
        assert_eq!(data["message"], "raise agentBudget");
        assert_eq!(data["data"]["used"], 128);
        assert_eq!(data["data"]["limit"], 128);
    }
}
