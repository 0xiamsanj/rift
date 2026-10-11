use std::collections::HashSet;
use std::sync::Arc;

use crate::fs::path_utils::{build_breadcrumbs, get_parent_path, BreadcrumbItem};
use crate::fs::types::{FileEntry, SortColumn, SortDirection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneId {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct PaneState {
    pub id: PaneId,
    pub is_remote: bool,
    pub current_path: String,
    /// Shared listing: render snapshots `Arc::clone` (pointer copy) instead
    /// of cloning every entry per frame.
    pub entries: Arc<Vec<FileEntry>>,
    /// Breadcrumb trail, rebuilt only when the path changes — never per frame.
    pub cached_crumbs: Vec<BreadcrumbItem>,
    /// Directory/file counts and total bytes, refreshed with the listing.
    /// Selection counts stay live (they must reflect every toggle).
    pub cached_totals: (usize, usize, u64),
    pub filter: String,
    pub selected_items: HashSet<String>,
    pub focused_index: Option<usize>,
    pub sort_column: SortColumn,
    pub sort_direction: SortDirection,
    pub directories_first: bool,
    pub show_hidden: bool,
    pub history_back: Vec<String>,
    pub history_forward: Vec<String>,
    pub recent_dirs: Vec<String>,
    pub is_loading: bool,
    pub error_message: Option<String>,
    pub path_input_active: bool,
    pub path_input_buffer: String,
    pub filter_active: bool,
}

impl PaneState {
    pub fn new(id: PaneId, is_remote: bool, initial_path: String) -> Self {
        let cached_crumbs = build_breadcrumbs(&initial_path, is_remote);
        Self {
            id,
            is_remote,
            current_path: initial_path.clone(),
            entries: Arc::new(Vec::new()),
            cached_crumbs,
            cached_totals: (0, 0, 0),
            filter: String::new(),
            selected_items: HashSet::new(),
            focused_index: None,
            sort_column: SortColumn::Name,
            sort_direction: SortDirection::Ascending,
            directories_first: true,
            show_hidden: false,
            history_back: Vec::new(),
            history_forward: Vec::new(),
            recent_dirs: Vec::new(),
            is_loading: false,
            error_message: None,
            path_input_active: false,
            path_input_buffer: initial_path,
            filter_active: false,
        }
    }

    /// Central path change: updates path, buffer, crumbs together and
    /// drops selection/filter state that belongs to the old directory.
    fn set_path(&mut self, new_path: String) {
        self.current_path = new_path.clone();
        self.path_input_buffer = new_path;
        self.cached_crumbs = build_breadcrumbs(&self.current_path, self.is_remote);
        self.selected_items.clear();
        self.focused_index = None;
        self.filter.clear();
        self.filter_active = false;
    }

    pub fn navigate_to(&mut self, new_path: String) {
        if self.current_path != new_path {
            // Track recent directories (keep last 20, no duplicates)
            self.recent_dirs.retain(|p| p != &self.current_path);
            self.recent_dirs.insert(0, self.current_path.clone());
            if self.recent_dirs.len() > 20 {
                self.recent_dirs.truncate(20);
            }

            self.history_back.push(self.current_path.clone());
            // Cap navigation history: unbounded back/forward stacks would
            // otherwise grow for the lifetime of the session.
            if self.history_back.len() > 100 {
                let excess = self.history_back.len() - 100;
                self.history_back.drain(..excess);
            }
            self.history_forward.clear();
            self.set_path(new_path);
        }
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
    }

    pub fn go_up(&mut self) -> bool {
        if let Some(parent) = get_parent_path(&self.current_path, self.is_remote) {
            self.navigate_to(parent);
            true
        } else {
            false
        }
    }

    pub fn go_back(&mut self) -> bool {
        if let Some(prev) = self.history_back.pop() {
            self.history_forward.push(self.current_path.clone());
            self.set_path(prev);
            true
        } else {
            false
        }
    }

    pub fn go_forward(&mut self) -> bool {
        if let Some(next) = self.history_forward.pop() {
            self.history_back.push(self.current_path.clone());
            self.set_path(next);
            true
        } else {
            false
        }
    }

    pub fn toggle_sort(&mut self, col: SortColumn) {
        if self.sort_column == col {
            self.sort_direction = self.sort_direction.toggled();
        } else {
            self.sort_column = col;
            self.sort_direction = SortDirection::Ascending;
        }
    }

    pub fn select_all(&mut self) {
        self.selected_items.clear();
        for entry in self.entries.iter() {
            self.selected_items.insert(entry.name.clone());
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_items.clear();
        self.focused_index = None;
    }

    pub fn toggle_selection(&mut self, name: &str, index: usize) {
        if self.selected_items.contains(name) {
            self.selected_items.remove(name);
        } else {
            self.selected_items.insert(name.to_string());
        }
        self.focused_index = Some(index);
    }

    pub fn set_single_selection(&mut self, name: &str, index: usize) {
        self.selected_items.clear();
        self.selected_items.insert(name.to_string());
        self.focused_index = Some(index);
    }

    pub fn select_range_to(&mut self, target_idx: usize) {
        let start = self.focused_index.unwrap_or(0);
        let min = start.min(target_idx);
        let max = start.max(target_idx);

        for i in min..=max {
            if let Some(entry) = self.entries.get(i) {
                self.selected_items.insert(entry.name.clone());
            }
        }
        self.focused_index = Some(target_idx);
    }

    pub fn select_prev(&mut self) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        let next_idx = match self.focused_index {
            Some(i) => if i > 0 { i - 1 } else { 0 },
            None => self.entries.len().saturating_sub(1),
        };
        if let Some(entry) = self.entries.get(next_idx) {
            let name = entry.name.clone();
            self.set_single_selection(&name, next_idx);
            Some(next_idx)
        } else {
            None
        }
    }

    pub fn select_next(&mut self) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        let next_idx = match self.focused_index {
            Some(i) => (i + 1).min(self.entries.len().saturating_sub(1)),
            None => 0,
        };
        if let Some(entry) = self.entries.get(next_idx) {
            let name = entry.name.clone();
            self.set_single_selection(&name, next_idx);
            Some(next_idx)
        } else {
            None
        }
    }

    pub fn select_first(&mut self) -> Option<usize> {
        if let Some(entry) = self.entries.first() {
            let name = entry.name.clone();
            self.set_single_selection(&name, 0);
            Some(0)
        } else {
            None
        }
    }

    pub fn select_last(&mut self) -> Option<usize> {
        let idx = self.entries.len().checked_sub(1)?;
        let name = self.entries.get(idx)?.name.clone();
        self.set_single_selection(&name, idx);
        Some(idx)
    }

    pub fn page_up(&mut self) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        let cur = self.focused_index.unwrap_or(0);
        let next_idx = cur.saturating_sub(15);
        if let Some(entry) = self.entries.get(next_idx) {
            let name = entry.name.clone();
            self.set_single_selection(&name, next_idx);
            Some(next_idx)
        } else {
            None
        }
    }

    pub fn page_down(&mut self) -> Option<usize> {
        if self.entries.is_empty() {
            return None;
        }
        let cur = self.focused_index.unwrap_or(0);
        let next_idx = (cur + 15).min(self.entries.len().saturating_sub(1));
        if let Some(entry) = self.entries.get(next_idx) {
            let name = entry.name.clone();
            self.set_single_selection(&name, next_idx);
            Some(next_idx)
        } else {
            None
        }
    }

    pub fn selected_entries(&self) -> Vec<FileEntry> {
        self.entries
            .iter()
            .filter(|e| self.selected_items.contains(&e.name))
            .cloned()
            .collect()
    }

    /// Recomputes directory/file counts and total bytes from the current
    /// listing. Called whenever entries are refreshed; `stats()` reuses it.
    pub fn refresh_totals(&mut self) {
        let mut dir_count = 0;
        let mut file_count = 0;
        let mut total_size = 0;
        for entry in self.entries.iter() {
            if entry.is_dir() {
                dir_count += 1;
            } else {
                file_count += 1;
                total_size += entry.metadata.size;
            }
        }
        self.cached_totals = (dir_count, file_count, total_size);
    }

    pub fn stats(&self) -> (usize, usize, u64, usize, u64) {
        let (dir_count, file_count, total_size) = self.cached_totals;
        let mut selected_count = 0;
        let mut selected_size = 0;

        for entry in self.entries.iter() {
            if self.selected_items.contains(&entry.name) {
                selected_count += 1;
                if !entry.is_dir() {
                    selected_size += entry.metadata.size;
                }
            }
        }

        (dir_count, file_count, total_size, selected_count, selected_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::types::{FileEntry, FileKind, FileMetadata};
    use std::path::PathBuf;

    fn make_test_entry(name: &str, is_dir: bool) -> FileEntry {
        FileEntry::new(
            name.to_string(),
            PathBuf::from(name),
            if is_dir { FileKind::Directory } else { FileKind::File },
            FileMetadata {
                size: 100,
                modified: None,
                is_readonly: false,
                permissions_str: "rw-r--r--".to_string(),
            },
            if is_dir { String::new() } else { "txt".to_string() },
        )
    }

    #[test]
    fn test_keyboard_navigation() {
        let mut pane = PaneState::new(PaneId::Left, false, "/tmp".to_string());
        pane.entries = Arc::new(vec![
            make_test_entry("file1.txt", false),
            make_test_entry("file2.txt", false),
            make_test_entry("file3.txt", false),
        ]);

        // select_first
        assert_eq!(pane.select_first(), Some(0));
        assert_eq!(pane.focused_index, Some(0));
        assert!(pane.selected_items.contains("file1.txt"));

        // select_next
        assert_eq!(pane.select_next(), Some(1));
        assert_eq!(pane.focused_index, Some(1));
        assert!(pane.selected_items.contains("file2.txt"));
        assert!(!pane.selected_items.contains("file1.txt"));

        // select_next to last
        assert_eq!(pane.select_next(), Some(2));
        assert_eq!(pane.focused_index, Some(2));
        assert_eq!(pane.select_next(), Some(2)); // clamp at end

        // select_prev
        assert_eq!(pane.select_prev(), Some(1));
        assert_eq!(pane.focused_index, Some(1));

        // select_last
        assert_eq!(pane.select_last(), Some(2));
        assert_eq!(pane.focused_index, Some(2));
    }

    #[test]
    fn test_range_selection() {
        let mut pane = PaneState::new(PaneId::Left, false, "/tmp".to_string());
        pane.entries = Arc::new(vec![
            make_test_entry("file1.txt", false),
            make_test_entry("file2.txt", false),
            make_test_entry("file3.txt", false),
            make_test_entry("file4.txt", false),
        ]);

        pane.set_single_selection("file1.txt", 0);
        pane.select_range_to(2);

        assert_eq!(pane.selected_items.len(), 3);
        assert!(pane.selected_items.contains("file1.txt"));
        assert!(pane.selected_items.contains("file2.txt"));
        assert!(pane.selected_items.contains("file3.txt"));
        assert!(!pane.selected_items.contains("file4.txt"));
    }

    #[test]
    fn test_cached_crumbs_follow_navigation() {
        let mut pane = PaneState::new(PaneId::Left, false, "/a".to_string());
        assert!(pane.cached_crumbs.iter().any(|c| c.path == "/a"));
        pane.navigate_to("/a/b".to_string());
        assert!(pane.cached_crumbs.iter().any(|c| c.path == "/a/b"));
        assert!(pane.go_back());
        assert!(pane.cached_crumbs.iter().any(|c| c.path == "/a"));
        assert!(!pane.cached_crumbs.iter().any(|c| c.path == "/a/b"));
    }

    #[test]
    fn test_cached_totals_refresh() {
        let mut pane = PaneState::new(PaneId::Left, false, "/tmp".to_string());
        assert_eq!(pane.cached_totals, (0, 0, 0));
        pane.entries = Arc::new(vec![
            make_test_entry("d", true),
            make_test_entry("f.txt", false),
        ]);
        pane.refresh_totals();
        let (dirs, files, size, sel, selsize) = pane.stats();
        assert_eq!((dirs, files), (1, 1));
        assert_eq!(size, 100);
        assert_eq!((sel, selsize), (0, 0));
        pane.set_single_selection("f.txt", 1);
        let (_, _, _, sel2, selsize2) = pane.stats();
        assert_eq!((sel2, selsize2), (1, 100));
    }
}
