//! Fork-owned theme palettes as plain RGB tuples.
//!
//! Kept dependency-free (no `ratatui`) so an upstream sync can never break
//! this package. The pager-render crate expands the exported macros into a
//! `Theme { ... }` literal at the call site where `Color` / `Modifier` live.

/// CodexDark palette — ported from Conan Code's own embedded Ghostty theme
/// (`Coinor/Resources/GhosttyOverrides.conf`), so the pager and the terminal
/// chrome around it read as one surface instead of two unrelated palettes.
///
/// Canvas, accent, and foreground are the Ghostty values verbatim:
/// background `#181818`, accent `#339CFF` (also the terminal cursor and the
/// selected-tab underline in `TerminalTabStrip.swift`), and a warm cream
/// foreground `#FAF3DD` instead of a neutral gray or white. The ANSI-16
/// ramp is Conan Code's own diff/status palette (soft green, coral red,
/// amber warning, sky blue, muted purple, teal) verbatim too, so command
/// output painted by the shell agrees with output painted by this theme.
///
/// Elevation tiers, borders, and diff backgrounds have no Ghostty
/// equivalent (Ghostty only defines a flat 16-color terminal, not a
/// layered UI), so those are derived from the canvas/surface/accent below
/// and documented as such.
///
/// Values marked "Ghostty" are copied verbatim from `GhosttyOverrides.conf`;
/// the rest are derived from them (see doc comment above).
pub mod codex_dark {
    // ── Canvas and elevated surfaces ────────────────────────────────────
    pub const BASE: (u8, u8, u8) = (0x18, 0x18, 0x18); // #181818 — Ghostty background
    pub const SURFACE: (u8, u8, u8) = (0x2D, 0x2D, 0x2B); // #2D2D2B — Ghostty palette 0 ("surface")
    pub const ELEVATED: (u8, u8, u8) = (0x3D, 0x3D, 0x39); // #3D3D39 — surface, one step lighter
    pub const HOVER: (u8, u8, u8) = (0x4E, 0x4D, 0x47); // #4E4D47 — surface, two steps lighter
    pub const SUNKEN: (u8, u8, u8) = (0x12, 0x12, 0x12); // #121212 — base, one step darker
    pub const SELECTION: (u8, u8, u8) = (0x2A, 0x35, 0x3A); // #2A353A — Ghostty selection-background

    // ── Borders (derived: base→surface→accent ladder) ───────────────────
    pub const BORDER_SUBTLE: (u8, u8, u8) = (0x24, 0x24, 0x22); // #242422
    pub const BORDER: (u8, u8, u8) = (0x3A, 0x3A, 0x37); // #3A3A37
    pub const BORDER_ACTIVE: (u8, u8, u8) = (0x30, 0x6A, 0xA0); // #306AA0 — accent-tinted

    // ── Foreground and accent (Ghostty verbatim) ────────────────────────
    pub const FG: (u8, u8, u8) = (0xFA, 0xF3, 0xDD); // #FAF3DD — Ghostty foreground (warm cream)
    pub const FG_SECONDARY: (u8, u8, u8) = (0xC7, 0xC2, 0xB0); // #C7C2B0 — FG toward surface, 25%
    pub const ACCENT: (u8, u8, u8) = (0x33, 0x9C, 0xFF); // #339CFF — Ghostty cursor-color / AccentColor
    pub const GOLD: (u8, u8, u8) = (0xE8, 0xBC, 0x47); // #E8BC47 — plan-mode gold, warmed toward FG

    // ── ANSI 0-15 (Ghostty `palette =` lines, verbatim) ─────────────────
    pub const GRAY_DIM: (u8, u8, u8) = (0x6B, 0x68, 0x60); // #6B6860 — FG toward surface, 70%
    pub const GRAY: (u8, u8, u8) = (0x96, 0x96, 0x96); // #969696 — ANSI 8, bright black
    pub const GRAY_BRIGHT: (u8, u8, u8) = (0xC7, 0xC7, 0xC7); // #C7C7C7 — ANSI 7, white
    pub const RED: (u8, u8, u8) = (0xE7, 0x52, 0x48); // #E75248 — ANSI 1/9, coral red
    pub const GREEN: (u8, u8, u8) = (0x6B, 0xC6, 0x7F); // #6BC67F — ANSI 2/10, soft green
    pub const ORANGE: (u8, u8, u8) = (0xEF, 0x8C, 0x57); // #EF8C57 — ANSI 3/11, amber warning
    pub const BLUE: (u8, u8, u8) = (0x91, 0xC1, 0xFA); // #91C1FA — ANSI 4/12, sky blue
    pub const PURPLE: (u8, u8, u8) = (0xB9, 0xA3, 0xEC); // #B9A3EC — ANSI 5/13, muted purple
    pub const TEAL: (u8, u8, u8) = (0x7F, 0xD4, 0xC1); // #7FD4C1 — ANSI 6/14, teal

    // ── Diff backgrounds (derived: base tinted toward red/green) ────────
    pub const DIFF_DEL_BG: (u8, u8, u8) = (0x46, 0x25, 0x23); // #462523
    pub const DIFF_INS_BG: (u8, u8, u8) = (0x2A, 0x3E, 0x2F); // #2A3E2F
}

/// ItermGreen palette — green-on-violet, ported from an iTerm2 profile.
///
/// Accents are the profile's ANSI-16 palette verbatim, so the TUI agrees with
/// whatever the shell prints around it.
///
/// The canvas is painted with the profile's `#160C2A` rather than left
/// unpainted. Leaving it unpainted preserves `background-opacity` inside
/// the pane, but it also makes the pane indistinguishable from any surrounding
/// multiplexer chrome, which draws on that same unpainted canvas. Painting the
/// profile background at full opacity keeps the hue identical while the chrome
/// stays translucent, so the two read as separate surfaces.
///
/// Values marked "ANSI n" are the profile's 16-color palette; the rest are
/// derived from the `#160C2A` background.
pub mod iterm_green {
    // ── Canvas and elevated surfaces ────────────────────────────────────
    pub const BASE: (u8, u8, u8) = (0x16, 0x0C, 0x2A); // #160C2A — profile background
    pub const SURFACE: (u8, u8, u8) = (0x1E, 0x11, 0x38); // #1E1138 — code blocks
    pub const ELEVATED: (u8, u8, u8) = (0x2A, 0x1A, 0x4C); // #2A1A4C — highlight
    pub const HOVER: (u8, u8, u8) = (0x35, 0x22, 0x5E); // #35225E — hover
    pub const SUNKEN: (u8, u8, u8) = (0x16, 0x0C, 0x2A); // #160C2A — scrollbar trough
    pub const SELECTION: (u8, u8, u8) = (0x36, 0x39, 0x83); // #363983 — profile selection

    // ── Borders ─────────────────────────────────────────────────────────
    pub const BORDER: (u8, u8, u8) = (0x3A, 0x2A, 0x5C); // #3A2A5C
    pub const BORDER_ACTIVE: (u8, u8, u8) = (0x5B, 0x44, 0x90); // #5B4490
    pub const BORDER_SUBTLE: (u8, u8, u8) = (0x26, 0x1A, 0x40); // #261A40

    // ── Green ramp (profile foreground + cursor) ────────────────────────
    pub const FG: (u8, u8, u8) = (0x76, 0xE7, 0x65); // #76E765 — profile foreground
    pub const CURSOR: (u8, u8, u8) = (0x78, 0xF9, 0x4C); // #78F94C — profile cursor
    pub const FG_DIM: (u8, u8, u8) = (0x5B, 0xB0, 0x4F); // #5BB04F — secondary text
    pub const GREEN_BRIGHT: (u8, u8, u8) = (0x8F, 0xCB, 0x84); // #8FCB84
    pub const GREEN_MID: (u8, u8, u8) = (0x5A, 0x91, 0x52); // #5A9152
    pub const GREEN_DARK: (u8, u8, u8) = (0x3C, 0x6B, 0x38); // #3C6B38

    // ── ANSI 0-7 ────────────────────────────────────────────────────────
    pub const BLACK: (u8, u8, u8) = (0x4F, 0x4F, 0x4F); // #4F4F4F — ANSI 0
    pub const RED: (u8, u8, u8) = (0xFF, 0x6C, 0x60); // #FF6C60 — ANSI 1
    pub const GREEN: (u8, u8, u8) = (0xA8, 0xFF, 0x60); // #A8FF60 — ANSI 2
    pub const YELLOW: (u8, u8, u8) = (0xFF, 0xFF, 0xB6); // #FFFFB6 — ANSI 3
    pub const BLUE: (u8, u8, u8) = (0x96, 0xCB, 0xFE); // #96CBFE — ANSI 4
    pub const MAGENTA: (u8, u8, u8) = (0xFF, 0x73, 0xFD); // #FF73FD — ANSI 5
    pub const CYAN: (u8, u8, u8) = (0xC6, 0xC5, 0xFE); // #C6C5FE — ANSI 6
    pub const WHITE: (u8, u8, u8) = (0xEE, 0xEE, 0xEE); // #EEEEEE — ANSI 7

    // ── ANSI 8-15 ───────────────────────────────────────────────────────
    pub const BR_BLACK: (u8, u8, u8) = (0x7C, 0x7C, 0x7C); // #7C7C7C — ANSI 8
    pub const BR_GREEN: (u8, u8, u8) = (0xCE, 0xFF, 0xAC); // #CEFFAC — ANSI 10
    pub const BR_BLUE: (u8, u8, u8) = (0xB5, 0xDC, 0xFF); // #B5DCFF — ANSI 12
    pub const BR_MAGENTA: (u8, u8, u8) = (0xFF, 0x9C, 0xFE); // #FF9CFE — ANSI 13
    pub const BR_CYAN: (u8, u8, u8) = (0xDF, 0xDF, 0xFE); // #DFDFFE — ANSI 14

    // ── Diff backgrounds ────────────────────────────────────────────────
    // Saturated enough to quantize to 256-color red/green rather than gray.
    pub const DIFF_DEL_BG: (u8, u8, u8) = (0x3A, 0x12, 0x16); // #3A1216
    pub const DIFF_INS_BG: (u8, u8, u8) = (0x12, 0x38, 0x0F); // #12380F
}

/// Expand to a full `Theme { ... }` struct literal for CodexDark.
///
/// Requires `Theme`, `Color`, and `Modifier` in scope at the expansion site.
/// Valid inside a `const fn` (const tuple field access).
#[macro_export]
macro_rules! codex_dark_theme {
    () => {
        Theme {
            bg_base: Color::Rgb(
                $crate::themes::codex_dark::BASE.0,
                $crate::themes::codex_dark::BASE.1,
                $crate::themes::codex_dark::BASE.2,
            ),
            bg_terminal: Color::Rgb(
                $crate::themes::codex_dark::BASE.0,
                $crate::themes::codex_dark::BASE.1,
                $crate::themes::codex_dark::BASE.2,
            ),
            bg_light: Color::Rgb(
                $crate::themes::codex_dark::ELEVATED.0,
                $crate::themes::codex_dark::ELEVATED.1,
                $crate::themes::codex_dark::ELEVATED.2,
            ),
            bg_dark: Color::Rgb(
                $crate::themes::codex_dark::SURFACE.0,
                $crate::themes::codex_dark::SURFACE.1,
                $crate::themes::codex_dark::SURFACE.2,
            ),
            bg_highlight: Color::Rgb(
                $crate::themes::codex_dark::ELEVATED.0,
                $crate::themes::codex_dark::ELEVATED.1,
                $crate::themes::codex_dark::ELEVATED.2,
            ),
            bg_hover: Color::Rgb(
                $crate::themes::codex_dark::HOVER.0,
                $crate::themes::codex_dark::HOVER.1,
                $crate::themes::codex_dark::HOVER.2,
            ),

            accent_user: Color::Rgb(
                $crate::themes::codex_dark::ACCENT.0,
                $crate::themes::codex_dark::ACCENT.1,
                $crate::themes::codex_dark::ACCENT.2,
            ),
            accent_assistant: Color::Rgb(
                $crate::themes::codex_dark::PURPLE.0,
                $crate::themes::codex_dark::PURPLE.1,
                $crate::themes::codex_dark::PURPLE.2,
            ),
            accent_thinking: Color::Rgb(
                $crate::themes::codex_dark::GRAY.0,
                $crate::themes::codex_dark::GRAY.1,
                $crate::themes::codex_dark::GRAY.2,
            ),
            accent_tool: Color::Rgb(
                $crate::themes::codex_dark::GRAY_BRIGHT.0,
                $crate::themes::codex_dark::GRAY_BRIGHT.1,
                $crate::themes::codex_dark::GRAY_BRIGHT.2,
            ),
            accent_system: Color::Rgb(
                $crate::themes::codex_dark::ACCENT.0,
                $crate::themes::codex_dark::ACCENT.1,
                $crate::themes::codex_dark::ACCENT.2,
            ),
            accent_error: Color::Rgb(
                $crate::themes::codex_dark::RED.0,
                $crate::themes::codex_dark::RED.1,
                $crate::themes::codex_dark::RED.2,
            ),
            accent_success: Color::Rgb(
                $crate::themes::codex_dark::GREEN.0,
                $crate::themes::codex_dark::GREEN.1,
                $crate::themes::codex_dark::GREEN.2,
            ),
            accent_running: Color::Rgb(
                $crate::themes::codex_dark::PURPLE.0,
                $crate::themes::codex_dark::PURPLE.1,
                $crate::themes::codex_dark::PURPLE.2,
            ),
            accent_skill: Color::Rgb(
                $crate::themes::codex_dark::TEAL.0,
                $crate::themes::codex_dark::TEAL.1,
                $crate::themes::codex_dark::TEAL.2,
            ),

            text_primary: Color::Rgb(
                $crate::themes::codex_dark::FG.0,
                $crate::themes::codex_dark::FG.1,
                $crate::themes::codex_dark::FG.2,
            ),
            text_secondary: Color::Rgb(
                $crate::themes::codex_dark::FG_SECONDARY.0,
                $crate::themes::codex_dark::FG_SECONDARY.1,
                $crate::themes::codex_dark::FG_SECONDARY.2,
            ),

            gray_dim: Color::Rgb(
                $crate::themes::codex_dark::GRAY_DIM.0,
                $crate::themes::codex_dark::GRAY_DIM.1,
                $crate::themes::codex_dark::GRAY_DIM.2,
            ),
            gray: Color::Rgb(
                $crate::themes::codex_dark::GRAY.0,
                $crate::themes::codex_dark::GRAY.1,
                $crate::themes::codex_dark::GRAY.2,
            ),
            gray_bright: Color::Rgb(
                $crate::themes::codex_dark::GRAY_BRIGHT.0,
                $crate::themes::codex_dark::GRAY_BRIGHT.1,
                $crate::themes::codex_dark::GRAY_BRIGHT.2,
            ),

            command: Color::Rgb(
                $crate::themes::codex_dark::ORANGE.0,
                $crate::themes::codex_dark::ORANGE.1,
                $crate::themes::codex_dark::ORANGE.2,
            ),
            path: Color::Rgb(
                $crate::themes::codex_dark::ORANGE.0,
                $crate::themes::codex_dark::ORANGE.1,
                $crate::themes::codex_dark::ORANGE.2,
            ),
            running: Color::Rgb(
                $crate::themes::codex_dark::TEAL.0,
                $crate::themes::codex_dark::TEAL.1,
                $crate::themes::codex_dark::TEAL.2,
            ),
            warning: Color::Rgb(
                $crate::themes::codex_dark::ORANGE.0,
                $crate::themes::codex_dark::ORANGE.1,
                $crate::themes::codex_dark::ORANGE.2,
            ),

            fuzzy_accent: Color::Rgb(
                $crate::themes::codex_dark::ACCENT.0,
                $crate::themes::codex_dark::ACCENT.1,
                $crate::themes::codex_dark::ACCENT.2,
            ),

            accent_plan: Color::Rgb(
                $crate::themes::codex_dark::GOLD.0,
                $crate::themes::codex_dark::GOLD.1,
                $crate::themes::codex_dark::GOLD.2,
            ),

            accent_verify: Color::Rgb(
                $crate::themes::codex_dark::PURPLE.0,
                $crate::themes::codex_dark::PURPLE.1,
                $crate::themes::codex_dark::PURPLE.2,
            ),

            accent_remember: Color::Rgb(
                $crate::themes::codex_dark::GREEN.0,
                $crate::themes::codex_dark::GREEN.1,
                $crate::themes::codex_dark::GREEN.2,
            ),

            selection_border: Color::Rgb(
                $crate::themes::codex_dark::BORDER.0,
                $crate::themes::codex_dark::BORDER.1,
                $crate::themes::codex_dark::BORDER.2,
            ),
            prompt_border: Color::Rgb(
                $crate::themes::codex_dark::BORDER_SUBTLE.0,
                $crate::themes::codex_dark::BORDER_SUBTLE.1,
                $crate::themes::codex_dark::BORDER_SUBTLE.2,
            ),
            prompt_border_active: Color::Rgb(
                $crate::themes::codex_dark::BORDER_ACTIVE.0,
                $crate::themes::codex_dark::BORDER_ACTIVE.1,
                $crate::themes::codex_dark::BORDER_ACTIVE.2,
            ),
            hover_border: Color::Rgb(
                $crate::themes::codex_dark::BORDER.0,
                $crate::themes::codex_dark::BORDER.1,
                $crate::themes::codex_dark::BORDER.2,
            ),

            accent_model: Color::Rgb(
                $crate::themes::codex_dark::TEAL.0,
                $crate::themes::codex_dark::TEAL.1,
                $crate::themes::codex_dark::TEAL.2,
            ),

            scrollbar_bg: Color::Rgb(
                $crate::themes::codex_dark::SUNKEN.0,
                $crate::themes::codex_dark::SUNKEN.1,
                $crate::themes::codex_dark::SUNKEN.2,
            ),
            scrollbar_fg: Color::Rgb(
                $crate::themes::codex_dark::ELEVATED.0,
                $crate::themes::codex_dark::ELEVATED.1,
                $crate::themes::codex_dark::ELEVATED.2,
            ),

            diff_delete_bg: Color::Rgb(
                $crate::themes::codex_dark::DIFF_DEL_BG.0,
                $crate::themes::codex_dark::DIFF_DEL_BG.1,
                $crate::themes::codex_dark::DIFF_DEL_BG.2,
            ),
            diff_delete_fg: Color::Rgb(
                $crate::themes::codex_dark::RED.0,
                $crate::themes::codex_dark::RED.1,
                $crate::themes::codex_dark::RED.2,
            ),
            diff_insert_bg: Color::Rgb(
                $crate::themes::codex_dark::DIFF_INS_BG.0,
                $crate::themes::codex_dark::DIFF_INS_BG.1,
                $crate::themes::codex_dark::DIFF_INS_BG.2,
            ),
            diff_insert_fg: Color::Rgb(
                $crate::themes::codex_dark::GREEN.0,
                $crate::themes::codex_dark::GREEN.1,
                $crate::themes::codex_dark::GREEN.2,
            ),
            diff_equal_fg: Color::Rgb(
                $crate::themes::codex_dark::GRAY.0,
                $crate::themes::codex_dark::GRAY.1,
                $crate::themes::codex_dark::GRAY.2,
            ),
            diff_gutter_fg: Color::Rgb(
                $crate::themes::codex_dark::GRAY_DIM.0,
                $crate::themes::codex_dark::GRAY_DIM.1,
                $crate::themes::codex_dark::GRAY_DIM.2,
            ),

            bg_visual: Color::Rgb(
                $crate::themes::codex_dark::SELECTION.0,
                $crate::themes::codex_dark::SELECTION.1,
                $crate::themes::codex_dark::SELECTION.2,
            ),

            paste_bg: Color::Rgb(
                $crate::themes::codex_dark::SUNKEN.0,
                $crate::themes::codex_dark::SUNKEN.1,
                $crate::themes::codex_dark::SUNKEN.2,
            ),
            paste_fg: Color::Rgb(
                $crate::themes::codex_dark::FG_SECONDARY.0,
                $crate::themes::codex_dark::FG_SECONDARY.1,
                $crate::themes::codex_dark::FG_SECONDARY.2,
            ),
            paste_dim: Color::Rgb(
                $crate::themes::codex_dark::GRAY_DIM.0,
                $crate::themes::codex_dark::GRAY_DIM.1,
                $crate::themes::codex_dark::GRAY_DIM.2,
            ),

            md_heading_h1: Color::Rgb(
                $crate::themes::codex_dark::ACCENT.0,
                $crate::themes::codex_dark::ACCENT.1,
                $crate::themes::codex_dark::ACCENT.2,
            ),
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: Color::Rgb(
                $crate::themes::codex_dark::TEAL.0,
                $crate::themes::codex_dark::TEAL.1,
                $crate::themes::codex_dark::TEAL.2,
            ),
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: Color::Rgb(
                $crate::themes::codex_dark::ORANGE.0,
                $crate::themes::codex_dark::ORANGE.1,
                $crate::themes::codex_dark::ORANGE.2,
            ),
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: Color::Rgb(
                $crate::themes::codex_dark::RED.0,
                $crate::themes::codex_dark::RED.1,
                $crate::themes::codex_dark::RED.2,
            ),
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: Color::Rgb(
                $crate::themes::codex_dark::GREEN.0,
                $crate::themes::codex_dark::GREEN.1,
                $crate::themes::codex_dark::GREEN.2,
            ),
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: Color::Rgb(
                $crate::themes::codex_dark::PURPLE.0,
                $crate::themes::codex_dark::PURPLE.1,
                $crate::themes::codex_dark::PURPLE.2,
            ),
            md_heading_h6_mod: Modifier::empty(),
            md_code: Color::Rgb(
                $crate::themes::codex_dark::TEAL.0,
                $crate::themes::codex_dark::TEAL.1,
                $crate::themes::codex_dark::TEAL.2,
            ),
            md_task_checked: Color::Rgb(
                $crate::themes::codex_dark::GREEN.0,
                $crate::themes::codex_dark::GREEN.1,
                $crate::themes::codex_dark::GREEN.2,
            ),
            md_task_unchecked: Color::Rgb(
                $crate::themes::codex_dark::ACCENT.0,
                $crate::themes::codex_dark::ACCENT.1,
                $crate::themes::codex_dark::ACCENT.2,
            ),
            md_muted: Color::Rgb(
                $crate::themes::codex_dark::GRAY.0,
                $crate::themes::codex_dark::GRAY.1,
                $crate::themes::codex_dark::GRAY.2,
            ),
            md_code_bg: Color::Rgb(
                $crate::themes::codex_dark::SURFACE.0,
                $crate::themes::codex_dark::SURFACE.1,
                $crate::themes::codex_dark::SURFACE.2,
            ),
            md_text: Color::Rgb(
                $crate::themes::codex_dark::FG.0,
                $crate::themes::codex_dark::FG.1,
                $crate::themes::codex_dark::FG.2,
            ),
            link_fg: Color::Rgb(
                $crate::themes::codex_dark::ACCENT.0,
                $crate::themes::codex_dark::ACCENT.1,
                $crate::themes::codex_dark::ACCENT.2,
            ),
        }
    };
}

/// Expand to a full `Theme { ... }` struct literal for ItermGreen.
///
/// Requires `Theme`, `Color`, and `Modifier` in scope at the expansion site.
/// Valid inside a `const fn` (const tuple field access).
#[macro_export]
macro_rules! iterm_green_theme {
    () => {
        Theme {
            bg_base: Color::Rgb(
                $crate::themes::iterm_green::BASE.0,
                $crate::themes::iterm_green::BASE.1,
                $crate::themes::iterm_green::BASE.2,
            ),
            bg_terminal: Color::Rgb(
                $crate::themes::iterm_green::BASE.0,
                $crate::themes::iterm_green::BASE.1,
                $crate::themes::iterm_green::BASE.2,
            ),
            bg_light: Color::Rgb(
                $crate::themes::iterm_green::ELEVATED.0,
                $crate::themes::iterm_green::ELEVATED.1,
                $crate::themes::iterm_green::ELEVATED.2,
            ),
            bg_dark: Color::Rgb(
                $crate::themes::iterm_green::SURFACE.0,
                $crate::themes::iterm_green::SURFACE.1,
                $crate::themes::iterm_green::SURFACE.2,
            ),
            bg_highlight: Color::Rgb(
                $crate::themes::iterm_green::ELEVATED.0,
                $crate::themes::iterm_green::ELEVATED.1,
                $crate::themes::iterm_green::ELEVATED.2,
            ),
            bg_hover: Color::Rgb(
                $crate::themes::iterm_green::HOVER.0,
                $crate::themes::iterm_green::HOVER.1,
                $crate::themes::iterm_green::HOVER.2,
            ),

            accent_user: Color::Rgb(
                $crate::themes::iterm_green::WHITE.0,
                $crate::themes::iterm_green::WHITE.1,
                $crate::themes::iterm_green::WHITE.2,
            ),
            accent_assistant: Color::Rgb(
                $crate::themes::iterm_green::CURSOR.0,
                $crate::themes::iterm_green::CURSOR.1,
                $crate::themes::iterm_green::CURSOR.2,
            ),
            accent_thinking: Color::Rgb(
                $crate::themes::iterm_green::CYAN.0,
                $crate::themes::iterm_green::CYAN.1,
                $crate::themes::iterm_green::CYAN.2,
            ),
            accent_tool: Color::Rgb(
                $crate::themes::iterm_green::BR_BLACK.0,
                $crate::themes::iterm_green::BR_BLACK.1,
                $crate::themes::iterm_green::BR_BLACK.2,
            ),
            accent_system: Color::Rgb(
                $crate::themes::iterm_green::BLUE.0,
                $crate::themes::iterm_green::BLUE.1,
                $crate::themes::iterm_green::BLUE.2,
            ),
            accent_error: Color::Rgb(
                $crate::themes::iterm_green::RED.0,
                $crate::themes::iterm_green::RED.1,
                $crate::themes::iterm_green::RED.2,
            ),
            accent_success: Color::Rgb(
                $crate::themes::iterm_green::GREEN.0,
                $crate::themes::iterm_green::GREEN.1,
                $crate::themes::iterm_green::GREEN.2,
            ),
            accent_running: Color::Rgb(
                $crate::themes::iterm_green::CURSOR.0,
                $crate::themes::iterm_green::CURSOR.1,
                $crate::themes::iterm_green::CURSOR.2,
            ),
            accent_skill: Color::Rgb(
                $crate::themes::iterm_green::BLUE.0,
                $crate::themes::iterm_green::BLUE.1,
                $crate::themes::iterm_green::BLUE.2,
            ),

            text_primary: Color::Rgb(
                $crate::themes::iterm_green::FG.0,
                $crate::themes::iterm_green::FG.1,
                $crate::themes::iterm_green::FG.2,
            ),
            text_secondary: Color::Rgb(
                $crate::themes::iterm_green::FG_DIM.0,
                $crate::themes::iterm_green::FG_DIM.1,
                $crate::themes::iterm_green::FG_DIM.2,
            ),

            gray_dim: Color::Rgb(
                $crate::themes::iterm_green::GREEN_DARK.0,
                $crate::themes::iterm_green::GREEN_DARK.1,
                $crate::themes::iterm_green::GREEN_DARK.2,
            ),
            gray: Color::Rgb(
                $crate::themes::iterm_green::GREEN_MID.0,
                $crate::themes::iterm_green::GREEN_MID.1,
                $crate::themes::iterm_green::GREEN_MID.2,
            ),
            gray_bright: Color::Rgb(
                $crate::themes::iterm_green::GREEN_BRIGHT.0,
                $crate::themes::iterm_green::GREEN_BRIGHT.1,
                $crate::themes::iterm_green::GREEN_BRIGHT.2,
            ),

            command: Color::Rgb(
                $crate::themes::iterm_green::YELLOW.0,
                $crate::themes::iterm_green::YELLOW.1,
                $crate::themes::iterm_green::YELLOW.2,
            ),
            path: Color::Rgb(
                $crate::themes::iterm_green::BLUE.0,
                $crate::themes::iterm_green::BLUE.1,
                $crate::themes::iterm_green::BLUE.2,
            ),
            running: Color::Rgb(
                $crate::themes::iterm_green::CYAN.0,
                $crate::themes::iterm_green::CYAN.1,
                $crate::themes::iterm_green::CYAN.2,
            ),
            warning: Color::Rgb(
                $crate::themes::iterm_green::YELLOW.0,
                $crate::themes::iterm_green::YELLOW.1,
                $crate::themes::iterm_green::YELLOW.2,
            ),

            fuzzy_accent: Color::Rgb(
                $crate::themes::iterm_green::BLUE.0,
                $crate::themes::iterm_green::BLUE.1,
                $crate::themes::iterm_green::BLUE.2,
            ),

            accent_plan: Color::Rgb(
                $crate::themes::iterm_green::YELLOW.0,
                $crate::themes::iterm_green::YELLOW.1,
                $crate::themes::iterm_green::YELLOW.2,
            ),

            accent_verify: Color::Rgb(
                $crate::themes::iterm_green::MAGENTA.0,
                $crate::themes::iterm_green::MAGENTA.1,
                $crate::themes::iterm_green::MAGENTA.2,
            ),

            accent_remember: Color::Rgb(
                $crate::themes::iterm_green::GREEN.0,
                $crate::themes::iterm_green::GREEN.1,
                $crate::themes::iterm_green::GREEN.2,
            ),

            selection_border: Color::Rgb(
                $crate::themes::iterm_green::BORDER.0,
                $crate::themes::iterm_green::BORDER.1,
                $crate::themes::iterm_green::BORDER.2,
            ),
            prompt_border: Color::Rgb(
                $crate::themes::iterm_green::BORDER_SUBTLE.0,
                $crate::themes::iterm_green::BORDER_SUBTLE.1,
                $crate::themes::iterm_green::BORDER_SUBTLE.2,
            ),
            prompt_border_active: Color::Rgb(
                $crate::themes::iterm_green::BORDER_ACTIVE.0,
                $crate::themes::iterm_green::BORDER_ACTIVE.1,
                $crate::themes::iterm_green::BORDER_ACTIVE.2,
            ),
            hover_border: Color::Rgb(
                $crate::themes::iterm_green::BORDER_SUBTLE.0,
                $crate::themes::iterm_green::BORDER_SUBTLE.1,
                $crate::themes::iterm_green::BORDER_SUBTLE.2,
            ),

            accent_model: Color::Rgb(
                $crate::themes::iterm_green::BR_CYAN.0,
                $crate::themes::iterm_green::BR_CYAN.1,
                $crate::themes::iterm_green::BR_CYAN.2,
            ),

            scrollbar_bg: Color::Rgb(
                $crate::themes::iterm_green::SUNKEN.0,
                $crate::themes::iterm_green::SUNKEN.1,
                $crate::themes::iterm_green::SUNKEN.2,
            ),
            scrollbar_fg: Color::Rgb(
                $crate::themes::iterm_green::BORDER.0,
                $crate::themes::iterm_green::BORDER.1,
                $crate::themes::iterm_green::BORDER.2,
            ),

            diff_delete_bg: Color::Rgb(
                $crate::themes::iterm_green::DIFF_DEL_BG.0,
                $crate::themes::iterm_green::DIFF_DEL_BG.1,
                $crate::themes::iterm_green::DIFF_DEL_BG.2,
            ),
            diff_delete_fg: Color::Rgb(
                $crate::themes::iterm_green::RED.0,
                $crate::themes::iterm_green::RED.1,
                $crate::themes::iterm_green::RED.2,
            ),
            diff_insert_bg: Color::Rgb(
                $crate::themes::iterm_green::DIFF_INS_BG.0,
                $crate::themes::iterm_green::DIFF_INS_BG.1,
                $crate::themes::iterm_green::DIFF_INS_BG.2,
            ),
            diff_insert_fg: Color::Rgb(
                $crate::themes::iterm_green::GREEN.0,
                $crate::themes::iterm_green::GREEN.1,
                $crate::themes::iterm_green::GREEN.2,
            ),
            diff_equal_fg: Color::Rgb(
                $crate::themes::iterm_green::GREEN_MID.0,
                $crate::themes::iterm_green::GREEN_MID.1,
                $crate::themes::iterm_green::GREEN_MID.2,
            ),
            diff_gutter_fg: Color::Rgb(
                $crate::themes::iterm_green::GREEN_DARK.0,
                $crate::themes::iterm_green::GREEN_DARK.1,
                $crate::themes::iterm_green::GREEN_DARK.2,
            ),

            bg_visual: Color::Rgb(
                $crate::themes::iterm_green::SELECTION.0,
                $crate::themes::iterm_green::SELECTION.1,
                $crate::themes::iterm_green::SELECTION.2,
            ),

            paste_bg: Color::Rgb(
                $crate::themes::iterm_green::SURFACE.0,
                $crate::themes::iterm_green::SURFACE.1,
                $crate::themes::iterm_green::SURFACE.2,
            ),
            paste_fg: Color::Rgb(
                $crate::themes::iterm_green::FG.0,
                $crate::themes::iterm_green::FG.1,
                $crate::themes::iterm_green::FG.2,
            ),
            paste_dim: Color::Rgb(
                $crate::themes::iterm_green::GREEN_DARK.0,
                $crate::themes::iterm_green::GREEN_DARK.1,
                $crate::themes::iterm_green::GREEN_DARK.2,
            ),

            md_heading_h1: Color::Rgb(
                $crate::themes::iterm_green::CURSOR.0,
                $crate::themes::iterm_green::CURSOR.1,
                $crate::themes::iterm_green::CURSOR.2,
            ),
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: Color::Rgb(
                $crate::themes::iterm_green::BLUE.0,
                $crate::themes::iterm_green::BLUE.1,
                $crate::themes::iterm_green::BLUE.2,
            ),
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: Color::Rgb(
                $crate::themes::iterm_green::MAGENTA.0,
                $crate::themes::iterm_green::MAGENTA.1,
                $crate::themes::iterm_green::MAGENTA.2,
            ),
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: Color::Rgb(
                $crate::themes::iterm_green::CYAN.0,
                $crate::themes::iterm_green::CYAN.1,
                $crate::themes::iterm_green::CYAN.2,
            ),
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: Color::Rgb(
                $crate::themes::iterm_green::GREEN_BRIGHT.0,
                $crate::themes::iterm_green::GREEN_BRIGHT.1,
                $crate::themes::iterm_green::GREEN_BRIGHT.2,
            ),
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: Color::Rgb(
                $crate::themes::iterm_green::GREEN_MID.0,
                $crate::themes::iterm_green::GREEN_MID.1,
                $crate::themes::iterm_green::GREEN_MID.2,
            ),
            md_heading_h6_mod: Modifier::empty(),
            md_code: Color::Rgb(
                $crate::themes::iterm_green::BR_CYAN.0,
                $crate::themes::iterm_green::BR_CYAN.1,
                $crate::themes::iterm_green::BR_CYAN.2,
            ),
            md_task_checked: Color::Rgb(
                $crate::themes::iterm_green::GREEN.0,
                $crate::themes::iterm_green::GREEN.1,
                $crate::themes::iterm_green::GREEN.2,
            ),
            md_task_unchecked: Color::Rgb(
                $crate::themes::iterm_green::FG_DIM.0,
                $crate::themes::iterm_green::FG_DIM.1,
                $crate::themes::iterm_green::FG_DIM.2,
            ),
            md_muted: Color::Rgb(
                $crate::themes::iterm_green::GREEN_MID.0,
                $crate::themes::iterm_green::GREEN_MID.1,
                $crate::themes::iterm_green::GREEN_MID.2,
            ),
            md_code_bg: Color::Rgb(
                $crate::themes::iterm_green::SURFACE.0,
                $crate::themes::iterm_green::SURFACE.1,
                $crate::themes::iterm_green::SURFACE.2,
            ),
            md_text: Color::Rgb(
                $crate::themes::iterm_green::FG.0,
                $crate::themes::iterm_green::FG.1,
                $crate::themes::iterm_green::FG.2,
            ),
            link_fg: Color::Rgb(
                $crate::themes::iterm_green::BR_BLUE.0,
                $crate::themes::iterm_green::BR_BLUE.1,
                $crate::themes::iterm_green::BR_BLUE.2,
            ),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{codex_dark, iterm_green};

    fn lum(c: (u8, u8, u8)) -> i32 {
        c.0 as i32 + c.1 as i32 + c.2 as i32
    }

    /// Canvas, accent, and foreground must match `GhosttyOverrides.conf` /
    /// `AccentColor.colorset` verbatim — drifting from them is what this
    /// theme exists to prevent.
    #[test]
    fn codex_dark_matches_the_ghostty_profile() {
        assert_eq!(codex_dark::BASE, (0x18, 0x18, 0x18));
        assert_eq!(codex_dark::FG, (0xFA, 0xF3, 0xDD));
        assert_eq!(codex_dark::ACCENT, (0x33, 0x9C, 0xFF));
        assert_eq!(codex_dark::SELECTION, (0x2A, 0x35, 0x3A));
    }

    /// Elevations are read against the canvas, so each must be lighter
    /// than it, and `SUNKEN` must be darker than `BASE`.
    #[test]
    fn codex_dark_elevations_sit_above_the_canvas() {
        let base = lum(codex_dark::BASE);
        for (name, color) in [
            ("SURFACE", codex_dark::SURFACE),
            ("ELEVATED", codex_dark::ELEVATED),
            ("HOVER", codex_dark::HOVER),
        ] {
            assert!(lum(color) > base, "{name} must be lighter than BASE");
        }
        assert!(
            lum(codex_dark::SUNKEN) < base,
            "SUNKEN must be darker than BASE"
        );
    }

    /// The canvas is painted on purpose: an unpainted canvas is the same
    /// surface a multiplexer's chrome draws on, leaving no visible seam
    /// between the two. It must stay the profile background exactly, so the
    /// hue matches the translucent chrome around it.
    #[test]
    fn iterm_green_canvas_is_the_painted_profile_background() {
        assert_eq!(iterm_green::BASE, (0x16, 0x0C, 0x2A));
        assert_eq!(iterm_green::SUNKEN, (0x16, 0x0C, 0x2A));
    }

    /// Elevations are read against the canvas, so each must be lighter than it.
    #[test]
    fn iterm_green_elevations_sit_above_the_canvas() {
        let base = lum(iterm_green::BASE);
        for (name, color) in [
            ("SURFACE", iterm_green::SURFACE),
            ("ELEVATED", iterm_green::ELEVATED),
            ("HOVER", iterm_green::HOVER),
        ] {
            assert!(lum(color) > base, "{name} must be lighter than BASE");
        }
    }

    /// Foreground and cursor are the iTerm2 profile values verbatim; drifting
    /// from them is what this theme exists to prevent.
    #[test]
    fn iterm_green_foreground_matches_the_iterm_profile() {
        assert_eq!(iterm_green::FG, (0x76, 0xE7, 0x65));
        assert_eq!(iterm_green::CURSOR, (0x78, 0xF9, 0x4C));
        assert_eq!(iterm_green::SELECTION, (0x36, 0x39, 0x83));
    }
}
