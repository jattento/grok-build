//! Wire contract for structured workflow ACP methods.
//!
//! These payloads are a shipped contract consumed by Conan Code. Field names,
//! casing, optionality, and enum string values must stay byte-identical on the
//! wire. Request/response DTOs use camelCase; run-state payloads embedded as
//! opaque JSON keep snake_case.

use serde::{Deserialize, Serialize};

/// ACP method names for the structured workflow surface.
pub const METHOD_LIST: &str = "x.ai/workflows/list";
pub const METHOD_LAUNCH: &str = "x.ai/workflows/launch";
pub const METHOD_SNAPSHOT: &str = "x.ai/workflows/snapshot";
pub const METHOD_CONTROL: &str = "x.ai/workflows/control";

/// Parsed structured-workflow ACP method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowMethod {
    List,
    Launch,
    Snapshot,
    Control,
}

/// Map an ACP method string to a structured workflow operation.
pub fn parse_method(method: &str) -> Option<WorkflowMethod> {
    match method {
        METHOD_LIST => Some(WorkflowMethod::List),
        METHOD_LAUNCH => Some(WorkflowMethod::Launch),
        METHOD_SNAPSHOT => Some(WorkflowMethod::Snapshot),
        METHOD_CONTROL => Some(WorkflowMethod::Control),
        _ => None,
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowsListRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct WorkflowsListResponse {
    pub workflows: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowLaunchRequest {
    pub session_id: String,
    pub name: String,
    #[serde(default)]
    pub args: Option<serde_json::Value>,
    #[serde(default)]
    pub agent_budget: Option<u64>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowLaunchResponse {
    pub run_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSnapshotRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct WorkflowSnapshotResponse {
    pub runs: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowControlOperationRequest {
    Pause,
    Resume,
    Stop,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowControlRequest {
    pub session_id: String,
    pub run_id: String,
    pub operation: WorkflowControlOperationRequest,
    #[serde(default)]
    pub agent_budget: Option<u64>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct WorkflowControlResponse {
    pub run: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_method_accepts_exact_workflow_methods() {
        assert_eq!(parse_method(METHOD_LIST), Some(WorkflowMethod::List));
        assert_eq!(parse_method(METHOD_LAUNCH), Some(WorkflowMethod::Launch));
        assert_eq!(
            parse_method(METHOD_SNAPSHOT),
            Some(WorkflowMethod::Snapshot)
        );
        assert_eq!(parse_method(METHOD_CONTROL), Some(WorkflowMethod::Control));
        assert_eq!(parse_method("x.ai/workflows/unknown"), None);
        assert_eq!(parse_method("x.ai/skills/list"), None);
    }

    #[test]
    fn requests_deserialize_camel_case_contract() {
        let list: WorkflowsListRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "session-0"
        }))
        .unwrap();
        assert_eq!(list.session_id, "session-0");

        let launch: WorkflowLaunchRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "session-1",
            "name": "deep-research",
            "args": { "objective": "trace the regression" },
            "agentBudget": 42
        }))
        .unwrap();
        assert_eq!(launch.session_id, "session-1");
        assert_eq!(launch.name, "deep-research");
        assert_eq!(launch.args.unwrap()["objective"], "trace the regression");
        assert_eq!(launch.agent_budget, Some(42));

        let snapshot: WorkflowSnapshotRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "session-2"
        }))
        .unwrap();
        assert_eq!(snapshot.session_id, "session-2");

        let control: WorkflowControlRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "session-3",
            "runId": "wf_123",
            "operation": "resume",
            "agentBudget": 256
        }))
        .unwrap();
        assert_eq!(control.session_id, "session-3");
        assert_eq!(control.run_id, "wf_123");
        assert_eq!(control.operation, WorkflowControlOperationRequest::Resume);
        assert_eq!(control.agent_budget, Some(256));
    }

    #[test]
    fn launch_request_optional_fields_default_when_absent() {
        let launch: WorkflowLaunchRequest = serde_json::from_value(serde_json::json!({
            "sessionId": "session-1",
            "name": "deep-research"
        }))
        .unwrap();
        assert_eq!(launch.args, None);
        assert_eq!(launch.agent_budget, None);

        // Re-serialize must not invent optional keys.
        let raw = serde_json::to_value(&serde_json::json!({
            "sessionId": launch.session_id,
            "name": launch.name
        }))
        .unwrap();
        assert!(raw.get("args").is_none());
        assert!(raw.get("agentBudget").is_none());
    }

    #[test]
    fn responses_serialize_exact_contract() {
        assert_eq!(
            serde_json::to_value(WorkflowsListResponse {
                workflows: vec![serde_json::json!({
                    "name": "deep-research",
                    "description": "trace regressions"
                })],
            })
            .unwrap(),
            serde_json::json!({
                "workflows": [{
                    "name": "deep-research",
                    "description": "trace regressions"
                }]
            })
        );
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
        // Snapshot/control envelopes are camelCase-free; embedded run state is
        // snake_case and must not be renamed by the envelope serializers.
        assert_eq!(
            serde_json::to_value(WorkflowSnapshotResponse {
                runs: vec![serde_json::json!({
                    "run_id": "wf_123",
                    "display_name": "deep-research",
                    "status": "running",
                    "elapsed_ms": 12
                })],
            })
            .unwrap(),
            serde_json::json!({
                "runs": [{
                    "run_id": "wf_123",
                    "display_name": "deep-research",
                    "status": "running",
                    "elapsed_ms": 12
                }]
            })
        );
        assert_eq!(
            serde_json::to_value(WorkflowControlResponse {
                run: serde_json::json!({
                    "run_id": "wf_123",
                    "status": "paused",
                    "elapsed_ms": 40
                }),
            })
            .unwrap(),
            serde_json::json!({
                "run": {
                    "run_id": "wf_123",
                    "status": "paused",
                    "elapsed_ms": 40
                }
            })
        );
    }

    #[test]
    fn control_operation_wire_values_are_lowercase() {
        for (op, wire) in [
            (WorkflowControlOperationRequest::Pause, "pause"),
            (WorkflowControlOperationRequest::Resume, "resume"),
            (WorkflowControlOperationRequest::Stop, "stop"),
        ] {
            assert_eq!(serde_json::to_value(op).unwrap(), serde_json::json!(wire));
            let parsed: WorkflowControlOperationRequest =
                serde_json::from_value(serde_json::json!(wire)).unwrap();
            assert_eq!(parsed, op);
        }
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
    fn request_field_names_reject_snake_case() {
        // Clients send camelCase; snake_case must not silently work.
        assert!(
            serde_json::from_value::<WorkflowLaunchRequest>(serde_json::json!({
                "session_id": "session-1",
                "name": "deep-research",
                "agent_budget": 1
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<WorkflowControlRequest>(serde_json::json!({
                "session_id": "session-3",
                "run_id": "wf_123",
                "operation": "pause"
            }))
            .is_err()
        );
    }

    #[test]
    fn launch_response_round_trips_camel_case_fields() {
        let original = WorkflowLaunchResponse {
            run_id: "wf_abc".into(),
            name: "review-changes".into(),
        };
        let value = serde_json::to_value(&original).unwrap();
        let obj = value.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert_eq!(obj.get("runId").and_then(|v| v.as_str()), Some("wf_abc"));
        assert_eq!(
            obj.get("name").and_then(|v| v.as_str()),
            Some("review-changes")
        );
        assert!(!obj.contains_key("run_id"));
        // Round-trip through raw JSON to prove field names survive the wire.
        let raw = serde_json::to_string(&value).unwrap();
        assert!(raw.contains("\"runId\""));
        assert!(!raw.contains("\"run_id\""));
        let back: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(back, value);
    }
}
