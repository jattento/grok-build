//! Affordance-row subject policy and width measurement for copyable markdown blocks.
//!
//! Dependency-free: labels, button sets, and the fit predicate shared by the
//! pager's row reservation, painter, and hit-test. Column geometry for the full
//! multi-button layout still lives in `xai-grok-pager`.

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

/// Display-column gap between adjacent affordance-row segments (label → first
/// button, button → button, last button → status hint).
pub const AFFORDANCE_GAP: u16 = 3;

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

/// Display columns for affordance chrome.
///
/// All labels and button strings are ASCII plus single-cell symbols (`◇`, `…`),
/// so character count equals terminal cells without a Unicode-width dependency.
#[inline]
pub fn affordance_display_cols(text: &str) -> u16 {
    text.chars().count() as u16
}

/// Whether a segment starting at display column `col` with text `label` fits
/// wholly inside a row of `width` columns.
#[inline]
pub fn affordance_segment_fits(col: u16, label: &str, width: u16) -> bool {
    col.saturating_add(affordance_display_cols(label)) <= width
}

/// Columns needed so the first action button fits after the subject label.
///
/// Matches the pager's left-to-right layout: label at column 0, then
/// [`AFFORDANCE_GAP`], then the first button. The row is only useful when this
/// much width is available — otherwise the painter would drop every button and
/// leave either an empty gap or an inert label.
pub fn affordance_min_action_width(subject: &AffordanceSubject) -> u16 {
    let policy = affordance_policy(subject, false);
    let label_w = affordance_display_cols(policy.label.as_ref());
    let Some(&(btn_label, _)) = policy.buttons.first() else {
        return 0;
    };
    label_w
        .saturating_add(AFFORDANCE_GAP)
        .saturating_add(affordance_display_cols(btn_label))
}

/// Whether at least one action button can be painted for `subject` at `width`.
///
/// Shared by row reservation (`apply_affordance_rows`), the painter, and
/// hit-testing: a row is reserved only when this is true, and a button is
/// painted/clickable only when its own segment also fits.
#[inline]
pub fn affordance_row_fits(subject: &AffordanceSubject, width: u16) -> bool {
    let policy = affordance_policy(subject, false);
    let label_w = affordance_display_cols(policy.label.as_ref());
    let Some(&(btn_label, _)) = policy.buttons.first() else {
        return false;
    };
    let btn_col = label_w.saturating_add(AFFORDANCE_GAP);
    affordance_segment_fits(btn_col, btn_label, width)
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

    #[test]
    fn mermaid_row_fits_at_first_button_edge_not_below() {
        // `◇ mermaid` (9) + gap (3) + `[Open Image]` (12) = 24.
        let min = affordance_min_action_width(&AffordanceSubject::Mermaid);
        assert_eq!(min, 24);
        assert!(affordance_row_fits(&AffordanceSubject::Mermaid, min));
        assert!(!affordance_row_fits(
            &AffordanceSubject::Mermaid,
            min.saturating_sub(1)
        ));
    }

    #[test]
    fn code_row_fits_requires_label_gap_and_copy_button() {
        let subject = AffordanceSubject::Code("rust".into());
        // `◇ rust` (6) + gap (3) + `[Copy]` (6) = 15.
        assert_eq!(affordance_min_action_width(&subject), 15);
        assert!(affordance_row_fits(&subject, 15));
        assert!(!affordance_row_fits(&subject, 14));
    }

    #[test]
    fn segment_fits_matches_row_predicate_for_first_button() {
        let subject = AffordanceSubject::Table;
        let policy = affordance_policy(&subject, false);
        let label_w = affordance_display_cols(policy.label.as_ref());
        let btn_col = label_w + AFFORDANCE_GAP;
        let (btn_label, _) = policy.buttons[0];
        let min = affordance_min_action_width(&subject);
        assert!(affordance_segment_fits(btn_col, btn_label, min));
        assert!(!affordance_segment_fits(
            btn_col,
            btn_label,
            min.saturating_sub(1)
        ));
        assert_eq!(
            affordance_row_fits(&subject, min),
            affordance_segment_fits(btn_col, btn_label, min)
        );
    }

    #[test]
    fn display_cols_treats_diamond_and_ellipsis_as_single_cells() {
        assert_eq!(affordance_display_cols(MERMAID_LABEL), 9);
        assert_eq!(affordance_display_cols(TABLE_LABEL), 7);
        // "rendering diagram" (17) + ellipsis (1) = 18.
        assert_eq!(affordance_display_cols(MERMAID_RENDERING), 18);
        assert_eq!(affordance_display_cols(AFFORDANCE_COPY), 6);
    }
}
