//! Affordance-row subject policy for copyable markdown blocks.
//!
//! Dependency-free labels and button sets only. Display-column geometry and
//! pager private types stay in `xai-grok-pager`.

use std::borrow::Cow;

/// What an affordance row is attached to — decides its label and its buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AffordanceSubject {
    /// A ` ```mermaid ` fence: render-on-click plus copy-source.
    Mermaid,
    /// Any other closed fence. Carries its display label (the info string's
    /// first token, or `code` when the fence has none).
    Code(String),
    /// A GFM table.
    Table,
}

/// Which click action an affordance-row button triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AffordanceKind {
    /// Render the diagram (if not already cached) at the live theme/width, then
    /// open the resulting PNG in the OS default app.
    Open,
    /// Render the diagram (if not already cached), then copy the PNG's path.
    CopyPath,
    /// Copy the diagram's Mermaid source (no render needed).
    CopySource,
    /// Copy the block's source (no render needed).
    Copy,
}

/// Subtle `◇ mermaid` marker: the leading (dim, non-clickable) label on the
/// Mermaid affordance row.
pub const MERMAID_LABEL: &str = "\u{25c7} mermaid";

/// Leading label for a GFM table's affordance row.
pub const TABLE_LABEL: &str = "\u{25c7} table";

/// Status hint shown in the affordance row while an on-click diagram render is
/// in flight.
pub const MERMAID_RENDERING: &str = "rendering diagram\u{2026}";

/// Affordance-row button label: open the rendered PNG in the OS default app.
pub const AFFORDANCE_OPEN: &str = "[Open Image]";
/// Affordance-row button label: copy the rendered PNG's filesystem path.
pub const AFFORDANCE_COPY_PATH: &str = "[Copy Image Path]";
/// Affordance-row button label: copy the diagram's Mermaid source.
pub const AFFORDANCE_COPY_SOURCE: &str = "[Copy Source]";
/// Affordance-row button label: copy a code block or table's source.
pub const AFFORDANCE_COPY: &str = "[Copy]";

/// Plain policy for one affordance row — labels and kinds only, no columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffordancePolicy {
    /// Leading dim, non-clickable subject label.
    pub label: Cow<'static, str>,
    /// Buttons left-to-right as `(label, kind)`.
    pub buttons: Vec<(&'static str, AffordanceKind)>,
    /// Trailing status hint text while a Mermaid on-click render is in flight.
    pub status: Option<&'static str>,
}

/// Subject → label/button/status policy shared by the pager painter and hit tests.
pub fn affordance_policy(subject: &AffordanceSubject, rendering: bool) -> AffordancePolicy {
    match subject {
        AffordanceSubject::Mermaid => AffordancePolicy {
            label: Cow::Borrowed(MERMAID_LABEL),
            buttons: vec![
                (AFFORDANCE_OPEN, AffordanceKind::Open),
                (AFFORDANCE_COPY_PATH, AffordanceKind::CopyPath),
                (AFFORDANCE_COPY_SOURCE, AffordanceKind::CopySource),
            ],
            status: rendering.then_some(MERMAID_RENDERING),
        },
        AffordanceSubject::Code(lang) => AffordancePolicy {
            label: Cow::Owned(format!("\u{25c7} {lang}")),
            buttons: vec![(AFFORDANCE_COPY, AffordanceKind::Copy)],
            status: None,
        },
        AffordanceSubject::Table => AffordancePolicy {
            label: Cow::Borrowed(TABLE_LABEL),
            buttons: vec![(AFFORDANCE_COPY, AffordanceKind::Copy)],
            status: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mermaid_policy_keeps_three_render_buttons_and_optional_status() {
        let idle = affordance_policy(&AffordanceSubject::Mermaid, false);
        assert_eq!(idle.label, Cow::Borrowed(MERMAID_LABEL));
        assert_eq!(
            idle.buttons,
            vec![
                (AFFORDANCE_OPEN, AffordanceKind::Open),
                (AFFORDANCE_COPY_PATH, AffordanceKind::CopyPath),
                (AFFORDANCE_COPY_SOURCE, AffordanceKind::CopySource),
            ]
        );
        assert_eq!(idle.status, None);

        let busy = affordance_policy(&AffordanceSubject::Mermaid, true);
        assert_eq!(busy.buttons, idle.buttons);
        assert_eq!(busy.status, Some(MERMAID_RENDERING));
    }

    #[test]
    fn code_and_table_policy_are_source_copy_only() {
        let code = affordance_policy(&AffordanceSubject::Code("bash".into()), true);
        assert_eq!(code.label.as_ref(), "\u{25c7} bash");
        assert_eq!(code.buttons, vec![(AFFORDANCE_COPY, AffordanceKind::Copy)]);
        assert_eq!(code.status, None, "code rows never render on click");

        let table = affordance_policy(&AffordanceSubject::Table, true);
        assert_eq!(table.label, Cow::Borrowed(TABLE_LABEL));
        assert_eq!(table.buttons, vec![(AFFORDANCE_COPY, AffordanceKind::Copy)]);
        assert_eq!(table.status, None);
    }
}
