use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    pub size: u64,
    pub modified: Option<DateTime<Local>>,
    pub is_readonly: bool,
    pub permissions_str: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub kind: FileKind,
    pub metadata: FileMetadata,
    /// Lowercased extension, computed once at load so sorting and icon
    /// lookup never allocate per comparison.
    pub extension: String,
    /// Pre-formatted at load time so row rendering is a cheap clone
    /// rather than a format + allocation per frame.
    pub size_str: String,
    pub modified_str: String,
}

impl FileEntry {
    /// Builds an entry, pre-formatting display strings once so row
    /// rendering never formats per frame. `extension` must already be
    /// lowercased (or empty for directories).
    pub fn new(
        name: String,
        path: PathBuf,
        kind: FileKind,
        metadata: FileMetadata,
        extension: String,
    ) -> Self {
        let size_str = if kind == FileKind::Directory {
            "<DIR>".to_string()
        } else {
            format_bytes(metadata.size)
        };
        let modified_str = match metadata.modified {
            Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            None => "-".to_string(),
        };
        Self {
            name,
            path,
            kind,
            metadata,
            extension,
            size_str,
            modified_str,
        }
    }

    pub fn is_dir(&self) -> bool {
        self.kind == FileKind::Directory
    }

    pub fn formatted_size(&self) -> String {
        self.size_str.clone()
    }

    pub fn formatted_modified(&self) -> String {
        self.modified_str.clone()
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_speed(bytes_per_sec: u64) -> String {
    format!("{}/s", format_bytes(bytes_per_sec))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortColumn {
    Name,
    Extension,
    Size,
    Modified,
    Permissions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn toggled(&self) -> Self {
        match self {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        }
    }
}
