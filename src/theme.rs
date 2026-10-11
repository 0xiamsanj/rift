use gpui::{px, rgb, Pixels, Rgba};

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    // Colors
    pub bg_app: Rgba,
    pub bg_pane: Rgba,
    pub bg_header: Rgba,
    pub bg_toolbar: Rgba,
    pub bg_status: Rgba,
    pub bg_card: Rgba,
    pub bg_row_alt: Rgba,
    pub bg_row_hover: Rgba,
    pub bg_selected: Rgba,
    pub bg_modal: Rgba,
    pub bg_input: Rgba,
    pub bg_badge_accent: Rgba,
    pub bg_badge_success: Rgba,
    pub bg_badge_warning: Rgba,
    pub bg_badge_danger: Rgba,
    pub keycap_bg: Rgba,
    pub keycap_border: Rgba,
    pub keycap_text: Rgba,
    pub border: Rgba,
    pub border_focused: Rgba,
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_muted: Rgba,
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
    pub dir_icon: Rgba,
    pub file_icon: Rgba,
    pub syntax_purple: Rgba,
    pub syntax_cyan: Rgba,

    // Zed Typography Families
    pub font_ui: &'static str,
    pub font_mono: &'static str,

    // Zed Exact Typographic Scale
    pub font_size_modal_title: Pixels, // 15.0px - Modal dialog titles
    pub font_size_body: Pixels,        // 13.0px - File and directory names, input fields
    pub font_size_ui: Pixels,          // 12.0px - Toolbar actions, dialog buttons
    pub font_size_breadcrumbs: Pixels, // 11.5px - Breadcrumbs
    pub font_size_caption: Pixels,     // 11.0px - Status bar, footer counts
    pub font_size_table_header: Pixels,// 11.0px - Table column headers
    pub font_size_mono: Pixels,        // 11.5px - Monospace columns (sizes, dates, perms)
}

impl Default for Theme {
    fn default() -> Self {
        Self::zed_dark()
    }
}

impl Theme {
    /// Authentic Zed Editor One Dark inspired theme & typography
    pub fn zed_dark() -> Self {
        Self {
            bg_app: rgb(0x18181d),        // Zed window background / titlebar
            bg_pane: rgb(0x21252b),       // Zed editor & file list surface
            bg_header: rgb(0x1e1e24),     // Zed tab bar / pane header
            bg_toolbar: rgb(0x191c21),    // Zed toolbar / action bar
            bg_status: rgb(0x18181d),     // Zed bottom status bar
            bg_card: rgb(0x1b1d23),       // Elevated card surface
            bg_row_alt: rgb(0x1f2329),    // Subtle alternating row cadence
            bg_row_hover: rgb(0x282c34),  // Zed hover highlight
            bg_selected: rgb(0x2d3a4f),   // Zed file tree selection (refined slate blue)
            bg_modal: rgb(0x21252b),      // Zed command palette modal
            bg_input: rgb(0x16181d),      // Recessed dark input box
            bg_badge_accent: gpui::rgba(0x528bff24),
            bg_badge_success: gpui::rgba(0x98c37924),
            bg_badge_warning: gpui::rgba(0xe5c07b24),
            bg_badge_danger: gpui::rgba(0xe06c7524),
            keycap_bg: rgb(0x282c34),      // Keycap pill background
            keycap_border: rgb(0x383e4a),  // Keycap border
            keycap_text: rgb(0x9da5b4),    // Keycap text
            border: rgb(0x2b2f38),        // Subtle structural border
            border_focused: rgb(0x528bff),// Zed active focus border
            text_primary: rgb(0xd7dae0),  // Zed crisp high-contrast text
            text_secondary: rgb(0xabb2bf),// Zed secondary text
            text_muted: rgb(0x5c6370),    // Zed muted comment gray
            accent: rgb(0x528bff),        // Zed signature blue
            accent_hover: rgb(0x6ba4ff),  // Zed blue hover
            success: rgb(0x98c379),       // Zed One Dark green
            warning: rgb(0xe5c07b),       // Zed One Dark yellow/amber
            danger: rgb(0xe06c75),        // Zed One Dark coral red
            dir_icon: rgb(0xe5c07b),      // Zed warm amber folder icon
            file_icon: rgb(0x61afef),     // Zed cyan/blue file icon
            syntax_purple: rgb(0xc678dd), // Zed syntax purple
            syntax_cyan: rgb(0x56b6c2),   // Zed syntax cyan

            // Typography families & compact scale
            font_ui: "Noto Sans",
            font_mono: "JetBrainsMono Nerd Font",
            font_size_modal_title: px(12.0),
            font_size_body: px(10.5),
            font_size_ui: px(10.0),
            font_size_breadcrumbs: px(9.5),
            font_size_caption: px(9.0),
            font_size_table_header: px(9.0),
            font_size_mono: px(9.5),
        }
    }

    /// Authentic Zed One Light theme
    pub fn zed_light() -> Self {
        Self {
            bg_app: rgb(0xf3f3f5),
            bg_pane: rgb(0xffffff),
            bg_header: rgb(0xeaebed),
            bg_toolbar: rgb(0xeaebed),
            bg_status: rgb(0xe4e5e8),
            bg_card: rgb(0xffffff),
            bg_row_alt: rgb(0xffffff),
            bg_row_hover: rgb(0xebedf0),
            bg_selected: rgb(0xd8e4f8),
            bg_modal: rgb(0xffffff),
            bg_input: rgb(0xf0f1f4),
            bg_badge_accent: gpui::rgba(0x0969da20),
            bg_badge_success: gpui::rgba(0x2da44e20),
            bg_badge_warning: gpui::rgba(0xd4a72c20),
            bg_badge_danger: gpui::rgba(0xcf222e20),
            keycap_bg: rgb(0xebedf0),
            keycap_border: rgb(0xd0d4dc),
            keycap_text: rgb(0x57606a),
            border: rgb(0xd8dbe2),
            border_focused: rgb(0x0969da),
            text_primary: rgb(0x24292e),
            text_secondary: rgb(0x57606a),
            text_muted: rgb(0x8c959f),
            accent: rgb(0x0969da),
            accent_hover: rgb(0x0550ae),
            success: rgb(0x2da44e),
            warning: rgb(0xd4a72c),
            danger: rgb(0xcf222e),
            dir_icon: rgb(0xd4a72c),
            file_icon: rgb(0x0969da),
            syntax_purple: rgb(0x8250df),
            syntax_cyan: rgb(0x0598bc),

            font_ui: "Noto Sans",
            font_mono: "JetBrainsMono Nerd Font",
            font_size_modal_title: px(12.0),
            font_size_body: px(10.5),
            font_size_ui: px(10.0),
            font_size_breadcrumbs: px(9.5),
            font_size_caption: px(9.0),
            font_size_table_header: px(9.0),
            font_size_mono: px(9.5),
        }
    }

    pub fn dark() -> Self {
        Self::zed_dark()
    }
}


