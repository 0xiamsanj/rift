pub struct Icons;

impl Icons {
    pub const FOLDER: &'static str = "▸";
    pub const FOLDER_OPEN: &'static str = "▾";
    pub const FILE: &'static str = "▫";
    pub const FILE_CODE: &'static str = "{}";
    pub const FILE_ARCHIVE: &'static str = "▤";
    pub const FILE_IMAGE: &'static str = "◈";
    pub const SYMLINK: &'static str = "↗";
    pub const DRIVE: &'static str = "■";
    pub const NETWORK: &'static str = "✦";
    pub const UP_ARROW: &'static str = "↑";
    pub const REFRESH: &'static str = "⟳";
    pub const CONNECT: &'static str = "✦";
    pub const DISCONNECT: &'static str = "○";
    pub const DELETE: &'static str = "×";
    pub const EDIT: &'static str = "✎";
    pub const COPY: &'static str = "⎘";
    pub const NEW_FOLDER: &'static str = "+";
    pub const BOOKMARK: &'static str = "★";
    pub const TRANSFER_RIGHT: &'static str = "→";
    pub const TRANSFER_LEFT: &'static str = "←";
    pub const CLOSE: &'static str = "✕";
    pub const CHECK: &'static str = "✓";
    pub const WARNING: &'static str = "▲";
    pub const SORT_ASC: &'static str = " ▴";
    pub const SORT_DESC: &'static str = " ▾";
    pub const SEARCH: &'static str = "⌕";
    pub const CHEVRON_LEFT: &'static str = "‹";
    pub const CHEVRON_RIGHT: &'static str = "›";
    pub const HIDDEN: &'static str = ".*";
    pub const PLAY: &'static str = "▶";
    pub const DISCONNECTED: &'static str = "○";
    pub const DOT: &'static str = "●";
    pub const DOT_HOLLOW: &'static str = "○";

    pub fn for_file(ext: &str) -> &'static str {
        match ext.to_lowercase().as_str() {
            "zip" | "tar" | "gz" | "7z" | "rar" | "bz2" | "xz" => Self::FILE_ARCHIVE,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico" => Self::FILE_IMAGE,
            "rs" | "go" | "c" | "cpp" | "h" | "py" | "js" | "ts" | "json" | "toml" | "yaml" | "sh" | "html" | "css" => Self::FILE_CODE,
            _ => Self::FILE,
        }
    }
}
