use super::types::{FileEntry, FileKind, FileMetadata, SortColumn, SortDirection};
use chrono::{DateTime, Local};
use std::fs;
use std::path::Path;

pub struct LocalFs;

impl LocalFs {
    /// Reads entries in a local directory, returning sorted results according to given column & direction.
    pub fn read_dir(
        dir: &Path,
        sort_by: SortColumn,
        direction: SortDirection,
        filter: Option<&str>,
        directories_first: bool,
        show_hidden: bool,
    ) -> std::io::Result<Vec<FileEntry>> {
        let read_dir = fs::read_dir(dir)?;
        let mut entries = Vec::new();

        for item in read_dir {
            let entry = match item {
                Ok(e) => e,
                Err(_) => continue, // Ignore unreadable individual files gracefully
            };

            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files/dirs (dot-prefix on Unix) unless show_hidden is set
            #[cfg(unix)]
            if !show_hidden && name.starts_with('.') {
                continue;
            }

            // Filter check (case-insensitive substring)
            if let Some(query) = filter {
                let q = query.trim().to_lowercase();
                if !q.is_empty() && !name.to_lowercase().contains(&q) {
                    continue;
                }
            }

            let file_type = entry.file_type();
            let kind = match file_type {
                Ok(ft) => {
                    if ft.is_dir() {
                        FileKind::Directory
                    } else if ft.is_symlink() {
                        FileKind::Symlink
                    } else {
                        FileKind::File
                    }
                }
                Err(_) => FileKind::File,
            };

            let extension = if kind == FileKind::Directory {
                String::new()
            } else {
                path.extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default()
            };

            let metadata = match entry.metadata() {
                Ok(m) => {
                    let size = if kind == FileKind::Directory { 0 } else { m.len() };
                    let modified = m.modified().ok().map(DateTime::<Local>::from);
                    let is_readonly = m.permissions().readonly();
                    
                    #[cfg(unix)]
                    let permissions_str = {
                        use std::os::unix::fs::PermissionsExt;
                        format!("{:o}", m.permissions().mode() & 0o777)
                    };
                    #[cfg(windows)]
                    let permissions_str = if is_readonly { "R".to_string() } else { "RW".to_string() };
                    #[cfg(not(any(unix, windows)))]
                    let permissions_str = "-".to_string();

                    FileMetadata {
                        size,
                        modified,
                        is_readonly,
                        permissions_str,
                    }
                }
                Err(_) => FileMetadata {
                    size: 0,
                    modified: None,
                    is_readonly: false,
                    permissions_str: "-".to_string(),
                },
            };

            entries.push(FileEntry::new(name, path, kind, metadata, extension));
        }

        Self::sort_entries(&mut entries, sort_by, direction, directories_first);
        Ok(entries)
    }

    /// Sorts file entries in place.
    pub fn sort_entries(
        entries: &mut [FileEntry],
        sort_by: SortColumn,
        direction: SortDirection,
        directories_first: bool,
    ) {
        // Name/Extension sorts hoist the lowercase keys (one alloc per
        // entry) instead of allocating inside the comparator, then reorder
        // once. Other columns compare inline with no allocation.
        if matches!(sort_by, SortColumn::Name | SortColumn::Extension) {
            let names: Vec<String> = entries.iter().map(|e| e.name.to_lowercase()).collect();
            let mut order: Vec<usize> = (0..entries.len()).collect();
            order.sort_by(|&a, &b| {
                if directories_first {
                    match (entries[a].is_dir(), entries[b].is_dir()) {
                        (true, false) => return std::cmp::Ordering::Less,
                        (false, true) => return std::cmp::Ordering::Greater,
                        _ => {}
                    }
                }
                let mut ord = match sort_by {
                    SortColumn::Name => names[a].cmp(&names[b]),
                    _ => entries[a].extension.cmp(&entries[b].extension),
                };
                // Deterministic tie-break on name so repeated sorts are
                // stable regardless of OS enumeration order.
                if ord == std::cmp::Ordering::Equal && sort_by != SortColumn::Name {
                    ord = names[a].cmp(&names[b]);
                }
                match direction {
                    SortDirection::Ascending => ord,
                    SortDirection::Descending => ord.reverse(),
                }
            });
            let sorted: Vec<FileEntry> = order.into_iter().map(|k| entries[k].clone()).collect();
            entries.clone_from_slice(&sorted);
            return;
        }

        entries.sort_by(|a, b| {
            if directories_first {
                let a_is_dir = a.is_dir();
                let b_is_dir = b.is_dir();
                if a_is_dir && !b_is_dir {
                    return std::cmp::Ordering::Less;
                }
                if !a_is_dir && b_is_dir {
                    return std::cmp::Ordering::Greater;
                }
            }

            let ord = match sort_by {
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::Extension => a.extension.to_lowercase().cmp(&b.extension.to_lowercase()),
                SortColumn::Size => a.metadata.size.cmp(&b.metadata.size),
                SortColumn::Modified => a.metadata.modified.cmp(&b.metadata.modified),
                SortColumn::Permissions => a.metadata.permissions_str.cmp(&b.metadata.permissions_str),
            };

            // Deterministic tie-break on name so repeated sorts are stable
            // regardless of the OS directory-enumeration order.
            let ord = if ord == std::cmp::Ordering::Equal && sort_by != SortColumn::Name {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            } else {
                ord
            };

            match direction {
                SortDirection::Ascending => ord,
                SortDirection::Descending => ord.reverse(),
            }
        });
    }

    /// Creates a directory at the given path.
    pub fn create_dir(path: &Path) -> std::io::Result<()> {
        fs::create_dir_all(path)
    }

    /// Deletes a file or directory recursively.
    pub fn delete_entry(path: &Path) -> std::io::Result<()> {
        if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        }
    }

    /// Renames or moves a file or directory.
    pub fn rename_entry(from: &Path, to: &Path) -> std::io::Result<()> {
        fs::rename(from, to)
    }
}
