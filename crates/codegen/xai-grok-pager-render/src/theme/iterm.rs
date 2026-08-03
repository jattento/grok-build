//! ItermGreen theme — green-on-violet, ported from an iTerm2 profile.
//!
//! Accents are the profile's ANSI-16 palette verbatim, so the TUI agrees with
//! whatever the shell prints around it.
//!
//! The canvas is painted with the profile's `#160C2A` rather than left as
//! [`Color::Reset`]. Leaving it unpainted preserves `background-opacity` inside
//! the pane, but it also makes the pane indistinguishable from any surrounding
//! multiplexer chrome, which draws on that same unpainted canvas. Painting the
//! profile background at full opacity keeps the hue identical while the chrome
//! stays translucent, so the two read as separate surfaces.

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

/// Helper for concise const `Color::Rgb` definitions.
const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

// iTerm2 profile palette. Values marked "ANSI n" are the profile's 16-color
// palette; the rest are derived from the `#160C2A` background.
#[allow(dead_code)]
mod palette {
    use super::*;

    // ── Canvas and elevated surfaces ────────────────────────────────────
    pub const BASE: Color = rgb(22, 12, 42); //  #160C2A — profile background
    pub const SURFACE: Color = rgb(30, 17, 56); //  #1E1138 — code blocks
    pub const ELEVATED: Color = rgb(42, 26, 76); //  #2A1A4C — highlight
    pub const HOVER: Color = rgb(53, 34, 94); //  #35225E — hover
    pub const SUNKEN: Color = rgb(22, 12, 42); //  #160C2A — scrollbar trough
    pub const SELECTION: Color = rgb(54, 57, 131); // #363983 — profile selection

    // ── Borders ─────────────────────────────────────────────────────────
    pub const BORDER: Color = rgb(58, 42, 92); //  #3A2A5C
    pub const BORDER_ACTIVE: Color = rgb(91, 68, 144); // #5B4490
    pub const BORDER_SUBTLE: Color = rgb(38, 26, 64); //  #261A40

    // ── Green ramp (profile foreground + cursor) ────────────────────────
    pub const FG: Color = rgb(118, 231, 101); // #76E765 — profile foreground
    pub const CURSOR: Color = rgb(120, 249, 76); // #78F94C — profile cursor
    pub const FG_DIM: Color = rgb(91, 176, 79); //  #5BB04F — secondary text
    pub const GREEN_BRIGHT: Color = rgb(143, 203, 132); // #8FCB84
    pub const GREEN_MID: Color = rgb(90, 145, 82); //  #5A9152
    pub const GREEN_DARK: Color = rgb(60, 107, 56); //  #3C6B38

    // ── ANSI 0-7 ────────────────────────────────────────────────────────
    pub const BLACK: Color = rgb(79, 79, 79); //  #4F4F4F — ANSI 0
    pub const RED: Color = rgb(255, 108, 96); //  #FF6C60 — ANSI 1
    pub const GREEN: Color = rgb(168, 255, 96); // #A8FF60 — ANSI 2
    pub const YELLOW: Color = rgb(255, 255, 182); // #FFFFB6 — ANSI 3
    pub const BLUE: Color = rgb(150, 203, 254); // #96CBFE — ANSI 4
    pub const MAGENTA: Color = rgb(255, 115, 253); // #FF73FD — ANSI 5
    pub const CYAN: Color = rgb(198, 197, 254); // #C6C5FE — ANSI 6
    pub const WHITE: Color = rgb(238, 238, 238); // #EEEEEE — ANSI 7

    // ── ANSI 8-15 ───────────────────────────────────────────────────────
    pub const BR_BLACK: Color = rgb(124, 124, 124); // #7C7C7C — ANSI 8
    pub const BR_GREEN: Color = rgb(206, 255, 172); // #CEFFAC — ANSI 10
    pub const BR_BLUE: Color = rgb(181, 220, 255); // #B5DCFF — ANSI 12
    pub const BR_MAGENTA: Color = rgb(255, 156, 254); // #FF9CFE — ANSI 13
    pub const BR_CYAN: Color = rgb(223, 223, 254); // #DFDFFE — ANSI 14

    // ── Diff backgrounds ────────────────────────────────────────────────
    // Saturated enough to quantize to 256-color red/green rather than gray.
    pub const DIFF_DEL_BG: Color = rgb(58, 18, 22); //  #3A1216
    pub const DIFF_INS_BG: Color = rgb(18, 56, 15); //  #12380F
}
use palette::*;

impl Theme {
    /// ItermGreen theme — green-on-violet with a transparent canvas.
    ///
    /// Colors are defined in RGB. Call [`Theme::quantized`] to downgrade
    /// them to the terminal's supported color level before rendering.
    pub const fn iterm_green() -> Self {
        Self {
            bg_base: BASE,
            bg_terminal: BASE,
            bg_light: ELEVATED,
            bg_dark: SURFACE,
            bg_highlight: ELEVATED,
            bg_hover: HOVER,

            accent_user: WHITE,
            accent_assistant: CURSOR,
            accent_thinking: CYAN,
            accent_tool: BR_BLACK,
            accent_system: BLUE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: CURSOR,
            accent_skill: BLUE,

            text_primary: FG,
            text_secondary: FG_DIM,

            gray_dim: GREEN_DARK,
            gray: GREEN_MID,
            gray_bright: GREEN_BRIGHT,

            command: YELLOW,
            path: BLUE,
            running: CYAN,
            warning: YELLOW,

            fuzzy_accent: BLUE,

            accent_plan: YELLOW,

            accent_verify: MAGENTA,

            accent_feedback: BR_GREEN,

            accent_remember: GREEN,

            selection_border: BORDER,
            prompt_border: BORDER_SUBTLE,
            prompt_border_active: BORDER_ACTIVE,
            hover_border: BORDER_SUBTLE,

            accent_model: BR_CYAN,

            scrollbar_bg: SUNKEN,
            scrollbar_fg: BORDER,

            diff_delete_bg: DIFF_DEL_BG,
            diff_delete_fg: RED,
            diff_insert_bg: DIFF_INS_BG,
            diff_insert_fg: GREEN,
            diff_equal_fg: GREEN_MID,
            diff_gutter_fg: GREEN_DARK,

            bg_visual: SELECTION,

            paste_bg: SURFACE,
            paste_fg: FG,
            paste_dim: GREEN_DARK,

            md_heading_h1: CURSOR,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: BLUE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: MAGENTA,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: CYAN,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: GREEN_BRIGHT,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: GREEN_MID,
            md_heading_h6_mod: Modifier::empty(),
            md_code: BR_CYAN,
            md_task_checked: GREEN,
            md_task_unchecked: FG_DIM,
            md_muted: GREEN_MID,
            md_code_bg: SURFACE,
            md_text: FG,
            link_fg: BR_BLUE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canvas is painted on purpose: an unpainted canvas is the same
    /// surface a multiplexer's chrome draws on, leaving no visible seam
    /// between the two. It must stay the profile background exactly, so the
    /// hue matches the translucent chrome around it.
    #[test]
    fn canvas_is_the_painted_profile_background() {
        let theme = Theme::iterm_green();
        assert_eq!(theme.bg_base, Color::Rgb(0x16, 0x0C, 0x2A));
        assert_eq!(theme.bg_terminal, Color::Rgb(0x16, 0x0C, 0x2A));
    }

    /// Elevations are read against the canvas, so each must be lighter than it.
    #[test]
    fn elevations_sit_above_the_canvas() {
        let theme = Theme::iterm_green();
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
    }

    /// Foreground and cursor are the iTerm2 profile values verbatim; drifting
    /// from them is what this theme exists to prevent.
    #[test]
    fn foreground_matches_the_iterm_profile() {
        let theme = Theme::iterm_green();
        assert_eq!(theme.text_primary, Color::Rgb(0x76, 0xE7, 0x65));
        assert_eq!(theme.accent_assistant, Color::Rgb(0x78, 0xF9, 0x4C));
        assert_eq!(theme.bg_visual, Color::Rgb(0x36, 0x39, 0x83));
    }

    /// The theme is truecolor-only, so the native values must reach the screen
    /// untouched when the terminal supports them.
    #[test]
    fn truecolor_passes_through_unquantized() {
        use crate::theme::color_support::ColorLevel;
        let theme = Theme::iterm_green().quantized(ColorLevel::TrueColor);
        assert_eq!(theme.bg_base, Color::Rgb(0x16, 0x0C, 0x2A));
        assert_eq!(theme.text_primary, Color::Rgb(0x76, 0xE7, 0x65));
    }
}
