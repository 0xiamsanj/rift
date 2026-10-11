use gpui::{px, rgb, Pixels, Rgba};

/// Design tokens modeled on Zed's `ThemeColors` architecture
/// (`crates/theme/src/styles/colors.rs`) with values extracted from Zed's
/// official One Dark / One Light definitions (`assets/themes/one/one.json`).
///
/// Rift-specific additions (badges, keycaps, file-type hues) follow the same
/// vocabulary and are marked below. Type stays compact and quiet per
/// project direction; Zed's 16/15px defaults are starting points, not law.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    // Surfaces (Zed names)
    pub background: Rgba,
    pub surface_background: Rgba,
    pub elevated_surface_background: Rgba,
    pub tab_bar_background: Rgba,
    pub title_bar_background: Rgba,
    pub toolbar_background: Rgba,
    pub status_bar_background: Rgba,
    pub panel_background: Rgba,
    pub element_background: Rgba,
    pub element_hover: Rgba,
    pub element_active: Rgba,
    pub element_selected: Rgba,
    pub ghost_element_hover: Rgba,
    // Severity tints (Rift pattern on Zed's info/success/warning/error hues)
    pub bg_badge_accent: Rgba,
    pub bg_badge_success: Rgba,
    pub bg_badge_warning: Rgba,
    pub bg_badge_danger: Rgba,
    pub keycap_bg: Rgba,
    pub keycap_border: Rgba,
    pub keycap_text: Rgba,
    // Borders (Zed names)
    pub border: Rgba,
    pub border_variant: Rgba,
    pub border_focused: Rgba,
    pub border_selected: Rgba,
    // Text (Zed names)
    pub text: Rgba,
    pub text_placeholder: Rgba,
    pub text_muted: Rgba,
    pub text_disabled: Rgba,
    pub text_accent: Rgba,
    // Interactive + semantic
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
    pub danger_hover: Rgba,
    // File-type hues (Rift convention on Zed syntax hues)
    pub dir_icon: Rgba,
    pub file_icon: Rgba,
    pub syntax_purple: Rgba,
    pub syntax_cyan: Rgba,

    // Theme mode flag (avoids fragile color comparisons to detect dark/light)
    pub is_dark: bool,

    // Standardized modal widths (uiux §6: consistent modal widths/padding)
    pub modal_w_sm: Pixels, // Small forms: new folder, rename, delete, conflict, bookmark
    pub modal_w_md: Pixels, // Medium forms: connect, host-key verification

    // Zed Typography Families
    pub font_ui: &'static str,
    pub font_mono: &'static str,

    // Compact typographic scale: small and quiet over large.
    // Compact-first rule: design_plan.md size tables are advisory only;
    // explicit compact direction from review always wins.
    pub font_size_modal_title: Pixels, // 12.0px - Modal dialog titles
    pub font_size_body: Pixels,        // 11.0px - File and directory names, input fields
    pub font_size_ui: Pixels,          // 10.0px - Toolbar actions, dialog buttons
    pub font_size_breadcrumbs: Pixels, // 10.0px - Breadcrumbs
    pub font_size_caption: Pixels,     // 9.0px - Status bar, footer counts
    pub font_size_shortcut: Pixels,    // 8.0px - Shortcut hints, keycap pills (deliberately secondary)
    pub font_size_table_header: Pixels,// 9.0px - Table column headers
    pub font_size_mono: Pixels,        // 10.0px - Monospace columns (sizes, dates, perms)
}

impl Default for Theme {
    fn default() -> Self {
        Self::zed_dark()
    }
}

impl Theme {
    /// Zed One Dark, exact values from `assets/themes/one/one.json`.
    /// `*_hover` derivations are noted; everything else is verbatim.
    pub fn zed_dark() -> Self {
        Self {
            background: rgb(0x282c33),
            surface_background: rgb(0x21252b),
            elevated_surface_background: rgb(0x282c33),
            tab_bar_background: rgb(0x2f343e),
            title_bar_background: rgb(0x21252b),
            toolbar_background: rgb(0x21252b),
            status_bar_background: rgb(0x3b414d),
            panel_background: rgb(0x2f343e),
            element_background: rgb(0x2c313a),
            element_hover: rgb(0x363c46),
            element_active: rgb(0x454a56),
            element_selected: rgb(0x454a56),
            ghost_element_hover: rgb(0x2f3540),
            bg_badge_accent: gpui::rgba(0x74ade81a),
            bg_badge_success: gpui::rgba(0xa1c1811a),
            bg_badge_warning: gpui::rgba(0xdec1841a),
            bg_badge_danger: gpui::rgba(0xd072771a),
            keycap_bg: rgb(0x363c46),
            keycap_border: rgb(0x464b57),
            keycap_text: rgb(0xa9afbc),
            border: rgb(0x464b57),
            border_variant: rgb(0x363c46),
            border_focused: rgb(0x47679e),
            border_selected: rgb(0x293b5b),
            text: rgb(0xdce0e5),
            text_muted: rgb(0xa9afbc),
            text_placeholder: rgb(0x878a98),
            text_disabled: rgb(0x878a98),
            text_accent: rgb(0x74ade8),
            accent: rgb(0x74ade8),
            accent_hover: rgb(0x8abceb), // derived: lightened accent for hover
            success: rgb(0xa1c181),
            warning: rgb(0xdec184),
            danger: rgb(0xd07277),
            danger_hover: rgb(0xe0858a), // derived: lightened danger for hover
            dir_icon: rgb(0xdec184),     // Rift: folders in Zed's const/number hue
            file_icon: rgb(0x73ade9),    // Rift: files in Zed's function hue
            syntax_purple: rgb(0xb477cf),
            syntax_cyan: rgb(0x6eb4bf),

            is_dark: true,
            modal_w_sm: px(440.0),
            modal_w_md: px(500.0),

            // IBM Plex Sans + IBM Plex Mono verified present
            // (Regular/Medium/SemiBold) via fontconfig; single-family
            // uniformity per design direction. Nerd-font glyphs fall back
            // per-glyph where Plex lacks them.
            font_ui: "IBM Plex Sans",
            font_mono: "IBM Plex Mono",
            font_size_modal_title: px(12.0),
            font_size_body: px(11.0),
            font_size_ui: px(10.0),
            font_size_breadcrumbs: px(10.0),
            font_size_caption: px(9.0),
            font_size_table_header: px(9.0),
            font_size_mono: px(10.0),
            font_size_shortcut: px(8.0),
        }
    }

    /// Zed One Light, exact values from `assets/themes/one/one.json`.
    pub fn zed_light() -> Self {
        Self {
            background: rgb(0xdcdcdd),
            surface_background: rgb(0xebebec),
            elevated_surface_background: rgb(0xebebec),
            tab_bar_background: rgb(0xebebec),
            title_bar_background: rgb(0xdcdcdd),
            toolbar_background: rgb(0xfafafa),
            status_bar_background: rgb(0xdcdcdd),
            panel_background: rgb(0xebebec),
            element_background: rgb(0xebebec),
            element_hover: rgb(0xdfdfe0),
            element_active: rgb(0xcacaca),
            element_selected: rgb(0xcacaca),
            ghost_element_hover: rgb(0xdfdfe0),
            bg_badge_accent: rgb(0xe2e2fa),
            bg_badge_success: rgb(0xdfeadb),
            bg_badge_warning: rgb(0xfaf2e6),
            bg_badge_danger: rgb(0xfbdfd9),
            keycap_bg: rgb(0xdfdfe0),
            keycap_border: rgb(0xc9c9ca),
            keycap_text: rgb(0x58585a),
            border: rgb(0xc9c9ca),
            border_variant: rgb(0xdfdfe0),
            border_focused: rgb(0x7d82e8),
            border_selected: rgb(0xcbcdf6),
            text: rgb(0x242529),
            text_muted: rgb(0x58585a),
            text_placeholder: rgb(0x7e8086),
            text_disabled: rgb(0x7e8086),
            text_accent: rgb(0x5c78e2),
            accent: rgb(0x5c78e2),
            accent_hover: rgb(0x4a63c4), // derived: darkened accent for hover
            success: rgb(0x669f59),
            warning: rgb(0xa48819),
            danger: rgb(0xd36151),
            danger_hover: rgb(0xc14e41), // derived: darkened danger for hover
            dir_icon: rgb(0xa48819),     // Rift: folders in Zed's warning hue
            file_icon: rgb(0x5b79e3),    // Rift: files in Zed's function hue
            syntax_purple: rgb(0xa449ab),
            syntax_cyan: rgb(0x3882b7),

            is_dark: false,
            modal_w_sm: px(440.0),
            modal_w_md: px(500.0),

            font_ui: "IBM Plex Sans",
            font_mono: "IBM Plex Mono",
            font_size_modal_title: px(12.0),
            font_size_body: px(11.0),
            font_size_ui: px(10.0),
            font_size_breadcrumbs: px(10.0),
            font_size_caption: px(9.0),
            font_size_table_header: px(9.0),
            font_size_mono: px(10.0),
            font_size_shortcut: px(8.0),
        }
    }

    pub fn dark() -> Self {
        Self::zed_dark()
    }
}
