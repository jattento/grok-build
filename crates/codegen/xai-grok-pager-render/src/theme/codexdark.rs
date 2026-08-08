//! CodexDark theme — ported from Conan Code's own embedded Ghostty theme
//! (`Coinor/Resources/GhosttyOverrides.conf`), so the pager and the terminal
//! chrome around it read as one surface instead of two unrelated palettes.
//!
//! Canvas, accent, and foreground are the Ghostty values verbatim:
//! background `#181818`, accent `#339CFF` (also the terminal cursor and the
//! selected-tab underline in `TerminalTabStrip.swift`), and a warm cream
//! foreground `#FAF3DD` instead of a neutral gray or white. The ANSI-16
//! ramp is Conan Code's own diff/status palette (soft green, coral red,
//! amber warning, sky blue, muted purple, teal) verbatim too, so command
//! output painted by the shell agrees with output painted by this theme.
//!
//! Elevation tiers, borders, and diff backgrounds have no Ghostty
//! equivalent (Ghostty only defines a flat 16-color terminal, not a
//! layered UI), so those are derived from the canvas/surface/accent below
//! and documented as such.

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

/// Helper for concise const `Color::Rgb` definitions.
const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

// Conan Code / Ghostty palette. Values marked "Ghostty" are copied verbatim
// from `GhosttyOverrides.conf`; the rest are derived from them (see doc
// comment above).
#[allow(dead_code)]
mod palette {
    use super::*;

    // ── Canvas and elevated surfaces ────────────────────────────────────
    pub const BASE: Color = rgb(0x18, 0x18, 0x18); // #181818 — Ghostty background
    pub const SURFACE: Color = rgb(0x2D, 0x2D, 0x2B); // #2D2D2B — Ghostty palette 0 ("surface")
    pub const ELEVATED: Color = rgb(0x3D, 0x3D, 0x39); // #3D3D39 — surface, one step lighter
    pub const HOVER: Color = rgb(0x4E, 0x4D, 0x47); // #4E4D47 — surface, two steps lighter
    pub const SUNKEN: Color = rgb(0x12, 0x12, 0x12); // #121212 — base, one step darker
    pub const SELECTION: Color = rgb(0x2A, 0x35, 0x3A); // #2A353A — Ghostty selection-background

    // ── Borders (derived: base→surface→accent ladder) ───────────────────
    pub const BORDER_SUBTLE: Color = rgb(0x24, 0x24, 0x22); // #242422
    pub const BORDER: Color = rgb(0x3A, 0x3A, 0x37); // #3A3A37
    pub const BORDER_ACTIVE: Color = rgb(0x30, 0x6A, 0xA0); // #306AA0 — accent-tinted

    // ── Foreground and accent (Ghostty verbatim) ────────────────────────
    pub const FG: Color = rgb(0xFA, 0xF3, 0xDD); // #FAF3DD — Ghostty foreground (warm cream)
    pub const FG_SECONDARY: Color = rgb(0xC7, 0xC2, 0xB0); // #C7C2B0 — FG toward surface, 25%
    pub const ACCENT: Color = rgb(0x33, 0x9C, 0xFF); // #339CFF — Ghostty cursor-color / AccentColor
    pub const GOLD: Color = rgb(0xE8, 0xBC, 0x47); // #E8BC47 — plan-mode gold, warmed toward FG

    // ── ANSI 0-15 (Ghostty `palette =` lines, verbatim) ─────────────────
    pub const GRAY_DIM: Color = rgb(0x6B, 0x68, 0x60); // #6B6860 — FG toward surface, 70%
    pub const GRAY: Color = rgb(0x96, 0x96, 0x96); // #969696 — ANSI 8, bright black
    pub const GRAY_BRIGHT: Color = rgb(0xC7, 0xC7, 0xC7); // #C7C7C7 — ANSI 7, white
    pub const RED: Color = rgb(0xE7, 0x52, 0x48); // #E75248 — ANSI 1/9, coral red
    pub const GREEN: Color = rgb(0x6B, 0xC6, 0x7F); // #6BC67F — ANSI 2/10, soft green
    pub const ORANGE: Color = rgb(0xEF, 0x8C, 0x57); // #EF8C57 — ANSI 3/11, amber warning
    pub const BLUE: Color = rgb(0x91, 0xC1, 0xFA); // #91C1FA — ANSI 4/12, sky blue
    pub const PURPLE: Color = rgb(0xB9, 0xA3, 0xEC); // #B9A3EC — ANSI 5/13, muted purple
    pub const TEAL: Color = rgb(0x7F, 0xD4, 0xC1); // #7FD4C1 — ANSI 6/14, teal

    // ── Diff backgrounds (derived: base tinted toward red/green) ────────
    pub const DIFF_DEL_BG: Color = rgb(0x46, 0x25, 0x23); // #462523
    pub const DIFF_INS_BG: Color = rgb(0x2A, 0x3E, 0x2F); // #2A3E2F
}
use palette::*;

impl Theme {
    /// CodexDark theme — Conan Code's own Ghostty palette, ported verbatim.
    ///
    /// Colors are defined in RGB. Call [`Theme::quantized`] to downgrade
    /// them to the terminal's supported color level before rendering.
    pub const fn codex_dark() -> Self {
        Self {
            bg_base: BASE,
            bg_terminal: BASE,
            bg_light: ELEVATED,
            bg_dark: SURFACE,
            bg_highlight: ELEVATED,
            bg_hover: HOVER,

            accent_user: ACCENT,
            accent_assistant: PURPLE,
            accent_thinking: GRAY,
            accent_tool: GRAY_BRIGHT,
            accent_system: ACCENT,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: PURPLE,
            accent_skill: TEAL,

            text_primary: FG,
            text_secondary: FG_SECONDARY,

            gray_dim: GRAY_DIM,
            gray: GRAY,
            gray_bright: GRAY_BRIGHT,

            command: ORANGE,
            path: ORANGE,
            running: TEAL,
            warning: ORANGE,

            fuzzy_accent: ACCENT,

            accent_plan: GOLD,

            accent_verify: PURPLE,


            accent_remember: GREEN,

            selection_border: BORDER,
            prompt_border: BORDER_SUBTLE,
            prompt_border_active: BORDER_ACTIVE,
            hover_border: BORDER,

            accent_model: TEAL,

            scrollbar_bg: SUNKEN,
            scrollbar_fg: ELEVATED,

            diff_delete_bg: DIFF_DEL_BG,
            diff_delete_fg: RED,
            diff_insert_bg: DIFF_INS_BG,
            diff_insert_fg: GREEN,
            diff_equal_fg: GRAY,
            diff_gutter_fg: GRAY_DIM,

            bg_visual: SELECTION,

            paste_bg: SUNKEN,
            paste_fg: FG_SECONDARY,
            paste_dim: GRAY_DIM,

            md_heading_h1: ACCENT,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: TEAL,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: ORANGE,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: RED,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: GREEN,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: PURPLE,
            md_heading_h6_mod: Modifier::empty(),
            md_code: TEAL,
            md_task_checked: GREEN,
            md_task_unchecked: ACCENT,
            md_muted: GRAY,
            md_code_bg: SURFACE,
            md_text: FG,
            link_fg: ACCENT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canvas, accent, and foreground must match `GhosttyOverrides.conf` /
    /// `AccentColor.colorset` verbatim — drifting from them is what this
    /// theme exists to prevent.
    #[test]
    fn matches_the_ghostty_profile() {
        let theme = Theme::codex_dark();
        assert_eq!(theme.bg_base, Color::Rgb(0x18, 0x18, 0x18));
        assert_eq!(theme.bg_terminal, Color::Rgb(0x18, 0x18, 0x18));
        assert_eq!(theme.text_primary, Color::Rgb(0xFA, 0xF3, 0xDD));
        assert_eq!(theme.accent_user, Color::Rgb(0x33, 0x9C, 0xFF));
        assert_eq!(theme.accent_system, Color::Rgb(0x33, 0x9C, 0xFF));
        assert_eq!(theme.bg_visual, Color::Rgb(0x2A, 0x35, 0x3A));
    }

    /// Elevations are read against the canvas, so each must be lighter
    /// than it, and `SUNKEN` must be darker than `BASE`.
    #[test]
    fn elevations_sit_above_the_canvas() {
        let theme = Theme::codex_dark();
        let lum = |c: Color| match c {
            Color::Rgb(r, g, b) => r as i32 + g as i32 + b as i32,
            other => panic!("expected Rgb, got {other:?}"),
        };
        let base = lum(theme.bg_base);
        for (name, color) in [
            ("bg_dark", theme.bg_dark),
            ("bg_light", theme.bg_light),
            ("bg_hover", theme.bg_hover),
            ("md_code_bg", theme.md_code_bg),
        ] {
            assert!(lum(color) > base, "{name} must be lighter than bg_base");
        }
        assert!(
            lum(theme.scrollbar_bg) < base,
            "scrollbar_bg (SUNKEN) must be darker than bg_base"
        );
    }

    /// The theme is truecolor-only, so the native values must reach the
    /// screen untouched when the terminal supports them.
    #[test]
    fn truecolor_passes_through_unquantized() {
        use crate::theme::color_support::ColorLevel;
        let theme = Theme::codex_dark().quantized(ColorLevel::TrueColor);
        assert_eq!(theme.bg_base, Color::Rgb(0x18, 0x18, 0x18));
        assert_eq!(theme.text_primary, Color::Rgb(0xFA, 0xF3, 0xDD));
    }
}
