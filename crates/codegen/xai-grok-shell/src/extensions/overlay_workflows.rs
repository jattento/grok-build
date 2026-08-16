use agent_client_protocol as acp;
use overlay_workflow_control::{
    WorkflowControlOperationRequest, WorkflowControlRequest, WorkflowControlResponse,
    WorkflowLaunchRequest, WorkflowLaunchResponse, WorkflowMethod, WorkflowSnapshotRequest,
    WorkflowSnapshotResponse, WorkflowsListRequest, parse_method,
};

use super::{ExtResult, parse_params, to_raw_response};
use crate::agent::mvp_agent::MvpAgent;
use crate::session::{WorkflowCommandError, WorkflowControlOperation};

impl From<WorkflowControlOperationRequest> for WorkflowControlOperation {
    fn from(value: WorkflowControlOperationRequest) -> Self {
        match value {
            WorkflowControlOperationRequest::Pause => Self::Pause,
            WorkflowControlOperationRequest::Resume => Self::Resume,
            WorkflowControlOperationRequest::Stop => Self::Stop,
        }
    }
}

pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    match parse_method(args.method.as_ref()) {
        Some(WorkflowMethod::List) => handle_list(agent, args).await,
        Some(WorkflowMethod::Launch) => handle_launch(agent, args).await,
        Some(WorkflowMethod::Snapshot) => handle_snapshot(agent, args).await,
        Some(WorkflowMethod::Control) => handle_control(agent, args).await,
        None => Err(acp::Error::method_not_found()),
    }
}

async fn handle_list(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let req: WorkflowsListRequest = serde_json::from_str(args.params.get())?;
    let session_id = acp::SessionId::new(req.session_id.as_str());
    let Some(handle) = agent.session_handle_waiting_for_load(&session_id).await else {
        return super::to_ext_response(Err::<serde_json::Value, _>(anyhow::anyhow!(
            "unknown session id: {}",
            req.session_id
        )));
    };
    let (launches_enabled, _management_available) = handle.workflow_catalog_state().await;
    let workflows = if launches_enabled {
        crate::session::workflow::registry::list_workflows(Some(handle.tool_context.cwd.as_path()))
    } else {
        Vec::new()
    };
    // Keep the ExtMethodResult envelope used by the shipped list path.
    super::to_ext_response(Ok(serde_json::json!({ "workflows": workflows })))
}

async fn handle_launch(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let req: WorkflowLaunchRequest = parse_params(args)?;
    let session_id = acp::SessionId::new(req.session_id.as_str());
    let handle = resident_session(agent, &session_id)?;
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
    let session_id = acp::SessionId::new(req.session_id.as_str());
    let handle = resident_session(agent, &session_id)?;
    let runs = handle.workflow_snapshot().await.map_err(workflow_error)?;
    to_raw_response(&WorkflowSnapshotResponse { runs })
}

async fn handle_control(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let req: WorkflowControlRequest = parse_params(args)?;
    let session_id = acp::SessionId::new(req.session_id.as_str());
    let handle = resident_session(agent, &session_id)?;
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
