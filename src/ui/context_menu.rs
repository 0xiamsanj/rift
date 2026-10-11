use crate::fs::types::FileEntry;
use super::pane_state::PaneId;
use gpui::{Pixels, Point};

#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub position: Point<Pixels>,
    pub target_pane: PaneId,
    pub target_entry: Option<FileEntry>,
    pub selected_index: Option<usize>,
}
