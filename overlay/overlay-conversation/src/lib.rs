//! Bidirectional conversation translation across API backends.
//!
//! Stored history stays in the unified [`ConversationItem`] form, including
//! provider-private reasoning blobs (`encrypted_content` / Anthropic thinking
//! signatures). Each outgoing request is a clone of that history; this crate
//! rewrites only that clone so the *target* backend can accept it:
//!
//! - Anthropic → Grok/OpenAI Responses: drop Anthropic signatures, keep any
//!   plaintext thinking so the new model still sees the prior reasoning.
//! - Grok/OpenAI → Anthropic: drop Responses `encrypted_content` (and the
//!   whole thinking block — Anthropic rejects thinking without its own
//!   signature). User/assistant/tool turns stay intact.
//! - Same-family continue: leave encrypted blobs alone so the prefix cache
//!   stays warm.
//!
//! Origin is inferred from the reasoning item itself: Anthropic's Messages
//! stream stores signatures on items with an empty `id`; the Responses API
//! always stamps a stable id (`rs_*`, `tco_*`, …).

mod structured_output;

pub use structured_output::{
    STRUCTURED_OUTPUT_TOOL, prepare_structured_output, structured_output_text,
};

use xai_grok_sampling_types::{
    ApiBackend, ContentPart, ConversationItem, ConversationRequest, reasoning_item_text, rs,
};

/// What the translator changed on one request. Zeroes mean the outgoing
/// history was already valid for the target backend.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TranslateReport {
    /// Reasoning items whose `encrypted_content` was cleared.
    pub stripped_encrypted: usize,
    /// Reasoning items removed entirely (signature-only after a strip, or
    /// thinking the target backend cannot represent).
    pub dropped_reasoning: usize,
    /// Images replaced with a text placeholder because the target model has
    /// no vision support (see [`substitute_images_for_no_vision_model`]).
    pub substituted_images: usize,
}

impl TranslateReport {
    pub fn changed(&self) -> bool {
        self.stripped_encrypted > 0 || self.dropped_reasoning > 0 || self.substituted_images > 0
    }
}

/// Rewrite `request.items` so `backend` will accept them.
///
/// Does not touch stored session history — callers pass a request clone.
pub fn prepare_request(request: &mut ConversationRequest, backend: ApiBackend) -> TranslateReport {
    // Matches `build_messages_request`: thinking config is only emitted when
    // `to_messages_api()` yields a level (Low and above; None/Minimal stay off).
    let thinking_enabled = request
        .reasoning_effort
        .and_then(xai_grok_sampling_types::ReasoningEffort::to_messages_api)
        .is_some();
    let mut report = prepare_items(&mut request.items, backend.clone(), thinking_enabled);
    if let Some(model) = request.model.as_deref() {
        if !overlay_core::supports_vision(model) {
            report.substituted_images +=
                substitute_images_for_no_vision_model(&mut request.items, model);
        }
    }
    if report.changed() {
        tracing::info!(
            ?backend,
            thinking_enabled,
            stripped_encrypted = report.stripped_encrypted,
            dropped_reasoning = report.dropped_reasoning,
            substituted_images = report.substituted_images,
            "overlay-conversation: translated history for target backend"
        );
    }
    report
}

/// Replaces every image in `items` with a text placeholder for a model that
/// cannot see images, instead of letting it reach the provider and error out
/// (or be silently dropped) after a full round trip. The placeholder tells
/// the agent where the image is so it can act on it explicitly — read or
/// describe it with a tool, delegate to a vision-capable model, or ask the
/// user — rather than losing it without a trace.
///
/// Returns the number of images substituted.
fn substitute_images_for_no_vision_model(items: &mut [ConversationItem], model: &str) -> usize {
    let mut substituted = 0;
    for item in items {
        match item {
            ConversationItem::User(user) => {
                for part in &mut user.content {
                    if let ContentPart::Image { url } = part {
                        *part = ContentPart::Text {
                            text: std::sync::Arc::<str>::from(no_vision_placeholder(url, model)),
                        };
                        substituted += 1;
                    }
                }
            }
            ConversationItem::ToolResult(t) if !t.images.is_empty() => {
                // Unlike a user message, a `ToolResult`'s `images` field is
                // not folded into `content` by the wire-format converters, so
                // a placeholder left there would never reach the model (see
                // `strip_images_where`'s doc comment). Note each image's
                // location in `content`, the field the converters do emit,
                // and drop the field.
                let notes: Vec<String> = t
                    .images
                    .iter()
                    .filter_map(|part| match part {
                        ContentPart::Image { url } => Some(no_vision_placeholder(url, model)),
                        ContentPart::Text { .. } => None,
                    })
                    .collect();
                substituted += notes.len();
                t.images.clear();
                if !notes.is_empty() {
                    let content = format!("{}\n\n{}", t.content, notes.join("\n"));
                    t.content = std::sync::Arc::<str>::from(content);
                }
            }
            ConversationItem::ToolResult(_)
            | ConversationItem::System(_)
            | ConversationItem::Assistant(_)
            | ConversationItem::BackendToolCall(_)
            | ConversationItem::Reasoning(_) => {}
        }
    }
    substituted
}

/// Text substituted for one image when `model` cannot see it. Surfaces the
/// image's location when the URL carries one (the `file://` convention used
/// for placeholder-image attachments); falls back to the raw URL otherwise.
fn no_vision_placeholder(url: &str, model: &str) -> String {
    let location = url.strip_prefix("file://").unwrap_or(url);
    format!(
        "[image attached at {location} — {model} has no vision support and \
         cannot see it directly. Read or describe it with a tool, delegate \
         to a vision-capable model, or ask the user to describe it, as the \
         task requires.]"
    )
}

/// Same rewrite as [`prepare_request`], operating on a bare item list.
pub fn prepare_items(
    items: &mut Vec<ConversationItem>,
    backend: ApiBackend,
    thinking_enabled: bool,
) -> TranslateReport {
    let mut report = TranslateReport::default();
    let mut kept = Vec::with_capacity(items.len());
    for item in items.drain(..) {
        match item {
            ConversationItem::Reasoning(r) => match decide(&backend, thinking_enabled, &r) {
                Action::Keep => kept.push(ConversationItem::Reasoning(r)),
                Action::StripEncrypted => {
                    report.stripped_encrypted += 1;
                    let mut r = r;
                    r.encrypted_content = None;
                    if reasoning_survives(&r) {
                        kept.push(ConversationItem::Reasoning(r));
                    } else {
                        report.dropped_reasoning += 1;
                    }
                }
                Action::Drop => {
                    if has_encrypted(&r) {
                        report.stripped_encrypted += 1;
                    }
                    report.dropped_reasoning += 1;
                }
            },
            other => kept.push(other),
        }
    }
    *items = kept;
    report
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Keep,
    StripEncrypted,
    Drop,
}

fn decide(backend: &ApiBackend, thinking_enabled: bool, r: &rs::ReasoningItem) -> Action {
    let has_sig = has_encrypted(r);
    let responses_id = !r.id.is_empty();
    match backend {
        // Anthropic rejects thinking blocks unless a top-level `thinking`
        // config is set, and rejects thinking whose signature did not come
        // from Anthropic.
        ApiBackend::Messages => {
            if thinking_enabled && has_sig && !responses_id {
                Action::Keep
            } else {
                Action::Drop
            }
        }
        // Responses will 400 on Anthropic signatures (empty id) replayed as
        // `encrypted_content`. Text-only reasoning is accepted.
        ApiBackend::Responses => {
            if responses_id {
                Action::Keep
            } else if has_sig {
                Action::StripEncrypted
            } else {
                Action::Keep
            }
        }
        // Chat Completions never replays `encrypted_content`; it only folds
        // plaintext into `reasoning_content`.
        ApiBackend::ChatCompletions => {
            if has_sig {
                Action::StripEncrypted
            } else {
                Action::Keep
            }
        }
    }
}

fn has_encrypted(r: &rs::ReasoningItem) -> bool {
    r.encrypted_content
        .as_deref()
        .is_some_and(|s| !s.is_empty())
}

fn reasoning_survives(r: &rs::ReasoningItem) -> bool {
    !reasoning_item_text(r).is_empty() || has_encrypted(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use xai_grok_sampling_types::{ConversationItem, ReasoningEffort};

    fn anthropic_thinking(text: &str, signature: &str) -> ConversationItem {
        ConversationItem::Reasoning(rs::ReasoningItem {
            id: String::new(),
            summary: if text.is_empty() {
                vec![]
            } else {
                vec![rs::SummaryPart::SummaryText(rs::SummaryTextContent {
                    text: text.to_string(),
                })]
            },
            content: None,
            encrypted_content: Some(signature.to_string()),
            status: None,
        })
    }

    fn responses_reasoning(id: &str, text: &str, encrypted: Option<&str>) -> ConversationItem {
        ConversationItem::Reasoning(rs::ReasoningItem {
            id: id.to_string(),
            summary: if text.is_empty() {
                vec![]
            } else {
                vec![rs::SummaryPart::SummaryText(rs::SummaryTextContent {
                    text: text.to_string(),
                })]
            },
            content: None,
            encrypted_content: encrypted.map(str::to_string),
            status: None,
        })
    }

    fn encrypted<'a>(items: &'a [ConversationItem]) -> Vec<Option<&'a str>> {
        items
            .iter()
            .filter_map(|item| match item {
                ConversationItem::Reasoning(r) => Some(r.encrypted_content.as_deref()),
                _ => None,
            })
            .collect()
    }

    fn reasoning_texts(items: &[ConversationItem]) -> Vec<String> {
        items
            .iter()
            .filter_map(|item| match item {
                ConversationItem::Reasoning(r) => Some(reasoning_item_text(r)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn anthropic_to_responses_strips_signature_keeps_text() {
        let mut items = vec![
            ConversationItem::user("continue"),
            anthropic_thinking("plan the fix", "anth-sig"),
            ConversationItem::assistant_with_model("ok", "claude-opus-5"),
        ];
        let report = prepare_items(&mut items, ApiBackend::Responses, true);
        assert_eq!(
            report,
            TranslateReport {
                stripped_encrypted: 1,
                dropped_reasoning: 0,
                substituted_images: 0
            }
        );
        assert_eq!(encrypted(&items), vec![None]);
        assert_eq!(reasoning_texts(&items), vec!["plan the fix".to_string()]);
        assert!(matches!(items[0], ConversationItem::User(_)));
        assert!(matches!(items[2], ConversationItem::Assistant(_)));
    }

    #[test]
    fn anthropic_signature_only_dropped_for_responses() {
        let mut items = vec![
            anthropic_thinking("", "anth-sig"),
            ConversationItem::assistant("ok"),
        ];
        let report = prepare_items(&mut items, ApiBackend::Responses, true);
        assert_eq!(report.stripped_encrypted, 1);
        assert_eq!(report.dropped_reasoning, 1);
        assert!(reasoning_texts(&items).is_empty());
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn responses_to_messages_drops_foreign_thinking() {
        let mut items = vec![
            ConversationItem::user("go"),
            responses_reasoning("rs_1", "hidden chain", Some("xai-blob")),
            ConversationItem::assistant_with_model("done", "grok-4.6"),
        ];
        let report = prepare_items(&mut items, ApiBackend::Messages, true);
        assert_eq!(
            report,
            TranslateReport {
                stripped_encrypted: 1,
                dropped_reasoning: 1,
                substituted_images: 0
            }
        );
        assert!(reasoning_texts(&items).is_empty());
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn messages_keeps_own_signatures_when_thinking_enabled() {
        let mut items = vec![
            anthropic_thinking("think", "anth-sig"),
            ConversationItem::assistant_with_model("ok", "claude-sonnet-5"),
        ];
        let report = prepare_items(&mut items, ApiBackend::Messages, true);
        assert_eq!(report, TranslateReport::default());
        assert_eq!(encrypted(&items), vec![Some("anth-sig")]);
    }

    #[test]
    fn messages_drops_thinking_when_effort_off() {
        let mut items = vec![
            anthropic_thinking("think", "anth-sig"),
            ConversationItem::assistant("ok"),
        ];
        let report = prepare_items(&mut items, ApiBackend::Messages, false);
        assert_eq!(report.dropped_reasoning, 1);
        assert!(reasoning_texts(&items).is_empty());
    }

    #[test]
    fn responses_keeps_own_encrypted_blobs() {
        let mut items = vec![
            responses_reasoning("rs_1", "summary", Some("xai-blob")),
            responses_reasoning("tco_2", "", Some("tool-blob")),
            ConversationItem::assistant_with_model("ok", "grok-4.6"),
        ];
        let report = prepare_items(&mut items, ApiBackend::Responses, true);
        assert_eq!(report, TranslateReport::default());
        assert_eq!(encrypted(&items), vec![Some("xai-blob"), Some("tool-blob")]);
    }

    #[test]
    fn chat_completions_strips_encrypted_keeps_text() {
        let mut items = vec![
            anthropic_thinking("plan", "anth-sig"),
            responses_reasoning("rs_1", "chain", Some("xai-blob")),
            ConversationItem::assistant("ok"),
        ];
        let report = prepare_items(&mut items, ApiBackend::ChatCompletions, true);
        assert_eq!(report.stripped_encrypted, 2);
        assert_eq!(report.dropped_reasoning, 0);
        assert_eq!(encrypted(&items), vec![None, None]);
        assert_eq!(
            reasoning_texts(&items),
            vec!["plan".to_string(), "chain".to_string()]
        );
    }

    #[test]
    fn mixed_session_round_trip_is_bidirectional() {
        // Stored history is never mutated; each target gets its own clone.
        let stored = vec![
            ConversationItem::user("from claude"),
            anthropic_thinking("claude thoughts", "anth-sig"),
            ConversationItem::assistant_with_model("step 1", "claude-opus-5"),
            ConversationItem::user("now grok"),
            responses_reasoning("rs_9", "grok thoughts", Some("xai-blob")),
            ConversationItem::assistant_with_model("step 2", "grok-4.6"),
        ];

        let mut for_grok = stored.clone();
        prepare_items(&mut for_grok, ApiBackend::Responses, true);
        assert_eq!(encrypted(&for_grok), vec![None, Some("xai-blob")]);
        assert_eq!(
            reasoning_texts(&for_grok),
            vec!["claude thoughts".to_string(), "grok thoughts".to_string()]
        );

        let mut for_claude = stored.clone();
        prepare_items(&mut for_claude, ApiBackend::Messages, true);
        assert_eq!(encrypted(&for_claude), vec![Some("anth-sig")]);
        assert_eq!(
            reasoning_texts(&for_claude),
            vec!["claude thoughts".to_string()]
        );

        // Original session is still complete — switch back anytime.
        assert_eq!(encrypted(&stored), vec![Some("anth-sig"), Some("xai-blob")]);
    }

    #[test]
    fn prepare_request_uses_reasoning_effort_as_thinking_flag() {
        let mut req = ConversationRequest::from_items(vec![
            anthropic_thinking("think", "anth-sig"),
            ConversationItem::assistant("ok"),
        ]);
        req.reasoning_effort = None;
        let report = prepare_request(&mut req, ApiBackend::Messages);
        assert_eq!(report.dropped_reasoning, 1);

        let mut req = ConversationRequest::from_items(vec![
            anthropic_thinking("think", "anth-sig"),
            ConversationItem::assistant("ok"),
        ]);
        req.reasoning_effort = Some(ReasoningEffort::High);
        let report = prepare_request(&mut req, ApiBackend::Messages);
        assert_eq!(report, TranslateReport::default());
    }

    #[test]
    fn non_reasoning_items_pass_through() {
        let mut items = vec![
            ConversationItem::system("sys"),
            ConversationItem::user("hi"),
            ConversationItem::assistant_tool_calls(vec![]),
            ConversationItem::tool_result("c1", "out"),
        ];
        let before = items.len();
        let report = prepare_items(&mut items, ApiBackend::Responses, true);
        assert_eq!(report, TranslateReport::default());
        assert_eq!(items.len(), before);
    }

    // ========== no-vision image substitution tests ==========

    #[test]
    fn no_vision_model_gets_a_located_placeholder_instead_of_the_image() {
        let mut user = ConversationItem::user("describe this");
        user.add_image("file:///Users/me/screenshot.png");
        let mut req = ConversationRequest::from_items(vec![user]);
        req.model = Some("opencode-glm-5.1".to_string());

        let report = prepare_request(&mut req, ApiBackend::ChatCompletions);

        assert_eq!(report.substituted_images, 1);
        assert!(report.changed());
        let ConversationItem::User(user) = &req.items[0] else {
            panic!("expected a user item");
        };
        assert_eq!(user.content.len(), 2);
        assert_matches!(&user.content[1], ContentPart::Text { text } => {
            assert!(text.contains("/Users/me/screenshot.png"), "{text}");
            assert!(text.contains("opencode-glm-5.1"), "{text}");
        });
    }

    #[test]
    fn no_vision_model_notes_a_dropped_tool_result_image_in_its_content() {
        let tool_result = ConversationItem::tool_result_with_images(
            "call-1",
            "file read ok",
            vec![ContentPart::Image {
                url: std::sync::Arc::<str>::from("file:///tmp/read.png"),
            }],
        );
        let mut req = ConversationRequest::from_items(vec![tool_result]);
        req.model = Some("opencode-hy3".to_string());

        let report = prepare_request(&mut req, ApiBackend::ChatCompletions);

        assert_eq!(report.substituted_images, 1);
        let ConversationItem::ToolResult(tool_result) = &req.items[0] else {
            panic!("expected a tool result item");
        };
        assert!(tool_result.images.is_empty());
        assert!(tool_result.content.contains("file read ok"));
        assert!(tool_result.content.contains("/tmp/read.png"));
    }

    #[test]
    fn vision_capable_model_keeps_the_image_untouched() {
        let mut user = ConversationItem::user("describe this");
        user.add_image("data:image/png;base64,abc123");
        let mut req = ConversationRequest::from_items(vec![user]);
        req.model = Some("claude-sonnet-5".to_string());

        let report = prepare_request(&mut req, ApiBackend::Messages);

        assert_eq!(report.substituted_images, 0);
        let ConversationItem::User(user) = &req.items[0] else {
            panic!("expected a user item");
        };
        assert_matches!(&user.content[1], ContentPart::Image { url } => {
            assert_eq!(url.as_ref(), "data:image/png;base64,abc123");
        });
    }

    #[test]
    fn no_vision_model_with_no_images_reports_no_change() {
        let mut req = ConversationRequest::from_items(vec![ConversationItem::user("hi")]);
        req.model = Some("opencode-qwen3.7-max".to_string());

        let report = prepare_request(&mut req, ApiBackend::Responses);

        assert_eq!(report, TranslateReport::default());
    }
}
