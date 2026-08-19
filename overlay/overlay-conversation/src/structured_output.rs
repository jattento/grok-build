//! Route JSON-schema requests through the same Messages-backend path the
//! agent turn loop already uses (`StructuredOutput` tool), instead of the
//! native `json_schema` field that cliproxy/DeepSeek drop.

use overlay_core::goal_eval;
use xai_grok_sampling_types::{
    ApiBackend, ConversationItem, ConversationRequest, ConversationResponse,
    ConversationToolChoice, ToolSpec,
};

/// Same synthetic tool name the agent turn loop advertises on Messages.
pub const STRUCTURED_OUTPUT_TOOL: &str = "StructuredOutput";

const TOOL_REMINDER: &str = "A response schema is required. Call the `StructuredOutput` tool \
exactly once with your final answer as its arguments; do not return the \
answer as text.";

/// Rewrite `request` so `backend` can actually return structured output.
///
/// Chat Completions / Responses keep `json_schema` (native constrained
/// decoding). Messages moves the schema onto the `StructuredOutput` tool and
/// drops the wire field — a schema there is ignored by cliproxy and
/// suppresses tool use on Anthropic.
pub fn prepare_structured_output(request: &mut ConversationRequest, backend: ApiBackend) {
    if backend.supports_native_schema() {
        return;
    }
    let Some(schema) = request.json_schema.take() else {
        return;
    };
    request.tools.push(ToolSpec {
        name: STRUCTURED_OUTPUT_TOOL.to_string(),
        description: Some(
            "Return your final answer as JSON matching the required schema. \
             Call this exactly once, at the end."
                .to_string(),
        ),
        parameters: schema,
    });
    request.tool_choice = Some(ConversationToolChoice::Function(
        STRUCTURED_OUTPUT_TOOL.to_string(),
    ));
    request.items.push(ConversationItem::system(TOOL_REMINDER));
}

/// Best payload for a schema-constrained judge: StructuredOutput tool args,
/// else the first JSON object in assistant text, else the raw text.
pub fn structured_output_text(response: &ConversationResponse) -> String {
    if let Some(call) = response
        .tool_calls()
        .iter()
        .find(|call| call.name == STRUCTURED_OUTPUT_TOOL)
    {
        return call.arguments.as_ref().to_owned();
    }
    let text = response.assistant_text();
    goal_eval::extract_json_object(&text)
        .map(str::to_owned)
        .unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_sampling_types::{AssistantItem, ConversationItem, ToolCall};

    fn schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": { "decision": { "type": "string" } },
            "required": ["decision"]
        })
    }

    fn request_with_schema() -> ConversationRequest {
        ConversationRequest {
            items: vec![
                ConversationItem::system("judge"),
                ConversationItem::user("go"),
            ],
            json_schema: Some(schema()),
            ..ConversationRequest::default()
        }
    }

    #[test]
    fn native_backend_keeps_json_schema_and_no_tool() {
        let mut request = request_with_schema();
        prepare_structured_output(&mut request, ApiBackend::ChatCompletions);
        assert!(request.json_schema.is_some());
        assert!(request.tools.is_empty());
        assert!(request.tool_choice.is_none());
    }

    #[test]
    fn messages_backend_moves_schema_onto_structured_output_tool() {
        let mut request = request_with_schema();
        prepare_structured_output(&mut request, ApiBackend::Messages);
        assert!(request.json_schema.is_none());
        assert_eq!(request.tools.len(), 1);
        assert_eq!(request.tools[0].name, STRUCTURED_OUTPUT_TOOL);
        assert_eq!(request.tools[0].parameters, schema());
        assert!(matches!(
            request.tool_choice,
            Some(ConversationToolChoice::Function(ref name)) if name == STRUCTURED_OUTPUT_TOOL
        ));
        assert!(
            request
                .items
                .iter()
                .any(|item| matches!(item, ConversationItem::System(s) if s.content.contains("StructuredOutput")))
        );
    }

    #[test]
    fn structured_output_text_prefers_tool_args() {
        let response = ConversationResponse {
            items: vec![ConversationItem::Assistant(AssistantItem {
                content: "<review>Doing.".into(),
                tool_calls: vec![ToolCall {
                    id: "1".into(),
                    name: STRUCTURED_OUTPUT_TOOL.to_string(),
                    arguments: r#"{"decision":"continue"}"#.into(),
                }],
                model_id: None,
                model_fingerprint: None,
                reasoning_effort: None,
            })],
            stop_reason: None,
            usage: None,
            cost_usd_ticks: None,
            message_chunks_emitted: 0,
            doom_loop_signals: vec![],
            stop_message: None,
            message_id: None,
            raw_stop_reason: None,
            stop_sequence: None,
        };
        assert_eq!(
            structured_output_text(&response),
            r#"{"decision":"continue"}"#
        );
    }

    #[test]
    fn structured_output_text_extracts_object_from_leaked_content() {
        let inner = r#"{"decision":"continue","evidence":"x","next_step":"y","blocker_key":""}"#;
        let response = ConversationResponse {
            items: vec![ConversationItem::assistant(format!(
                "<review>Doing.\n{inner}\n"
            ))],
            stop_reason: None,
            usage: None,
            cost_usd_ticks: None,
            message_chunks_emitted: 0,
            doom_loop_signals: vec![],
            stop_message: None,
            message_id: None,
            raw_stop_reason: None,
            stop_sequence: None,
        };
        assert_eq!(structured_output_text(&response), inner);
    }
}
