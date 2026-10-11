use std::path::PathBuf;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::*;
use tokio::sync::mpsc;

use crate::bookmarks::{Bookmark, BookmarkStore};
use crate::fs::path_utils::{join_path, list_system_roots};
use crate::fs::types::FileEntry;
use crate::fs::LocalFs;
use crate::remote::client::RemoteClient;
use crate::remote::known_hosts::HostKeyStatus;
use crate::remote::profile::{AuthMethod, ConnectionProfile, ProfileStore};
use crate::theme::Theme;
use crate::transfer::queue::{TransferEvent, TransferManager};
use crate::transfer::types::TransferItem;
use crate::ui::context_menu::{ContextMenuState, MenuAction};
use crate::ui::modals::{
    ActiveModal, AddBookmarkModalState, AuthKind, CommandPaletteModalState,
    ConfirmDeleteModalState, ConnectModalState, HostKeyModalState, NewFolderModalState,
    PropertiesModalState, RenameModalState,
};
use crate::ui::palette::{filter_palette, PaletteItem, PaletteMatch};
use uuid::Uuid;
use crate::ui::pane_state::{PaneId, PaneState};

pub struct RiftApp {
    pub theme: Theme,
    pub left_pane: PaneState,
    pub right_pane: PaneState,
    pub active_pane: PaneId,
    pub left_scroll_handle: UniformListScrollHandle,
    pub right_scroll_handle: UniformListScrollHandle,
    pub drawer_scroll_handle: ScrollHandle,
    pub palette_scroll_handle: ScrollHandle,
    /// Weak self handle so virtualized list rows (which only receive
    /// `&mut App`) can dispatch actions back into the app.
    self_entity: Option<WeakEntity<RiftApp>>,
    pub remote_client: RemoteClient,
    pub transfer_manager: Arc<TransferManager>,
    pub transfer_items: Vec<TransferItem>,
    pub bookmark_store: BookmarkStore,
    pub modal: ActiveModal,
    pub context_menu: Option<ContextMenuState>,
    /// Palette invocation counts for usage-ranked results (Zed history).
    pub palette_usage: std::collections::HashMap<u8, u32>,
    pub show_transfer_drawer: bool,
    pub system_roots: Vec<String>,
    pub status_message: Option<String>,
    pub focus_handle: FocusHandle,
    /// Interface animations (entrance fades, activity pulses). Off = instant
    /// UI for reduced-motion users. See `ui::motion`.
    pub animations_enabled: bool,
}

impl RiftApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let initial_local_path = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());

        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let transfer_manager = Arc::new(TransferManager::new(event_tx));

        // Background listener for transfer updates
        cx.spawn(async move |this, cx| {
            while let Some(event) = event_rx.recv().await {
                let _ = this.update(cx, |app, cx| {
                    app.handle_transfer_event(event, cx);
                    cx.notify();
                });
            }
        })
        .detach();

        let roots = list_system_roots();
        let remote_client = RemoteClient::new();
        let focus_handle = cx.focus_handle();

        let mut app = Self {
            theme: Theme::zed_dark(),
            left_pane: PaneState::new(PaneId::Left, false, initial_local_path.clone()),
            right_pane: PaneState::new(PaneId::Right, true, "/".to_string()),
            active_pane: PaneId::Left,
            left_scroll_handle: UniformListScrollHandle::new(),
            right_scroll_handle: UniformListScrollHandle::new(),
            drawer_scroll_handle: ScrollHandle::new(),
            palette_scroll_handle: ScrollHandle::new(),
            remote_client,
            transfer_manager,
            transfer_items: Vec::new(),
            bookmark_store: BookmarkStore::new(),
            modal: ActiveModal::None,
            context_menu: None,
            palette_usage: std::collections::HashMap::new(),
            show_transfer_drawer: false,
            system_roots: roots,
            status_message: Some("Ready".to_string()),
            focus_handle,
            self_entity: None,
            animations_enabled: true,
        };

        app.refresh_local_pane(PaneId::Left, cx);
        Self::purge_stale_temp_copies(&std::env::temp_dir().join("rift-open"));
        app
    }

    /// Deletes temp copies of remotely opened files older than a day.
    /// "Open" downloads land here and are otherwise never cleaned; a
    /// same-session file is always newer, so only stale files go.
    /// Failures are ignored entry-by-entry: temp hygiene must never
    /// break startup.
    fn purge_stale_temp_copies(dir: &std::path::Path) {
        let cutoff = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(24 * 3600));
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in rd.flatten() {
            let stale = entry
                .metadata()
                .and_then(|m| m.modified())
                .map(|t| cutoff.is_some_and(|c| t < c))
                .unwrap_or(false);
            if stale {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    fn handle_transfer_event(&mut self, event: TransferEvent, cx: &mut Context<Self>) {
        match event {
            TransferEvent::ItemAdded(item) => {
                self.transfer_items.push(item);
                self.show_transfer_drawer = true;
            }
            TransferEvent::ProgressUpdated {
                id,
                transferred_bytes,
                speed_bps,
            } => {
                if let Some(it) = self.transfer_items.iter_mut().find(|i| i.id == id) {
                    it.transferred_bytes = transferred_bytes;
                    it.speed_bps = speed_bps;
                }
            }
            TransferEvent::ItemFinished { id, state } => {
                if let Some(it) = self.transfer_items.iter_mut().find(|i| i.id == id) {
                    it.state = state;
                }
                // Auto-prune history: keep at most 50 finished items so a
                // long session cannot accumulate transfers without bound.
                // (The drawer button still clears them on demand.)
                for removed in self.transfer_manager.prune_finished(50) {
                    self.transfer_items.retain(|i| i.id != removed);
                }
                // Refresh panes if transfer finished successfully
                self.refresh_pane(PaneId::Left, cx);
                self.refresh_pane(PaneId::Right, cx);
            }
            TransferEvent::QueueFinished => {}
        }
    }

    pub fn refresh_pane(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        let is_remote = match pane_id {
            PaneId::Left => self.left_pane.is_remote,
            PaneId::Right => self.right_pane.is_remote,
        };

        if is_remote {
            self.refresh_remote_pane(pane_id, cx);
        } else {
            self.refresh_local_pane(pane_id, cx);
        }
    }

    pub fn refresh_local_pane(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        let pane = self.pane_mut(pane_id);

        // Never block the UI thread on the filesystem: network mounts and
        // slow media can stall synchronous scans for seconds.
        pane.is_loading = true;
        pane.error_message = None;

        let path = PathBuf::from(&pane.current_path);
        let current = pane.current_path.clone();
        let sort_by = pane.sort_column;
        let direction = pane.sort_direction;
        let filter = if pane.filter.is_empty() {
            None
        } else {
            Some(pane.filter.clone())
        };
        let dirs_first = pane.directories_first;
        let show_hidden = pane.show_hidden;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let listed = crate::runtime::spawn_blocking(move || {
                LocalFs::read_dir(
                    &path,
                    sort_by,
                    direction,
                    filter.as_deref(),
                    dirs_first,
                    show_hidden,
                )
            })
            .await
            .map_err(|e| format!("Listing task failed: {e}"))
            .and_then(|inner| inner.map_err(|e| format!("Error: {e}")));
            let _ = this.update(cx, |app, cx| {
                let pane = app.pane_mut(pane_id);
                if pane.current_path != current {
                    return;
                }
                pane.is_loading = false;
                match listed {
                    Ok(entries) => {
                        pane.entries = Arc::new(entries);
                        pane.refresh_totals();
                        Self::prune_selection(pane);
                    }
                    Err(e) => {
                        pane.error_message = Some(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Drops selected names and clamps the cursor after the entry list
    /// changed, so refreshes never leave stale selections behind.
    fn prune_selection(pane: &mut PaneState) {
        let live: std::collections::HashSet<String> =
            pane.entries.iter().map(|e| e.name.clone()).collect();
        pane.selected_items.retain(|n| live.contains(n));
        if let Some(idx) = pane.focused_index {
            if idx >= pane.entries.len() {
                pane.focused_index = None;
            }
        }
    }

    pub fn refresh_remote_pane(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        if !self.remote_client.is_connected() {
            return;
        }

        let pane = self.pane_mut(pane_id);

        pane.is_loading = true;
        pane.error_message = None;

        let remote_path = pane.current_path.clone();
        let sort_by = pane.sort_column;
        let direction = pane.sort_direction;
        let filter = if pane.filter.is_empty() {
            None
        } else {
            Some(pane.filter.clone())
        };
        let dirs_first = pane.directories_first;
        let client = self.remote_client.clone();
        cx.notify();

        // Fetch off the UI thread, then apply the result back on it. The path
        // is re-checked on completion so a slow listing cannot overwrite a
        // newer navigation. A hung server cannot spin the spinner forever:
        // listings time out after 30 s.
        cx.spawn(async move |this, cx| {
            let owned = client.clone();
            let rp = remote_path.clone();
            let res = crate::runtime::timeout(
                std::time::Duration::from_secs(30),
                async move {
                    owned
                        .list_dir(rp, sort_by, direction, filter, dirs_first)
                        .await
                },
            )
            .await
            .map_err(|_| "Remote listing timed out after 30s".to_string())
            .and_then(|inner| inner);
            let _ = this.update(cx, |app, cx| {
                let pane = app.pane_mut(pane_id);
                if pane.current_path != remote_path {
                    return;
                }
                pane.is_loading = false;
                match res {
                    Ok(entries) => {
                        pane.entries = Arc::new(entries);
                        pane.refresh_totals();
                        Self::prune_selection(pane);
                    }
                    Err(e) => {
                        pane.error_message = Some(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn navigate_pane(&mut self, pane_id: PaneId, target: String, cx: &mut Context<Self>) {
        let pane = self.pane_mut(pane_id);
        pane.navigate_to(target);
        self.refresh_pane(pane_id, cx);
    }

    pub fn go_parent(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        let pane = self.pane_mut(pane_id);
        if pane.go_up() {
            self.refresh_pane(pane_id, cx);
        }
    }

    pub fn go_back(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        let pane = self.pane_mut(pane_id);
        if pane.go_back() {
            self.refresh_pane(pane_id, cx);
        }
    }

    pub fn go_forward(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        let pane = self.pane_mut(pane_id);
        if pane.go_forward() {
            self.refresh_pane(pane_id, cx);
        }
    }

    pub fn toggle_hidden(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        let pane = self.pane_mut(pane_id);
        pane.toggle_hidden();
        self.refresh_pane(pane_id, cx);
    }

    pub fn transfer_selected(&mut self, from_pane: PaneId, cx: &mut Context<Self>) {
        let (src_is_remote, src_entries, _src_dir, dst_is_remote, dst_dir) = match from_pane {
            PaneId::Left => (
                self.left_pane.is_remote,
                self.left_pane.selected_entries(),
                self.left_pane.current_path.clone(),
                self.right_pane.is_remote,
                self.right_pane.current_path.clone(),
            ),
            PaneId::Right => (
                self.right_pane.is_remote,
                self.right_pane.selected_entries(),
                self.right_pane.current_path.clone(),
                self.left_pane.is_remote,
                self.left_pane.current_path.clone(),
            ),
        };

        if src_entries.is_empty() {
            self.status_message = Some("No files selected to transfer".to_string());
            return;
        }

        // Upload: Local -> Remote
        if !src_is_remote && dst_is_remote {
            if !self.remote_client.is_connected() {
                self.status_message = Some("Remote server is not connected".to_string());
                return;
            }

            for entry in src_entries {
                if !entry.is_dir() {
                    let remote_target = join_path(&dst_dir, &entry.name, true);
                    self.transfer_manager.enqueue_upload(
                        entry.path,
                        remote_target,
                        self.remote_client.clone(),
                    );
                }
            }
            self.status_message = Some("Upload transfer queued".to_string());
        } else if src_is_remote && !dst_is_remote {
            // Download: Remote -> Local
            for entry in src_entries {
                if !entry.is_dir() {
                    let local_target = PathBuf::from(&dst_dir).join(&entry.name);
                    self.transfer_manager.enqueue_download(
                        entry.path.to_string_lossy().to_string(),
                        local_target,
                        self.remote_client.clone(),
                        entry.metadata.size,
                    );
                }
            }
            self.status_message = Some("Download transfer queued".to_string());
        } else {
            // Local -> Local copy. Never silently overwrite: existing
            // destinations and directories are skipped and reported.
            let mut copied = 0usize;
            let mut skipped = 0usize;
            let mut skipped_dirs = 0usize;
            for entry in src_entries {
                if entry.is_dir() {
                    skipped += 1;
                    skipped_dirs += 1;
                    continue;
                }
                let dest = PathBuf::from(&dst_dir).join(&entry.name);
                if dest.exists() {
                    skipped += 1;
                    continue;
                }
                match std::fs::copy(&entry.path, &dest) {
                    Ok(_) => copied += 1,
                    Err(e) => {
                        skipped += 1;
                        self.status_message =
                            Some(format!("Copy failed for '{}': {e}", entry.name));
                    }
                }
            }
            self.refresh_pane(PaneId::Left, cx);
            self.refresh_pane(PaneId::Right, cx);
            self.status_message = Some(if skipped == 0 {
                format!("Copied {copied} file(s)")
            } else if skipped_dirs > 0 {
                format!(
                    "Copied {copied} file(s), skipped {skipped} (directories cannot be copied yet)"
                )
            } else {
                format!("Copied {copied} file(s), skipped {skipped} (already exist)")
            });
        }
    }

    pub fn delete_selected(&mut self, pane_id: PaneId) {
        let pane = self.pane(pane_id);

        let items: Vec<String> = pane.selected_items.iter().cloned().collect();
        if items.is_empty() {
            return;
        }

        self.modal = ActiveModal::ConfirmDelete(ConfirmDeleteModalState {
            target_pane: pane_id,
            item_names: items,
        });
    }

    pub fn confirm_delete(&mut self, pane_id: PaneId, items: Vec<String>, cx: &mut Context<Self>) {
        let (is_remote, dir) = match pane_id {
            PaneId::Left => (self.left_pane.is_remote, self.left_pane.current_path.clone()),
            PaneId::Right => (self.right_pane.is_remote, self.right_pane.current_path.clone()),
        };

        if !is_remote {
            let mut failed = Vec::new();
            for name in items {
                let p = PathBuf::from(&dir).join(&name);
                if let Err(e) = LocalFs::delete_entry(&p) {
                    failed.push(format!("{name}: {e}"));
                }
            }
            if failed.is_empty() {
                self.status_message = Some("Deleted selected item(s)".to_string());
            } else {
                self.status_message = Some(format!("Delete failed: {}", failed.join("; ")));
            }
            self.refresh_pane(pane_id, cx);
        } else if self.remote_client.is_connected() {
            let client = self.remote_client.clone();
            cx.spawn(async move |this, cx| {
                let mut failed = Vec::new();
                for name in items {
                    let remote_p = join_path(&dir, &name, true);
                    // Files unlink; directories need rmdir. Try both.
                    let res = match client.remove_file(remote_p.clone()).await {
                        Ok(_) => Ok(()),
                        Err(_) => client.remove_dir(remote_p).await,
                    };
                    if let Err(e) = res {
                        failed.push(format!("{name}: {e}"));
                    }
                }
                let _ = this.update(cx, |app, cx| {
                    if failed.is_empty() {
                        app.status_message = Some("Deleted selected item(s)".to_string());
                    } else {
                        app.status_message =
                            Some(format!("Remote delete failed: {}", failed.join("; ")));
                    }
                    app.refresh_pane(pane_id, cx);
                    cx.notify();
                });
            })
            .detach();
        } else {
            self.status_message = Some("Remote server is not connected".to_string());
        }
        self.modal = ActiveModal::None;
    }

    pub fn create_folder(&mut self, pane_id: PaneId, folder_name: &str, cx: &mut Context<Self>) {
        let folder_name = folder_name.trim();
        if folder_name.is_empty() {
            return;
        }
        if !crate::fs::path_utils::is_valid_filename(folder_name) {
            self.status_message = Some(format!(
                "Invalid folder name '{folder_name}': avoid <>:\"/\\|?* and reserved names"
            ));
            return;
        }

        let (is_remote, dir) = match pane_id {
            PaneId::Left => (self.left_pane.is_remote, self.left_pane.current_path.clone()),
            PaneId::Right => (self.right_pane.is_remote, self.right_pane.current_path.clone()),
        };

        if !is_remote {
            let p = PathBuf::from(&dir).join(folder_name);
            match LocalFs::create_dir(&p) {
                Ok(_) => {
                    self.status_message = Some(format!("Created folder '{folder_name}'"));
                    self.refresh_pane(pane_id, cx);
                }
                Err(e) => {
                    self.status_message = Some(format!("Create folder failed: {e}"));
                }
            }
        } else if self.remote_client.is_connected() {
            let client = self.remote_client.clone();
            let remote_p = join_path(&dir, folder_name, true);
            let label = folder_name.to_string();
            cx.spawn(async move |this, cx| {
                let res = client.create_dir(remote_p).await;
                let _ = this.update(cx, |app, cx| {
                    match res {
                        Ok(_) => {
                            app.status_message =
                                Some(format!("Created remote folder '{label}'"));
                        }
                        Err(e) => {
                            app.status_message =
                                Some(format!("Create remote folder failed: {e}"));
                        }
                    }
                    app.refresh_pane(pane_id, cx);
                    cx.notify();
                });
            })
            .detach();
        } else {
            self.status_message = Some("Remote server is not connected".to_string());
        }
        self.modal = ActiveModal::None;
    }

    pub fn rename_entry(&mut self, pane_id: PaneId, old_name: &str, new_name: &str, cx: &mut Context<Self>) {
        let new_name = new_name.trim();
        if new_name.is_empty() || new_name == old_name {
            self.modal = ActiveModal::None;
            return;
        }
        if !crate::fs::path_utils::is_valid_filename(new_name) {
            self.status_message = Some(format!(
                "Invalid name '{new_name}': avoid <>:\"/\\|?* and reserved names"
            ));
            return;
        }

        let (is_remote, dir) = match pane_id {
            PaneId::Left => (self.left_pane.is_remote, self.left_pane.current_path.clone()),
            PaneId::Right => (self.right_pane.is_remote, self.right_pane.current_path.clone()),
        };

        if !is_remote {
            let from = PathBuf::from(&dir).join(old_name);
            let to = PathBuf::from(&dir).join(new_name);
            if to.exists() {
                self.status_message = Some(format!(
                    "'{new_name}' already exists; rename cancelled to avoid overwriting"
                ));
                return;
            }
            match LocalFs::rename_entry(&from, &to) {
                Ok(_) => {
                    self.status_message =
                        Some(format!("Renamed '{old_name}' to '{new_name}'"));
                    self.refresh_pane(pane_id, cx);
                }
                Err(e) => {
                    self.status_message = Some(format!("Rename failed: {e}"));
                }
            }
        } else if self.remote_client.is_connected() {
            let client = self.remote_client.clone();
            let from = join_path(&dir, old_name, true);
            let to = join_path(&dir, new_name, true);
            let label = format!("{old_name} -> {new_name}");
            cx.spawn(async move |this, cx| {
                let res = client.rename(from, to).await;
                let _ = this.update(cx, |app, cx| {
                    match res {
                        Ok(_) => app.status_message = Some(format!("Renamed {label}")),
                        Err(e) => {
                            app.status_message = Some(format!("Remote rename failed: {e}"))
                        }
                    }
                    app.refresh_pane(pane_id, cx);
                    cx.notify();
                });
            })
            .detach();
        } else {
            self.status_message = Some("Remote server is not connected".to_string());
        }
        self.modal = ActiveModal::None;
    }

    /// Keyboard cursor first, then first selected entry, with its pane's
    /// remote flag. Shared by Properties, Open, and copy-path.
    fn focused_entry(&self, pane_id: PaneId) -> Option<(FileEntry, bool)> {
        let pane = self.pane(pane_id);
        pane.focused_index
            .and_then(|i| pane.entries.get(i))
            .or_else(|| {
                pane.selected_items
                    .iter()
                    .filter_map(|n| pane.entries.iter().find(|e| &e.name == n))
                    .next()
            })
            .cloned()
            .map(|e| (e, pane.is_remote))
    }

    pub fn open_properties(&mut self, pane_id: PaneId) {
        match self.focused_entry(pane_id) {
            Some((e, _)) => {
                self.modal = ActiveModal::Properties(PropertiesModalState {
                    name: e.name.clone(),
                    path: e.path.to_string_lossy().to_string(),
                    is_remote: self.pane_is_remote(pane_id),
                    is_dir: e.is_dir(),
                    size_str: e.formatted_size(),
                    modified_str: e.formatted_modified(),
                    permissions: e.metadata.permissions_str.clone(),
                    extension: if e.extension.is_empty() {
                        "-".to_string()
                    } else {
                        e.extension.clone()
                    },
                });
            }
            None => {
                self.status_message = Some("No file selected to inspect".to_string());
            }
        }
    }

    fn pane_is_remote(&self, pane_id: PaneId) -> bool {
        match pane_id {
            PaneId::Left => self.left_pane.is_remote,
            PaneId::Right => self.right_pane.is_remote,
        }
    }

    /// Opens the focused/selected entry: directories navigate in place,
    /// local files launch the OS default application, remote files are
    /// downloaded to a temp copy and opened (one-way; edits are not
    /// uploaded back).
    pub fn open_entry(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        let (entry, is_remote) = match self.focused_entry(pane_id) {
            Some(found) => found,
            None => {
                self.status_message = Some("No file selected to open".to_string());
                cx.notify();
                return;
            }
        };

        if entry.is_dir() {
            let path = if is_remote {
                let dir = match pane_id {
                    PaneId::Left => self.left_pane.current_path.clone(),
                    PaneId::Right => self.right_pane.current_path.clone(),
                };
                join_path(&dir, &entry.name, true)
            } else {
                entry.path.to_string_lossy().to_string()
            };
            self.navigate_pane(pane_id, path, cx);
            cx.notify();
            return;
        }

        if !is_remote {
            let path = entry.path.to_string_lossy().to_string();
            let name = entry.name.clone();
            // Wait for the launcher off the UI thread so zombies are reaped.
            std::thread::spawn(move || {
                let _ = crate::fs::path_utils::default_open_command(&path).status();
            });
            self.status_message = Some(format!("Opened '{name}'"));
            cx.notify();
            return;
        }

        if !self.remote_client.is_connected() {
            self.status_message = Some("Remote server is not connected".to_string());
            cx.notify();
            return;
        }
        let client = self.remote_client.clone();
        let remote_src = entry.path.to_string_lossy().to_string();
        let name = entry.name.clone();
        // Unique temp name: concurrent opens of the same remote file must
        // not share one destination path.
        let local_dst = std::env::temp_dir().join("rift-open").join(format!(
            "{}-{}",
            uuid::Uuid::new_v4(),
            entry.name
        ));
        self.status_message = Some(format!("Downloading '{name}' to open..."));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let dl = client
                .download_file(
                    remote_src,
                    local_dst.clone(),
                    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                    |_| {},
                )
                .await;
            match dl {
                Ok(_) => {
                    let path = local_dst.to_string_lossy().to_string();
                    std::thread::spawn(move || {
                        let _ = crate::fs::path_utils::default_open_command(&path).status();
                    });
                    let _ = this.update(cx, |app, cx| {
                        app.status_message =
                            Some(format!("Opened temp copy of '{name}' (edits are not uploaded)"));
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = this.update(cx, |app, cx| {
                        app.status_message = Some(format!("Open failed: {e}"));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    /// Validates and stores a bookmark. Bookmarks target directories only:
    /// file paths are rejected with an in-modal error instead of creating
    /// a bookmark that could never navigate anywhere useful.
    pub fn submit_add_bookmark(
        &mut self,
        label: String,
        path: String,
        is_remote: bool,
        is_session_only: bool,
        cx: &mut Context<Self>,
    ) {
        if label.trim().is_empty() {
            self.set_bookmark_error("Label is required".to_string(), cx);
            return;
        }

        if !is_remote {
            if !std::path::Path::new(&path).is_dir() {
                self.set_bookmark_error(
                    format!("'{path}' is not a directory; bookmarks target directories only"),
                    cx,
                );
                return;
            }
            self.store_bookmark(label, path, false, is_session_only);
            cx.notify();
            return;
        }

        if !self.remote_client.is_connected() {
            self.set_bookmark_error(
                "Remote bookmarks need an active SFTP connection".to_string(),
                cx,
            );
            return;
        }
        let client = self.remote_client.clone();
        cx.spawn(async move |this, cx| {
            let res = client.is_dir(path.clone()).await;
            let _ = this.update(cx, |app, cx| match res {
                Ok(true) => {
                    app.store_bookmark(label.clone(), path.clone(), true, is_session_only);
                    cx.notify();
                }
                Ok(false) => {
                    app.set_bookmark_error(
                        format!("'{path}' is not a directory; bookmarks target directories only"),
                        cx,
                    );
                }
                Err(e) => {
                    app.set_bookmark_error(format!("Cannot verify remote path: {e}"), cx);
                }
            });
        })
        .detach();
    }

    fn set_bookmark_error(&mut self, msg: String, cx: &mut Context<Self>) {
        if let ActiveModal::AddBookmark(ref mut s) = self.modal {
            s.error_msg = Some(msg);
        } else {
            self.status_message = Some(msg);
        }
        cx.notify();
    }

    fn store_bookmark(&mut self, label: String, path: String, is_remote: bool, is_session_only: bool) {
        if is_session_only {
            self.bookmark_store
                .add_session_bookmark(label.clone(), path, is_remote, None);
        } else {
            let _ = BookmarkStore::save_persistent_bookmark(Bookmark {
                id: Uuid::new_v4().to_string(),
                label: label.clone(),
                path,
                is_remote,
                profile_id: None,
                is_session_only: false,
            });
            self.bookmark_store.refresh_saved_cache();
        }
        self.modal = ActiveModal::None;
        self.status_message = Some(format!("Bookmarked '{label}'"));
    }

    pub fn open_connect_modal(&mut self) {
        let profiles = ProfileStore::load_profiles();
        let mut state = ConnectModalState {
            profiles,
            ..ConnectModalState::default()
        };
        if let Some(first) = state.profiles.first() {
            state.selected_profile_id = Some(first.id.clone());
            state.host_input = first.host.clone();
            state.port_input = first.port.to_string();
            state.username_input = first.username.clone();
        }
        self.modal = ActiveModal::Connect(state);
    }

    pub fn open_bookmarks_modal(&mut self) {
        self.bookmark_store.refresh_saved_cache();
        self.modal = ActiveModal::BookmarksList;
    }

    /// Navigates to a bookmark, guarding remote targets behind an active
    /// connection instead of stranding the pane on an unreachable path.
    pub fn navigate_bookmark(
        &mut self,
        pane_id: PaneId,
        path: String,
        is_remote: bool,
        cx: &mut Context<Self>,
    ) {
        if is_remote && !self.remote_client.is_connected() {
            self.status_message = Some(
                "Remote bookmark needs an active SFTP connection; connect first".to_string(),
            );
            self.open_connect_modal();
            cx.notify();
            return;
        }
        self.modal = ActiveModal::None;
        self.navigate_pane(pane_id, path, cx);
        cx.notify();
    }

    pub fn disconnect_remote(&mut self) {
        // Cancel in-flight transfers first: they hold the session that is
        // about to be dropped, and would otherwise fail confusingly.
        let in_flight: Vec<String> = self
            .transfer_items
            .iter()
            .filter(|i| {
                matches!(
                    i.state,
                    crate::transfer::types::TransferState::Queued
                        | crate::transfer::types::TransferState::Running
                )
            })
            .map(|i| i.id.clone())
            .collect();
        for id in in_flight {
            self.transfer_manager.cancel_transfer(&id);
        }
        self.remote_client.disconnect();
        self.right_pane.entries = Arc::new(Vec::new());
        self.right_pane.refresh_totals();
        self.right_pane.current_path = "/".to_string();
        self.right_pane.path_input_buffer = "/".to_string();
        self.right_pane.selected_items.clear();
        self.right_pane.focused_index = None;
        self.right_pane.error_message = None;
        self.right_pane.is_loading = false;
        self.status_message = Some("Disconnected from remote host".to_string());
    }

    pub fn execute_connect(&mut self, cx: &mut Context<Self>) {
        // First attempt never auto-trusts: unknown keys surface the
        // trust dialog instead of being silently added to known_hosts.
        if let ActiveModal::Connect(ref mut state) = self.modal {
            // The submit button disables while connecting, but Enter key
            // repeats must also be ignored to avoid duplicate sessions.
            if state.is_connecting {
                return;
            }
            state.is_connecting = true;
            state.error_msg = None;

            let host = state.host_input.trim().to_string();
            let port: u16 = state.port_input.trim().parse().unwrap_or(22);
            let username = state.username_input.trim().to_string();
            let password = state.password_input.clone();
            let key_path = state.key_path_input.clone();
            let auth_kind = state.auth_kind;
            let remember = state.remember_credentials;
            let save_name = state.save_as_profile_name.trim().to_string();

            if host.is_empty() || username.is_empty() {
                state.is_connecting = false;
                state.error_msg = Some("Host and username are required".to_string());
                cx.notify();
                return;
            }

            let auth_method = match auth_kind {
                AuthKind::PrivateKey => AuthMethod::PrivateKey {
                    key_path,
                    has_passphrase: false,
                },
                AuthKind::Agent => AuthMethod::Agent,
                AuthKind::Password => AuthMethod::Password,
            };

            let profile = ConnectionProfile {
                id: state
                    .selected_profile_id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                name: if save_name.is_empty() {
                    format!("{}@{}", username, host)
                } else {
                    save_name
                },
                host,
                port,
                username,
                auth_method,
                remember_credentials: remember,
                remote_default_dir: Some("/".to_string()),
            };

            let password_opt = if password.is_empty() {
                None
            } else {
                Some(password)
            };
            self.start_connection(profile, password_opt, false, cx);
        }
    }

    /// Retries the pending connection with explicit user trust after the
    /// host-key dialog. Previously a no-op: it re-read a connect dialog
    /// that was no longer open.
    pub fn trust_pending_connection(&mut self, cx: &mut Context<Self>) {
        if let ActiveModal::HostKeyTrust(ref state) = self.modal {
            let profile = state.pending_profile.clone();
            let password = state.pending_password.clone();
            self.start_connection(profile, password, true, cx);
        }
    }

    /// Shared connection driver. Profiles and secrets are persisted only
    /// after authentication succeeds, never on failure.
    fn start_connection(
        &mut self,
        profile: ConnectionProfile,
        password: Option<String>,
        trust_unknown_host: bool,
        cx: &mut Context<Self>,
    ) {
        let client = self.remote_client.clone();

        cx.spawn(async move |this, cx| {
            let res = client
                .connect(profile.clone(), password.clone(), trust_unknown_host)
                .await;
            let _ = this.update(cx, |app, cx| {
                match res {
                    Ok(HostKeyStatus::Trusted) => {
                        let mut msg = format!(
                            "Connected to {}@{}:{}",
                            profile.username, profile.host, profile.port
                        );
                        if profile.remember_credentials {
                            let _ = ProfileStore::save_profile(&profile);
                            if let Some(secret) = password.as_ref() {
                                if !secret.is_empty() {
                                    if let Err(e) =
                                        crate::remote::CredentialStore::save_password(
                                            &profile.id,
                                            secret,
                                        )
                                    {
                                        msg = format!(
                                            "{msg} (credential storage failed: {e})"
                                        );
                                    }
                                }
                            }
                        }
                        app.modal = ActiveModal::None;
                        app.status_message = Some(msg);
                        app.right_pane.is_remote = true;
                        app.right_pane.current_path = profile
                            .remote_default_dir
                            .clone()
                            .unwrap_or_else(|| "/".to_string());
                        app.refresh_pane(PaneId::Right, cx);
                    }
                    Ok(HostKeyStatus::Unknown {
                        host,
                        port,
                        fingerprint,
                        key_type,
                    }) => {
                        app.modal = ActiveModal::HostKeyTrust(HostKeyModalState {
                            host,
                            port,
                            fingerprint,
                            key_type,
                            pending_profile: profile,
                            pending_password: password,
                        });
                    }
                    Ok(HostKeyStatus::Mismatch { fingerprint, .. }) => {
                        if let ActiveModal::Connect(ref mut s) = app.modal {
                            s.is_connecting = false;
                            s.error_msg = Some(format!(
                                "HOST KEY CHANGED! Potential security risk. Fingerprint: {fingerprint}"
                            ));
                        } else {
                            app.modal = ActiveModal::None;
                            app.status_message = Some(format!(
                                "HOST KEY CHANGED! Connection refused. Fingerprint: {fingerprint}"
                            ));
                        }
                    }
                    Ok(HostKeyStatus::Error(e)) => {
                        if let ActiveModal::Connect(ref mut s) = app.modal {
                            s.is_connecting = false;
                            s.error_msg = Some(e);
                        } else {
                            app.modal = ActiveModal::None;
                            app.status_message = Some(format!("Connection failed: {e}"));
                        }
                    }
                    Err(e) => {
                        if let ActiveModal::Connect(ref mut s) = app.modal {
                            s.is_connecting = false;
                            s.error_msg = Some(e);
                        } else {
                            app.modal = ActiveModal::None;
                            app.status_message = Some(format!("Connection failed: {e}"));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn open_command_palette(&mut self) {
        self.modal = ActiveModal::CommandPalette(CommandPaletteModalState {
            query: String::new(),
            selected_index: 0,
        });
    }

    /// Static command list: all fields are `'static`, so this allocates
    /// nothing — filtering borrows the shared table every keypress.
    pub fn command_palette_items() -> &'static [PaletteItem] {
        static ITEMS: &[PaletteItem] = &[
            PaletteItem { category: "Connection", title: "Connect to SFTP Server...", action: 0, shortcut: Some("Ctrl+N") },
            PaletteItem { category: "Connection", title: "Disconnect Remote Session", action: 1, shortcut: Some("Ctrl+D") },
            PaletteItem { category: "Files", title: "Transfer Selection to Other Pane", action: 17, shortcut: Some("F5") },
            PaletteItem { category: "Files", title: "New Directory...", action: 2, shortcut: Some("F7") },
            PaletteItem { category: "Files", title: "Rename Selected Item...", action: 4, shortcut: Some("F2") },
            PaletteItem { category: "Files", title: "Show Properties", action: 15, shortcut: Some("Alt+Enter") },
            PaletteItem { category: "Files", title: "Delete Selected Item(s)...", action: 3, shortcut: Some("Delete") },
            PaletteItem { category: "View", title: "Toggle Transfers Drawer", action: 5, shortcut: Some("Ctrl+J") },
            PaletteItem { category: "View", title: "Toggle Theme (Dark / Light)", action: 6, shortcut: Some("Ctrl+T") },
            PaletteItem { category: "View", title: "Toggle Hidden Files", action: 16, shortcut: Some("Ctrl+H") },
            PaletteItem { category: "View", title: "Toggle Interface Animations", action: 18, shortcut: None },
            PaletteItem { category: "View", title: "Focus Filter Search", action: 12, shortcut: Some("Ctrl+F") },
            PaletteItem { category: "View", title: "Edit Path / Breadcrumb", action: 13, shortcut: Some("Ctrl+L") },
            PaletteItem { category: "View", title: "Select All Items in Pane", action: 14, shortcut: Some("Ctrl+A") },
            PaletteItem { category: "Navigate", title: "Switch Active Pane", action: 7, shortcut: Some("Tab") },
            PaletteItem { category: "Navigate", title: "Go to Parent Directory", action: 8, shortcut: Some("Backspace") },
            PaletteItem { category: "Navigate", title: "Refresh Current Pane", action: 9, shortcut: Some("Ctrl+R") },
            PaletteItem { category: "Bookmarks", title: "Open Bookmarks", action: 10, shortcut: Some("Ctrl+B") },
            PaletteItem { category: "Bookmarks", title: "Add Bookmark for Current Path", action: 11, shortcut: Some("Ctrl+Shift+B") },
        ];
        ITEMS
    }

    /// Ranked, match-highlighted palette entries for `query`.
    pub fn filtered_palette(&self, query: &str) -> Vec<PaletteMatch> {
        filter_palette(Self::command_palette_items(), query, &self.palette_usage)
    }

    pub fn execute_palette_action(&mut self, action_id: u8, cx: &mut Context<Self>) {
        *self.palette_usage.entry(action_id).or_insert(0) += 1;
        self.modal = ActiveModal::None;
        match action_id {
            0 => self.open_connect_modal(),
            1 => self.disconnect_remote(),
            2 => {
                let pane_id = self.active_pane;
                self.modal = ActiveModal::NewFolder(NewFolderModalState {
                    target_pane: pane_id,
                    folder_name: String::new(),
                });
            }
            3 => {
                let pane = self.pane(self.active_pane);
                if !pane.selected_items.is_empty() {
                    self.modal = ActiveModal::ConfirmDelete(ConfirmDeleteModalState {
                        target_pane: self.active_pane,
                        item_names: pane.selected_items.iter().cloned().collect(),
                    });
                }
            }
            4 => {
                let pane = self.pane(self.active_pane);
                if let Some(first_sel) = pane.selected_items.iter().next().cloned() {
                    self.modal = ActiveModal::Rename(RenameModalState {
                        target_pane: self.active_pane,
                        original_name: first_sel.clone(),
                        new_name: first_sel,
                    });
                }
            }
            5 => self.show_transfer_drawer = !self.show_transfer_drawer,
            6 => self.toggle_theme(),
            7 => {
                self.active_pane = match self.active_pane {
                    PaneId::Left => PaneId::Right,
                    PaneId::Right => PaneId::Left,
                };
            }
            8 => {
                let active = self.active_pane;
                self.go_parent(active, cx);
            }
            9 => {
                let active = self.active_pane;
                self.refresh_pane(active, cx);
            }
            10 => self.open_bookmarks_modal(),
            11 => {
                let pane = self.pane(self.active_pane);
                let path = pane.current_path.clone();
                let is_remote = pane.is_remote;
                let default_label = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Bookmark")
                    .to_string();
                self.modal = ActiveModal::AddBookmark(AddBookmarkModalState {
                    label: default_label,
                    path,
                    is_remote,
                    is_session_only: false,
                    error_msg: None,
                });
            }
            12 => {
                let p = self.pane_mut(self.active_pane);
                p.filter_active = true;
            }
            13 => {
                let p = self.pane_mut(self.active_pane);
                p.path_input_active = true;
                p.path_input_buffer = p.current_path.clone();
            }
            14 => {
                let p = self.pane_mut(self.active_pane);
                p.select_all();
            }
            15 => {
                let active = self.active_pane;
                self.open_properties(active);
            }
            16 => {
                let active = self.active_pane;
                self.toggle_hidden(active, cx);
            }
            17 => {
                let active = self.active_pane;
                self.transfer_selected(active, cx);
            }
            18 => {
                self.animations_enabled = !self.animations_enabled;
                self.status_message = Some(if self.animations_enabled {
                    "Interface animations on".to_string()
                } else {
                    "Interface animations off".to_string()
                });
            }
            _ => {}
        }
        cx.notify();
    }

    fn get_typed_char(ev: &KeyDownEvent) -> Option<char> {
        if ev.keystroke.modifiers.control || ev.keystroke.modifiers.alt {
            return None;
        }
        if let Some(ref ch_str) = ev.keystroke.key_char {
            if let Some(ch) = ch_str.chars().next() {
                return Some(ch);
            }
        }
        if ev.keystroke.key == "space" {
            return Some(' ');
        }
        if ev.keystroke.key.len() == 1 {
            return ev.keystroke.key.chars().next();
        }
        None
    }

    pub fn handle_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.to_lowercase();
        let ctrl = ev.keystroke.modifiers.control;
        let shift = ev.keystroke.modifiers.shift;

        // 1. Context menu keyboard navigation (Zed §5: arrows + Enter).
        // Escape dismisses; other keys fall through to normal handling.
        if self.context_menu.is_some() {
            match key.as_str() {
                "escape" => {
                    self.context_menu = None;
                    cx.notify();
                    return;
                }
                "up" => {
                    if let Some(menu) = self.context_menu.as_mut() {
                        menu.select_prev();
                    }
                    cx.notify();
                    return;
                }
                "down" => {
                    if let Some(menu) = self.context_menu.as_mut() {
                        menu.select_next();
                    }
                    cx.notify();
                    return;
                }
                "enter" => {
                    let pending = self.context_menu.as_ref().and_then(|menu| {
                        menu.selected_index.and_then(|i| menu.items.get(i)).cloned()
                    });
                    if let Some((action, enabled)) = pending {
                        if enabled {
                            let pane = self
                                .context_menu
                                .as_ref()
                                .map(|m| m.target_pane)
                                .unwrap_or(self.active_pane);
                            self.execute_menu_action(pane, action, cx);
                            return;
                        }
                    }
                    cx.notify();
                    return;
                }
                _ => {}
            }
        }

        // 2. Active modal keyboard interception
        match self.modal {
            ActiveModal::CommandPalette(_) => {
                if key == "escape" {
                    self.modal = ActiveModal::None;
                    cx.notify();
                    return;
                }
                // Snapshot owned values first: the modal borrow must not be
                // held across `&self` calls below.
                let (query, selected) = match &self.modal {
                    ActiveModal::CommandPalette(s) => (s.query.clone(), s.selected_index),
                    _ => (String::new(), 0),
                };
                // Ranked matches drive wrap-around navigation.
                let match_count = self.filtered_palette(&query).len();
                if key == "up" {
                    if match_count > 0 {
                        let next = if selected == 0 { match_count - 1 } else { selected - 1 };
                        if let ActiveModal::CommandPalette(s) = &mut self.modal {
                            s.selected_index = next;
                        }
                        self.palette_scroll_handle.scroll_to_item(next);
                        cx.notify();
                    }
                    return;
                }
                if key == "down" {
                    if match_count > 0 {
                        let next = (selected + 1) % match_count;
                        if let ActiveModal::CommandPalette(s) = &mut self.modal {
                            s.selected_index = next;
                        }
                        self.palette_scroll_handle.scroll_to_item(next);
                        cx.notify();
                    }
                    return;
                }
                if key == "enter" {
                    let matching = self.filtered_palette(&query);
                    if let Some(m) = matching.get(selected) {
                        let act = m.item.action;
                        self.modal = ActiveModal::None;
                        self.execute_palette_action(act, cx);
                    }
                    cx.notify();
                    return;
                }
                if key == "backspace" {
                    if let ActiveModal::CommandPalette(s) = &mut self.modal {
                        s.query.pop();
                        s.selected_index = 0;
                    }
                    self.palette_scroll_handle.scroll_to_item(0);
                    cx.notify();
                    return;
                }
                if let Some(ch) = Self::get_typed_char(ev) {
                    if let ActiveModal::CommandPalette(s) = &mut self.modal {
                        s.query.push(ch);
                        s.selected_index = 0;
                    }
                    self.palette_scroll_handle.scroll_to_item(0);
                    cx.notify();
                    return;
                }
                return;
            }
            ActiveModal::Connect(ref mut state) => {
                if key == "escape" {
                    self.modal = ActiveModal::None;
                    cx.notify();
                    return;
                }
                // Text fields: 0 host, 1 port, 2 username, 3 secret
                // (password or key path), 4 profile name.
                const CONNECT_FIELDS: usize = 5;
                if key == "tab" {
                    if shift {
                        state.focused_field =
                            (state.focused_field + CONNECT_FIELDS - 1) % CONNECT_FIELDS;
                    } else {
                        state.focused_field = (state.focused_field + 1) % CONNECT_FIELDS;
                    }
                    cx.notify();
                    return;
                }
                if key == "up" {
                    state.focused_field = state.focused_field.saturating_sub(1);
                    cx.notify();
                    return;
                }
                if key == "down" {
                    if state.focused_field + 1 < CONNECT_FIELDS {
                        state.focused_field += 1;
                        cx.notify();
                    }
                    return;
                }
                if key == "enter" {
                    self.execute_connect(cx);
                    return;
                }
                if key == "backspace" {
                    match state.focused_field {
                        0 => { state.host_input.pop(); }
                        1 => { state.port_input.pop(); }
                        2 => { state.username_input.pop(); }
                        3 => {
                            if state.auth_kind == AuthKind::PrivateKey {
                                state.key_path_input.pop();
                            } else if state.auth_kind == AuthKind::Password {
                                state.password_input.pop();
                            }
                        }
                        4 => { state.save_as_profile_name.pop(); }
                        _ => {}
                    }
                    cx.notify();
                    return;
                }
                if let Some(ch) = Self::get_typed_char(ev) {
                    match state.focused_field {
                        0 => state.host_input.push(ch),
                        1 => if ch.is_ascii_digit() { state.port_input.push(ch); },
                        2 => state.username_input.push(ch),
                        3 => {
                            if state.auth_kind == AuthKind::PrivateKey {
                                state.key_path_input.push(ch);
                            } else if state.auth_kind == AuthKind::Password {
                                state.password_input.push(ch);
                            }
                        }
                        4 => state.save_as_profile_name.push(ch),
                        _ => {}
                    }
                    cx.notify();
                    return;
                }
                return;
            }
            ActiveModal::NewFolder(ref mut state) => {
                if key == "escape" {
                    self.modal = ActiveModal::None;
                    cx.notify();
                    return;
                }
                if key == "enter" {
                    let target = state.target_pane;
                    let name = if state.folder_name.is_empty() {
                        "New Folder".to_string()
                    } else {
                        state.folder_name.clone()
                    };
                    self.modal = ActiveModal::None;
                    self.create_folder(target, &name, cx);
                    cx.notify();
                    return;
                }
                if key == "backspace" {
                    state.folder_name.pop();
                    cx.notify();
                    return;
                }
                if let Some(ch) = Self::get_typed_char(ev) {
                    state.folder_name.push(ch);
                    cx.notify();
                    return;
                }
                return;
            }
            ActiveModal::Rename(ref mut state) => {
                if key == "escape" {
                    self.modal = ActiveModal::None;
                    cx.notify();
                    return;
                }
                if key == "enter" {
                    let target = state.target_pane;
                    let orig = state.original_name.clone();
                    let new_n = state.new_name.clone();
                    self.modal = ActiveModal::None;
                    self.rename_entry(target, &orig, &new_n, cx);
                    cx.notify();
                    return;
                }
                if key == "backspace" {
                    state.new_name.pop();
                    cx.notify();
                    return;
                }
                if let Some(ch) = Self::get_typed_char(ev) {
                    state.new_name.push(ch);
                    cx.notify();
                    return;
                }
                return;
            }
            ActiveModal::AddBookmark(ref mut state) => {
                if key == "escape" {
                    self.modal = ActiveModal::None;
                    cx.notify();
                    return;
                }
                if key == "enter" {
                    let lbl = state.label.clone();
                    let pth = state.path.clone();
                    let is_rem = state.is_remote;
                    let is_sess = state.is_session_only;
                    self.submit_add_bookmark(lbl, pth, is_rem, is_sess, cx);
                    return;
                }
                if key == "backspace" {
                    state.label.pop();
                    cx.notify();
                    return;
                }
                if let Some(ch) = Self::get_typed_char(ev) {
                    state.label.push(ch);
                    cx.notify();
                    return;
                }
                return;
            }
            ActiveModal::ConfirmDelete(ref mut state) => {
                if key == "escape" {
                    self.modal = ActiveModal::None;
                    cx.notify();
                    return;
                }
                if key == "enter" {
                    let p = state.target_pane;
                    let items = state.item_names.clone();
                    self.modal = ActiveModal::None;
                    self.confirm_delete(p, items, cx);
                    cx.notify();
                    return;
                }
                return;
            }
            ActiveModal::HostKeyTrust(_) => {
                if key == "escape" {
                    self.modal = ActiveModal::None;
                    cx.notify();
                    return;
                }
                if key == "enter" {
                    self.trust_pending_connection(cx);
                    return;
                }
                return;
            }
            ActiveModal::Conflict(_) => {
                if key == "escape" {
                    self.modal = ActiveModal::None;
                    cx.notify();
                    return;
                }
                // Safe default: Enter skips instead of overwriting.
                if key == "enter" || key == "s" {
                    self.modal = ActiveModal::None;
                    self.status_message = Some("Skipped conflicting file".to_string());
                    cx.notify();
                    return;
                }
                return;
            }
            ActiveModal::BookmarksList => {
                if key == "escape" {
                    self.modal = ActiveModal::None;
                    cx.notify();
                    return;
                }
                return;
            }
            ActiveModal::Properties(_) => {
                if key == "escape" || key == "enter" {
                    self.modal = ActiveModal::None;
                    cx.notify();
                    return;
                }
                return;
            }
            ActiveModal::None => {}
        }

        // 3. Command Palette Trigger (Ctrl+P / Ctrl+Shift+P / F1)
        if (ctrl && key == "p") || (ctrl && shift && key == "p") || key == "f1" {
            self.modal = ActiveModal::CommandPalette(CommandPaletteModalState {
                query: String::new(),
                selected_index: 0,
            });
            cx.notify();
            return;
        }

        // 4. Global Theme Toggle (Ctrl+T)
        if ctrl && key == "t" {
            self.toggle_theme();
            cx.notify();
            return;
        }

        // 5. Active pane input interception (Filter / Path input)
        let active_pane = self.active_pane;
        let is_filter_active = match active_pane {
            PaneId::Left => self.left_pane.filter_active,
            PaneId::Right => self.right_pane.filter_active,
        };
        let is_path_input_active = match active_pane {
            PaneId::Left => self.left_pane.path_input_active,
            PaneId::Right => self.right_pane.path_input_active,
        };

        if (ctrl && key == "f") || (key == "/" && !is_filter_active && !is_path_input_active) {
            let pane = match active_pane {
                PaneId::Left => &mut self.left_pane,
                PaneId::Right => &mut self.right_pane,
            };
            pane.filter_active = true;
            cx.notify();
            return;
        }

        if ctrl && key == "l" {
            let pane = match active_pane {
                PaneId::Left => &mut self.left_pane,
                PaneId::Right => &mut self.right_pane,
            };
            pane.path_input_active = true;
            pane.path_input_buffer = pane.current_path.clone();
            cx.notify();
            return;
        }

        if is_filter_active {
            if key == "escape" {
                let pane = match active_pane {
                    PaneId::Left => &mut self.left_pane,
                    PaneId::Right => &mut self.right_pane,
                };
                pane.filter.clear();
                pane.filter_active = false;
                self.refresh_pane(active_pane, cx);
                cx.notify();
                return;
            }
            if key == "enter" {
                let pane = match active_pane {
                    PaneId::Left => &mut self.left_pane,
                    PaneId::Right => &mut self.right_pane,
                };
                pane.filter_active = false;
                pane.select_first();
                cx.notify();
                return;
            }
            if key == "backspace" {
                let pane = match active_pane {
                    PaneId::Left => &mut self.left_pane,
                    PaneId::Right => &mut self.right_pane,
                };
                pane.filter.pop();
                self.refresh_pane(active_pane, cx);
                cx.notify();
                return;
            }
            if let Some(ch) = Self::get_typed_char(ev) {
                let pane = match active_pane {
                    PaneId::Left => &mut self.left_pane,
                    PaneId::Right => &mut self.right_pane,
                };
                pane.filter.push(ch);
                self.refresh_pane(active_pane, cx);
                cx.notify();
                return;
            }
            return;
        }

        if is_path_input_active {
            if key == "escape" {
                let pane = match active_pane {
                    PaneId::Left => &mut self.left_pane,
                    PaneId::Right => &mut self.right_pane,
                };
                pane.path_input_active = false;
                cx.notify();
                return;
            }
            if key == "enter" {
                let p = match active_pane {
                    PaneId::Left => {
                        self.left_pane.path_input_active = false;
                        self.left_pane.path_input_buffer.clone()
                    }
                    PaneId::Right => {
                        self.right_pane.path_input_active = false;
                        self.right_pane.path_input_buffer.clone()
                    }
                };
                self.navigate_pane(active_pane, p, cx);
                cx.notify();
                return;
            }
            if key == "backspace" {
                let pane = match active_pane {
                    PaneId::Left => &mut self.left_pane,
                    PaneId::Right => &mut self.right_pane,
                };
                pane.path_input_buffer.pop();
                cx.notify();
                return;
            }
            if let Some(ch) = Self::get_typed_char(ev) {
                let pane = match active_pane {
                    PaneId::Left => &mut self.left_pane,
                    PaneId::Right => &mut self.right_pane,
                };
                pane.path_input_buffer.push(ch);
                cx.notify();
                return;
            }
            return;
        }

        // 6. Navigation and File Operations
        if key == "f5" || (ctrl && key == "r") {
            if ctrl && key == "r" {
                let active = self.active_pane;
                self.refresh_pane(active, cx);
            } else {
                let active = self.active_pane;
                self.transfer_selected(active, cx);
            }
            cx.notify();
            return;
        }

        if key == "f2" {
            let active = self.active_pane;
            let pane = self.pane(active);
            if let Some(entry) = pane.selected_entries().first() {
                self.modal = ActiveModal::Rename(RenameModalState {
                    target_pane: active,
                    original_name: entry.name.clone(),
                    new_name: entry.name.clone(),
                });
                cx.notify();
            }
            return;
        }

        if key == "f7" {
            self.modal = ActiveModal::NewFolder(NewFolderModalState {
                target_pane: self.active_pane,
                folder_name: String::new(),
            });
            cx.notify();
            return;
        }

        if ctrl && shift && key == "b" {
            let pane = self.pane(self.active_pane);
            let path = pane.current_path.clone();
            let is_remote = pane.is_remote;
            let default_label = std::path::Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Bookmark")
                .to_string();
            self.modal = ActiveModal::AddBookmark(AddBookmarkModalState {
                label: default_label,
                path,
                is_remote,
                is_session_only: false,
                error_msg: None,
            });
            cx.notify();
            return;
        }

        if ctrl && key == "b" {
            self.open_bookmarks_modal();
            cx.notify();
            return;
        }

        // Copy the focused (or first selected) entry's full path. The
        // context menu advertises Ctrl+C, so it must work globally too.
        if ctrl && key == "c" {
            let active = self.active_pane;
            let target: Option<String> = self
                .focused_entry(active)
                .map(|(e, _)| e.path.to_string_lossy().to_string());
            if let Some(path) = target {
                cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
                self.status_message = Some(format!("Copied: {path}"));
            }
            cx.notify();
            return;
        }

        if key == "delete" || key == "f8" || (key == "backspace" && shift) {
            self.delete_selected(self.active_pane);
            cx.notify();
            return;
        }

        if ctrl && key == "a" {
            let active = self.active_pane;
            let pane = self.pane_mut(active);
            pane.select_all();
            cx.notify();
            return;
        }

        if key == "backspace" || (ev.keystroke.modifiers.alt && key == "up") {
            let active = self.active_pane;
            self.go_parent(active, cx);
            cx.notify();
            return;
        }

        if key == "tab" {
            self.active_pane = match self.active_pane {
                PaneId::Left => PaneId::Right,
                PaneId::Right => PaneId::Left,
            };
            cx.notify();
            return;
        }

        if ev.keystroke.modifiers.alt && key == "left" {
            let active = self.active_pane;
            self.go_back(active, cx);
            cx.notify();
            return;
        }

        if ev.keystroke.modifiers.alt && key == "right" {
            let active = self.active_pane;
            self.go_forward(active, cx);
            cx.notify();
            return;
        }

        if ctrl && key == "h" {
            let active = self.active_pane;
            self.toggle_hidden(active, cx);
            cx.notify();
            return;
        }

        if key == "up" {
            let active = self.active_pane;
            let (pane, scroll) = match active {
                PaneId::Left => (&mut self.left_pane, &self.left_scroll_handle),
                PaneId::Right => (&mut self.right_pane, &self.right_scroll_handle),
            };
            if shift {
                if let Some(cur) = pane.focused_index {
                    if cur > 0 {
                        pane.select_range_to(cur - 1);
                        scroll.scroll_to_item(cur - 1, ScrollStrategy::Center);
                    }
                }
            } else if let Some(idx) = pane.select_prev() {
                scroll.scroll_to_item(idx, ScrollStrategy::Center);
            }
            cx.notify();
            return;
        }

        if key == "down" {
            let active = self.active_pane;
            let (pane, scroll) = match active {
                PaneId::Left => (&mut self.left_pane, &self.left_scroll_handle),
                PaneId::Right => (&mut self.right_pane, &self.right_scroll_handle),
            };
            if shift {
                if let Some(cur) = pane.focused_index {
                    if cur + 1 < pane.entries.len() {
                        pane.select_range_to(cur + 1);
                        scroll.scroll_to_item(cur + 1, ScrollStrategy::Center);
                    }
                }
            } else if let Some(idx) = pane.select_next() {
                scroll.scroll_to_item(idx, ScrollStrategy::Center);
            }
            cx.notify();
            return;
        }

        if key == "home" {
            let active = self.active_pane;
            let (pane, scroll) = match active {
                PaneId::Left => (&mut self.left_pane, &self.left_scroll_handle),
                PaneId::Right => (&mut self.right_pane, &self.right_scroll_handle),
            };
            if let Some(idx) = pane.select_first() {
                scroll.scroll_to_item(idx, ScrollStrategy::Center);
            }
            cx.notify();
            return;
        }

        if key == "end" {
            let active = self.active_pane;
            let (pane, scroll) = match active {
                PaneId::Left => (&mut self.left_pane, &self.left_scroll_handle),
                PaneId::Right => (&mut self.right_pane, &self.right_scroll_handle),
            };
            if let Some(idx) = pane.select_last() {
                scroll.scroll_to_item(idx, ScrollStrategy::Center);
            }
            cx.notify();
            return;
        }

        if key == "pageup" {
            let active = self.active_pane;
            let (pane, scroll) = match active {
                PaneId::Left => (&mut self.left_pane, &self.left_scroll_handle),
                PaneId::Right => (&mut self.right_pane, &self.right_scroll_handle),
            };
            if let Some(idx) = pane.page_up() {
                scroll.scroll_to_item(idx, ScrollStrategy::Center);
            }
            cx.notify();
            return;
        }

        if key == "pagedown" {
            let active = self.active_pane;
            let (pane, scroll) = match active {
                PaneId::Left => (&mut self.left_pane, &self.left_scroll_handle),
                PaneId::Right => (&mut self.right_pane, &self.right_scroll_handle),
            };
            if let Some(idx) = pane.page_down() {
                scroll.scroll_to_item(idx, ScrollStrategy::Center);
            }
            cx.notify();
            return;
        }

        if key == "enter" {
            let active = self.active_pane;
            self.open_entry(active, cx);
            return;
        }

        if ev.keystroke.modifiers.alt && key == "enter" {
            let active = self.active_pane;
            self.open_properties(active);
            cx.notify();
        }
    }
}

impl Render for RiftApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {

        div()
            .font_family(self.theme.font_ui)
            .text_size(self.theme.font_size_ui)
            .track_focus(&self.focus_handle)
            .key_context("RiftApp")
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                this.handle_key(ev, cx);
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(self.theme.background)
            .text_color(self.theme.text)
            .child(self.render_header(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    // The newly activated pane flashes subtly on switch; keys
                    // only change when `active_pane` does, so refreshes stay static.
                    .child(crate::ui::motion::flash(
                        self.animations_enabled,
                        self.render_pane(PaneId::Left, cx),
                        ElementId::Name(format!("pane-flash-left-{}", self.active_pane == PaneId::Left).into()),
                    ))
                    .child(crate::ui::motion::flash(
                        self.animations_enabled,
                        self.render_pane(PaneId::Right, cx),
                        ElementId::Name(format!("pane-flash-right-{}", self.active_pane == PaneId::Right).into()),
                    )),
            )
            .when(self.show_transfer_drawer, |el| {
                // Opacity-only: the drawer is in-flow, so a rise would
                // overlap the panes above it during the entrance.
                el.child(crate::ui::motion::flash(
                    self.animations_enabled,
                    self.render_transfer_drawer(cx),
                    ElementId::Name("transfer-drawer".into()),
                ))
            })
            .child(self.render_status_bar(cx))
            .when(!matches!(self.modal, ActiveModal::None), |el| {
                // Backdrop fades in; the dialog itself gets a fade-and-rise.
                el.child(crate::ui::motion::flash(
                    self.animations_enabled,
                    self.render_modal_overlay(cx),
                    ElementId::Name("modal-backdrop".into()),
                ))
            })
            .when_some(self.context_menu.as_ref(), |el, menu| {
                el.child(crate::ui::motion::entrance(
                    self.animations_enabled,
                    self.render_context_menu(menu, window, cx),
                    ElementId::Name("context-menu".into()),
                ))
            })
    }
}

impl RiftApp {
    pub fn open_context_menu(
        &mut self,
        pane_id: PaneId,
        target_entry: Option<crate::fs::types::FileEntry>,
        position: Point<Pixels>,
    ) {
        self.active_pane = pane_id;
        let (current_path, is_remote) = match pane_id {
            PaneId::Left => (
                self.left_pane.current_path.clone(),
                self.left_pane.is_remote,
            ),
            PaneId::Right => (
                self.right_pane.current_path.clone(),
                self.right_pane.is_remote,
            ),
        };
        let other_is_remote = match pane_id {
            PaneId::Left => self.right_pane.is_remote,
            PaneId::Right => self.left_pane.is_remote,
        };
        let connected = self.remote_client.is_connected();
        let transfer_enabled = (!is_remote && !other_is_remote) || connected;
        let remote_ops_enabled = !is_remote || connected;

        let mut items = Vec::new();
        if let Some(entry) = target_entry.clone() {
            let name = entry.name.clone();
            let target_path = if is_remote {
                join_path(&current_path, &name, true)
            } else {
                entry.path.to_string_lossy().to_string()
            };
            if entry.is_dir() {
                items.push((
                    MenuAction::OpenDirectory { path: target_path.clone() },
                    true,
                ));
            } else {
                items.push((MenuAction::OpenFile, remote_ops_enabled));
            }
            items.push((MenuAction::Transfer, transfer_enabled));
            items.push((MenuAction::Properties, true));
            items.push((
                MenuAction::Rename { name: name.clone() },
                remote_ops_enabled,
            ));
            items.push((MenuAction::Delete, remote_ops_enabled));
            items.push((MenuAction::CopyPath { path: target_path }, true));
            items.push((MenuAction::CopyName { name }, true));
            // Bookmarks target directories only.
            if entry.is_dir() {
                items.push((
                    MenuAction::AddBookmark {
                        path: if is_remote {
                            join_path(&current_path, &entry.name, true)
                        } else {
                            entry.path.to_string_lossy().to_string()
                        },
                        label: entry.name.clone(),
                        is_remote,
                    },
                    true,
                ));
            }
        } else {
            let folder_label = std::path::Path::new(&current_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&current_path)
                .to_string();
            items.push((MenuAction::NewFolder, true));
            items.push((
                MenuAction::BookmarkFolder {
                    path: current_path,
                    label: folder_label,
                    is_remote,
                },
                true,
            ));
            items.push((MenuAction::SelectAll, true));
            items.push((MenuAction::Refresh, true));
            items.push((MenuAction::GoParent, true));
            items.push((MenuAction::ToggleHidden, true));
        }

        let mut menu = ContextMenuState {
            position,
            target_pane: pane_id,
            target_entry,
            selected_index: None,
            items,
        };
        menu.select_first_enabled();
        self.context_menu = Some(menu);
    }

    /// Executes a context-menu action for both mouse and keyboard dispatch.
    pub fn execute_menu_action(
        &mut self,
        pane_id: PaneId,
        action: MenuAction,
        cx: &mut Context<Self>,
    ) {
        self.context_menu = None;
        self.active_pane = pane_id;
        match action {
            MenuAction::OpenDirectory { path } => {
                self.navigate_pane(pane_id, path, cx);
            }
            MenuAction::OpenFile => {
                self.open_entry(pane_id, cx);
            }
            MenuAction::Transfer => {
                self.transfer_selected(pane_id, cx);
            }
            MenuAction::Properties => {
                self.open_properties(pane_id);
            }
            MenuAction::Rename { name } => {
                self.modal = ActiveModal::Rename(RenameModalState {
                    target_pane: pane_id,
                    original_name: name.clone(),
                    new_name: name,
                });
            }
            MenuAction::Delete => {
                self.delete_selected(pane_id);
            }
            MenuAction::CopyPath { path } => {
                cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
                self.status_message = Some(format!("Copied: {path}"));
            }
            MenuAction::CopyName { name } => {
                cx.write_to_clipboard(ClipboardItem::new_string(name.clone()));
                self.status_message = Some(format!("Copied: {name}"));
            }
            MenuAction::AddBookmark { path, label, is_remote }
            | MenuAction::BookmarkFolder { path, label, is_remote } => {
                self.modal = ActiveModal::AddBookmark(AddBookmarkModalState {
                    path,
                    label,
                    is_remote,
                    is_session_only: false,
                    error_msg: None,
                });
            }
            MenuAction::NewFolder => {
                self.modal = ActiveModal::NewFolder(NewFolderModalState {
                    target_pane: pane_id,
                    folder_name: String::new(),
                });
            }
            MenuAction::SelectAll => {
                let p = self.pane_mut(pane_id);
                p.select_all();
            }
            MenuAction::Refresh => {
                self.refresh_pane(pane_id, cx);
            }
            MenuAction::GoParent => {
                self.go_parent(pane_id, cx);
            }
            MenuAction::ToggleHidden => {
                self.toggle_hidden(pane_id, cx);
            }
        }
        cx.notify();
    }

    /// Stores the weak self handle after construction (see `main.rs`).
    pub fn set_self_entity(&mut self, entity: WeakEntity<RiftApp>) {
        self.self_entity = Some(entity);
    }

    /// Weak self handle for render code (virtualized rows dispatch
    /// through it; see `pane_view.rs`).
    pub(crate) fn self_entity(&self) -> Option<WeakEntity<RiftApp>> {
        self.self_entity.clone()
    }

    fn pane(&self, id: PaneId) -> &PaneState {
        match id {
            PaneId::Left => &self.left_pane,
            PaneId::Right => &self.right_pane,
        }
    }

    fn pane_mut(&mut self, id: PaneId) -> &mut PaneState {
        match id {
            PaneId::Left => &mut self.left_pane,
            PaneId::Right => &mut self.right_pane,
        }
    }

    pub fn toggle_theme(&mut self) {
        if self.theme.is_dark {
            self.theme = Theme::zed_light();
        } else {
            self.theme = Theme::zed_dark();
        }
    }
}

#[cfg(test)]
mod tests {
    // NOTE: import RiftApp explicitly, never `use super::*` here — the
    // glob would re-export gpui's #[test] macro over the builtin one.
    use super::RiftApp;

    #[test]
    fn test_purge_stale_temp_copies_keeps_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = dir.path().join("fresh.bin");
        let stale = dir.path().join("stale.bin");
        std::fs::write(&fresh, b"new").unwrap();
        std::fs::write(&stale, b"old").unwrap();

        let old_time = std::time::SystemTime::now()
            - std::time::Duration::from_secs(48 * 3600);
        std::fs::File::options()
            .write(true)
            .open(&stale)
            .unwrap()
            .set_modified(old_time)
            .unwrap();

        RiftApp::purge_stale_temp_copies(dir.path());

        assert!(fresh.exists(), "fresh temp copy must survive");
        assert!(!stale.exists(), "day-old temp copy must be purged");

        // Missing directory must not panic.
        RiftApp::purge_stale_temp_copies(&dir.path().join("nope"));
    }
}
