use crate::fs::types::FileEntry;
use super::pane_state::PaneId;
use gpui::{Pixels, Point};

/// Data-driven context-menu actions shared by mouse and keyboard.
/// Visuals (icon/label/shortcut) derive from the action in the view;
/// execution lives in `RiftApp::execute_menu_action`.
#[derive(Debug, Clone)]
pub enum MenuAction {
    OpenDirectory { path: String },
    OpenFile,
    Transfer,
    Properties,
    Rename { name: String },
    Delete,
    CopyPath { path: String },
    CopyName { name: String },
    AddBookmark { path: String, label: String, is_remote: bool },
    NewFolder,
    BookmarkFolder { path: String, label: String, is_remote: bool },
    SelectAll,
    Refresh,
    GoParent,
    ToggleHidden,
}

impl MenuAction {
    /// Stable key for animations and element ids.
    pub fn key(&self) -> &'static str {
        match self {
            MenuAction::OpenDirectory { .. } => "open",
            MenuAction::OpenFile => "open",
            MenuAction::Transfer => "transfer",
            MenuAction::Properties => "properties",
            MenuAction::Rename { .. } => "rename",
            MenuAction::Delete => "delete",
            MenuAction::CopyPath { .. } => "copy-path",
            MenuAction::CopyName { .. } => "copy-name",
            MenuAction::AddBookmark { .. } => "add-bookmark",
            MenuAction::NewFolder => "new-folder",
            MenuAction::BookmarkFolder { .. } => "bookmark-folder",
            MenuAction::SelectAll => "select-all",
            MenuAction::Refresh => "refresh",
            MenuAction::GoParent => "go-parent",
            MenuAction::ToggleHidden => "toggle-hidden",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub position: Point<Pixels>,
    pub target_pane: PaneId,
    pub target_entry: Option<FileEntry>,
    pub selected_index: Option<usize>,
    /// Actionable rows in display order (separators/headers excluded).
    /// Keyboard navigation and click dispatch both run off this list.
    pub items: Vec<(MenuAction, bool)>,
}

impl ContextMenuState {
    pub fn select_first_enabled(&mut self) {
        self.selected_index = self.items.iter().position(|(_, enabled)| *enabled);
    }

    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let start = self.selected_index.map(|i| i + 1).unwrap_or(0);
        for step in 0..self.items.len() {
            let idx = (start + step) % self.items.len();
            if self.items[idx].1 {
                self.selected_index = Some(idx);
                return;
            }
        }
    }

    pub fn select_prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let start = self
            .selected_index
            .map(|i| (i + self.items.len() - 1) % self.items.len())
            .unwrap_or(self.items.len() - 1);
        for step in 0..self.items.len() {
            let idx = (start + self.items.len() - step) % self.items.len();
            if self.items[idx].1 {
                self.selected_index = Some(idx);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
        use super::*;

        fn enabled_items(n: usize, disabled: &[usize]) -> ContextMenuState {
            ContextMenuState {
                position: Point::default(),
                target_pane: PaneId::Left,
                target_entry: None,
                selected_index: None,
                items: (0..n)
                    .map(|i| {
                        (
                            MenuAction::Refresh,
                            !disabled.contains(&i),
                        )
                    })
                    .collect(),
            }
        }

        #[test]
        fn test_menu_keyboard_nav_skips_disabled_and_wraps() {
            let mut menu = enabled_items(4, &[0, 2]);
            menu.select_first_enabled();
            assert_eq!(menu.selected_index, Some(1));
            menu.select_next();
            assert_eq!(menu.selected_index, Some(3));
            menu.select_next();
            assert_eq!(menu.selected_index, Some(1)); // wraps, skips disabled
            menu.select_prev();
            assert_eq!(menu.selected_index, Some(3));
        }

        #[test]
        fn test_menu_nav_all_disabled_stays_none() {
            let mut menu = enabled_items(2, &[0, 1]);
            menu.select_first_enabled();
            assert_eq!(menu.selected_index, None);
            menu.select_next();
            assert_eq!(menu.selected_index, None);
        }
    }
