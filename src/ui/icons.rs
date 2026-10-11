/// Icon set: VS Code codicons served from the installed Symbols Nerd Font.
/// Every codepoint below was verified present in
/// `/usr/share/fonts/TTF/SymbolsNerdFont-Regular.ttf` — never guess PUA
/// values, since the Private Use Area is unstandardized and collisions
/// render the wrong glyph. Render icon spans with `font_family(FAMILY)`
/// so resolution never depends on fallback order.
pub struct Icons;

impl Icons {
    pub const FAMILY: &str = "Symbols Nerd Font";

    pub const FOLDER: &'static str = "\u{ea83}"; // cod-folder
    pub const FOLDER_OPEN: &'static str = "\u{eaf7}"; // cod-folder_opened
    pub const FILE: &'static str = "\u{ea7b}"; // cod-file
    pub const FILE_CODE: &'static str = "\u{eae9}"; // cod-file_code
    pub const FILE_ARCHIVE: &'static str = "\u{eaef}"; // cod-file_zip
    pub const FILE_IMAGE: &'static str = "\u{eaea}"; // cod-file_media
    pub const SYMLINK: &'static str = "\u{eaee}"; // cod-file_symlink_file
    pub const DRIVE: &'static str = "\u{eace}"; // cod-database
    pub const NETWORK: &'static str = "\u{eb3a}"; // cod-remote
    pub const UP_ARROW: &'static str = "\u{eaa1}"; // cod-arrow_up
    pub const REFRESH: &'static str = "\u{eb37}"; // cod-refresh
    pub const CONNECT: &'static str = "\u{eb2d}"; // cod-plug
    pub const DISCONNECT: &'static str = "\u{ead0}"; // cod-debug_disconnect
    pub const DISCONNECTED: &'static str = "\u{eabd}"; // cod-circle_slash
    pub const DELETE: &'static str = "\u{ea81}"; // cod-trash
    pub const EDIT: &'static str = "\u{ea73}"; // cod-edit
    pub const COPY: &'static str = "\u{ebcc}"; // cod-copy
    pub const NEW_FOLDER: &'static str = "\u{ea80}"; // cod-new_folder
    pub const BOOKMARK: &'static str = "\u{eaa5}"; // cod-bookmark
    pub const TRANSFER_RIGHT: &'static str = "\u{ea9c}"; // cod-arrow_right
    pub const TRANSFER_LEFT: &'static str = "\u{ea9b}"; // cod-arrow_left
    pub const CLOSE: &'static str = "\u{ea76}"; // cod-close
    pub const CHECK: &'static str = "\u{eab2}"; // cod-check
    pub const WARNING: &'static str = "\u{ea6c}"; // cod-warning
    pub const SORT_ASC: &'static str = " \u{eab7}"; // cod-chevron_up
    pub const SORT_DESC: &'static str = " \u{eab6}"; // cod-chevron_down
    pub const SEARCH: &'static str = "\u{ea6d}"; // cod-search
    pub const CHEVRON_LEFT: &'static str = "\u{eab5}"; // cod-chevron_left
    pub const CHEVRON_RIGHT: &'static str = "\u{eab6}"; // cod-chevron_right
    pub const HIDDEN: &'static str = ".*";
    pub const PLAY: &'static str = "\u{eb2c}"; // cod-play
    pub const DOT: &'static str = "\u{ea71}"; // cod-circle_filled
    pub const DOT_HOLLOW: &'static str = "\u{eabc}"; // cod-circle

    pub fn for_file(ext: &str) -> &'static str {
        // Extensions arrive lowercased from the loaders: plain match, no alloc.
        match ext {
            "zip" | "tar" | "gz" | "7z" | "rar" | "bz2" | "xz" => Self::FILE_ARCHIVE,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" => Self::FILE_IMAGE,
            "rs" | "go" | "c" | "cpp" | "h" | "py" | "js" | "ts" | "json" | "toml" | "yaml" | "sh" | "html" | "css" => Self::FILE_CODE,
            _ => Self::FILE,
        }
    }
}
