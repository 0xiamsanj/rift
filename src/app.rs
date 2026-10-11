use std::path::PathBuf;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::*;
use tokio::sync::mpsc;

use crate::bookmarks::{Bookmark, BookmarkStore};
use crate::fs::path_utils::{join_path, list_system_roots};
use crate::fs::LocalFs;
use crate::remote::client::RemoteClient;
use crate::remote::known_hosts::HostKeyStatus;
use crate::remote::profile::{AuthMethod, ConnectionProfile, ProfileStore};
use crate::theme::Theme;
use crate::transfer::queue::{TransferEvent, TransferManager};
use crate::transfer::types::TransferItem;
use crate::ui::context_menu::ContextMenuState;
use crate::ui::modals::{
    ActiveModal, AddBookmarkModalState, CommandPaletteModalState, ConfirmDeleteModalState,
    ConnectModalState, HostKeyModalState, NewFolderModalState, RenameModalState,
};
use uuid::Uuid;
use crate::ui::pane_state::{PaneId, PaneState};

pub struct RiftApp {
    pub theme: Theme,
    pub left_pane: PaneState,
    pub right_pane: PaneState,
    pub active_pane: PaneId,
    pub left_scroll_handle: ScrollHandle,
    pub right_scroll_handle: ScrollHandle,
    pub drawer_scroll_handle: ScrollHandle,
    pub remote_client: RemoteClient,
    pub transfer_manager: Arc<TransferManager>,
    pub transfer_items: Vec<TransferItem>,
    pub bookmark_store: BookmarkStore,
    pub modal: ActiveModal,
    pub context_menu: Option<ContextMenuState>,
    pub show_transfer_drawer: bool,
    pub system_roots: Vec<String>,
    pub status_message: Option<String>,
    pub focus_handle: FocusHandle,
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
            left_scroll_handle: ScrollHandle::new(),
            right_scroll_handle: ScrollHandle::new(),
            drawer_scroll_handle: ScrollHandle::new(),
            remote_client,
            transfer_manager,
            transfer_items: Vec::new(),
            bookmark_store: BookmarkStore::new(),
            modal: ActiveModal::None,
            context_menu: None,
            show_transfer_drawer: false,
            system_roots: roots,
            status_message: Some("Ready".to_string()),
            focus_handle,
        };

        app.refresh_local_pane(PaneId::Left);
        app
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
            self.refresh_local_pane(pane_id);
        }
    }

    pub fn refresh_local_pane(&mut self, pane_id: PaneId) {
        let pane = match pane_id {
            PaneId::Left => &mut self.left_pane,
            PaneId::Right => &mut self.right_pane,
        };

        let path = PathBuf::from(&pane.current_path);
        pane.is_loading = true;
        pane.error_message = None;

        match LocalFs::read_dir(
            &path,
            pane.sort_column,
            pane.sort_direction,
            if pane.filter.is_empty() {
                None
            } else {
                Some(&pane.filter)
            },
            pane.directories_first,
            pane.show_hidden,
        ) {
            Ok(entries) => {
                pane.entries = entries;
                pane.is_loading = false;
                Self::prune_selection(pane);
            }
            Err(e) => {
                pane.error_message = Some(format!("Error: {e}"));
                pane.is_loading = false;
            }
        }
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

        let pane = match pane_id {
            PaneId::Left => &mut self.left_pane,
            PaneId::Right => &mut self.right_pane,
        };

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
        // newer navigation.
        cx.spawn(async move |this, cx| {
            let res = client
                .list_dir(remote_path.clone(), sort_by, direction, filter, dirs_first)
                .await;
            let _ = this.update(cx, |app, cx| {
                let pane = match pane_id {
                    PaneId::Left => &mut app.left_pane,
                    PaneId::Right => &mut app.right_pane,
                };
                if pane.current_path != remote_path {
                    return;
                }
                pane.is_loading = false;
                match res {
                    Ok(entries) => {
                        pane.entries = entries;
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
        let pane = match pane_id {
            PaneId::Left => &mut self.left_pane,
            PaneId::Right => &mut self.right_pane,
        };
        pane.navigate_to(target);
        self.refresh_pane(pane_id, cx);
    }

    pub fn go_parent(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        let pane = match pane_id {
            PaneId::Left => &mut self.left_pane,
            PaneId::Right => &mut self.right_pane,
        };
        if pane.go_up() {
            self.refresh_pane(pane_id, cx);
        }
    }

    pub fn go_back(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        let pane = match pane_id {
            PaneId::Left => &mut self.left_pane,
            PaneId::Right => &mut self.right_pane,
        };
        if pane.go_back() {
            self.refresh_pane(pane_id, cx);
        }
    }

    pub fn go_forward(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        let pane = match pane_id {
            PaneId::Left => &mut self.left_pane,
            PaneId::Right => &mut self.right_pane,
        };
        if pane.go_forward() {
            self.refresh_pane(pane_id, cx);
        }
    }

    pub fn toggle_hidden(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        let pane = match pane_id {
            PaneId::Left => &mut self.left_pane,
            PaneId::Right => &mut self.right_pane,
        };
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
            for entry in src_entries {
                if entry.is_dir() {
                    skipped += 1;
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
            self.status_message = Some(format!(
                "Copied {copied} file(s), skipped {skipped} (existing or directories)"
            ));
        }
    }

    pub fn delete_selected(&mut self, pane_id: PaneId) {
        let pane = match pane_id {
            PaneId::Left => &self.left_pane,
            PaneId::Right => &self.right_pane,
        };

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
        self.remote_client.disconnect();
        self.right_pane.entries.clear();
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
            state.is_connecting = true;
            state.error_msg = None;

            let host = state.host_input.trim().to_string();
            let port: u16 = state.port_input.trim().parse().unwrap_or(22);
            let username = state.username_input.trim().to_string();
            let password = state.password_input.clone();
            let key_path = state.key_path_input.clone();
            let auth_type = state.auth_type;
            let remember = state.remember_credentials;
            let save_name = state.save_as_profile_name.trim().to_string();

            if host.is_empty() || username.is_empty() {
                state.is_connecting = false;
                state.error_msg = Some("Host and username are required".to_string());
                cx.notify();
                return;
            }

            let auth_method = match auth_type {
                1 => AuthMethod::PrivateKey {
                    key_path,
                    has_passphrase: false,
                },
                2 => AuthMethod::Agent,
                _ => AuthMethod::Password,
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
                        if profile.remember_credentials {
                            let _ = ProfileStore::save_profile(&profile);
                            if let Some(secret) = password.as_ref() {
                                if !secret.is_empty() {
                                    let _ = crate::remote::CredentialStore::save_password(
                                        &profile.id,
                                        secret,
                                    );
                                }
                            }
                        }
                        app.modal = ActiveModal::None;
                        app.status_message = Some(format!(
                            "Connected to {}@{}:{}",
                            profile.username, profile.host, profile.port
                        ));
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

    pub fn command_palette_items(&self) -> Vec<(&'static str, u8, Option<&'static str>)> {
        vec![
            ("Connect to SFTP Server...", 0, Some("Ctrl+N")),
            ("Disconnect Remote Session", 1, Some("Ctrl+D")),
            ("New Directory...", 2, Some("F7")),
            ("Delete Selected Item(s)...", 3, Some("F8 / Delete")),
            ("Rename Selected Item...", 4, Some("F2")),
            ("Toggle Transfers Drawer", 5, Some("Ctrl+J")),
            ("Toggle Theme (Dark / Light)", 6, Some("Ctrl+T")),
            ("Switch Active Pane", 7, Some("Tab")),
            ("Go to Parent Directory", 8, Some("Backspace")),
            ("Refresh Current Pane", 9, Some("Ctrl+R")),
            ("Open Bookmarks", 10, Some("Ctrl+B")),
            ("Add Bookmark for Current Path", 11, Some("Ctrl+Shift+B")),
            ("Focus Filter Search", 12, Some("Ctrl+F / /")),
            ("Edit Path / Breadcrumb", 13, Some("Ctrl+L")),
            ("Select All Items in Pane", 14, Some("Ctrl+A")),
        ]
    }

    pub fn execute_palette_action(&mut self, action_id: u8, cx: &mut Context<Self>) {
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
                let pane = match self.active_pane {
                    PaneId::Left => &self.left_pane,
                    PaneId::Right => &self.right_pane,
                };
                if !pane.selected_items.is_empty() {
                    self.modal = ActiveModal::ConfirmDelete(ConfirmDeleteModalState {
                        target_pane: self.active_pane,
                        item_names: pane.selected_items.iter().cloned().collect(),
                    });
                }
            }
            4 => {
                let pane = match self.active_pane {
                    PaneId::Left => &self.left_pane,
                    PaneId::Right => &self.right_pane,
                };
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
                let pane = match self.active_pane {
                    PaneId::Left => &self.left_pane,
                    PaneId::Right => &self.right_pane,
                };
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
                });
            }
            12 => {
                let p = match self.active_pane {
                    PaneId::Left => &mut self.left_pane,
                    PaneId::Right => &mut self.right_pane,
                };
                p.filter_active = true;
            }
            13 => {
                let p = match self.active_pane {
                    PaneId::Left => &mut self.left_pane,
                    PaneId::Right => &mut self.right_pane,
                };
                p.path_input_active = true;
                p.path_input_buffer = p.current_path.clone();
            }
            14 => {
                let p = match self.active_pane {
                    PaneId::Left => &mut self.left_pane,
                    PaneId::Right => &mut self.right_pane,
                };
                p.select_all();
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

        // 1. Context menu dismissal
        if self.context_menu.is_some() && key == "escape" {
            self.context_menu = None;
            cx.notify();
            return;
        }

        // 2. Active modal keyboard interception
        match self.modal {
            ActiveModal::CommandPalette(ref mut state) => {
                if key == "escape" {
                    self.modal = ActiveModal::None;
                    cx.notify();
                    return;
                }
                if key == "up" {
                    if state.selected_index > 0 {
                        state.selected_index -= 1;
                        cx.notify();
                    }
                    return;
                }
                if key == "down" {
                    state.selected_index += 1;
                    cx.notify();
                    return;
                }
                if key == "enter" {
                    let cmd_idx = state.selected_index;
                    let query = state.query.to_lowercase();
                    let matching: Vec<_> = self
                        .command_palette_items()
                        .into_iter()
                        .filter(|(name, _, _)| query.is_empty() || name.to_lowercase().contains(&query))
                        .collect();
                    if let Some((_, action_id, _)) = matching.get(cmd_idx) {
                        let act = *action_id;
                        self.modal = ActiveModal::None;
                        self.execute_palette_action(act, cx);
                    }
                    cx.notify();
                    return;
                }
                if key == "backspace" {
                    state.query.pop();
                    state.selected_index = 0;
                    cx.notify();
                    return;
                }
                if let Some(ch) = Self::get_typed_char(ev) {
                    state.query.push(ch);
                    state.selected_index = 0;
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
                            if state.auth_type == 1 {
                                state.key_path_input.pop();
                            } else if state.auth_type == 0 {
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
                            if state.auth_type == 1 {
                                state.key_path_input.push(ch);
                            } else if state.auth_type == 0 {
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
                    if is_sess {
                        self.bookmark_store.add_session_bookmark(lbl.clone(), pth.clone(), is_rem, None);
                    } else {
                        let _ = BookmarkStore::save_persistent_bookmark(Bookmark {
                            id: Uuid::new_v4().to_string(),
                            label: lbl.clone(),
                            path: pth.clone(),
                            is_remote: is_rem,
                            profile_id: None,
                            is_session_only: false,
                        });
                    }
                    self.modal = ActiveModal::None;
                    self.status_message = Some(format!("Bookmarked '{}'", lbl));
                    cx.notify();
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
            let pane = match active {
                PaneId::Left => &self.left_pane,
                PaneId::Right => &self.right_pane,
            };
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
            let pane = match self.active_pane {
                PaneId::Left => &self.left_pane,
                PaneId::Right => &self.right_pane,
            };
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
            let pane = match self.active_pane {
                PaneId::Left => &self.left_pane,
                PaneId::Right => &self.right_pane,
            };
            let target: Option<String> = pane
                .focused_index
                .and_then(|i| pane.entries.get(i))
                .or_else(|| {
                    pane.selected_items
                        .iter()
                        .filter_map(|n| pane.entries.iter().find(|e| &e.name == n))
                        .next()
                })
                .map(|e| e.path.to_string_lossy().to_string());
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
            let pane = match active {
                PaneId::Left => &mut self.left_pane,
                PaneId::Right => &mut self.right_pane,
            };
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
                        scroll.scroll_to_item(cur - 1);
                    }
                }
            } else if let Some(idx) = pane.select_prev() {
                scroll.scroll_to_item(idx);
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
                        scroll.scroll_to_item(cur + 1);
                    }
                }
            } else if let Some(idx) = pane.select_next() {
                scroll.scroll_to_item(idx);
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
                scroll.scroll_to_item(idx);
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
                scroll.scroll_to_item(idx);
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
                scroll.scroll_to_item(idx);
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
                scroll.scroll_to_item(idx);
            }
            cx.notify();
            return;
        }

        if key == "enter" {
            let active = self.active_pane;
            let (target_path, is_dir) = {
                let pane = match active {
                    PaneId::Left => &self.left_pane,
                    PaneId::Right => &self.right_pane,
                };
                if let Some(entry) = pane.selected_entries().first() {
                    let path = if pane.is_remote {
                        join_path(&pane.current_path, &entry.name, true)
                    } else {
                        entry.path.to_string_lossy().to_string()
                    };
                    (Some(path), entry.is_dir())
                } else {
                    (None, false)
                }
            };

            if let Some(path) = target_path {
                if is_dir {
                    self.navigate_pane(active, path, cx);
                    cx.notify();
                }
            }
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
            .bg(self.theme.bg_app)
            .text_color(self.theme.text_primary)
            .child(self.render_header(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(self.render_pane(PaneId::Left, cx))
                    .child(self.render_pane(PaneId::Right, cx)),
            )
            .when(self.show_transfer_drawer, |el| {
                el.child(self.render_transfer_drawer(cx))
            })
            .child(self.render_status_bar(cx))
            .when(!matches!(self.modal, ActiveModal::None), |el| {
                el.child(self.render_modal_overlay(cx))
            })
            .when_some(self.context_menu.as_ref(), |el, menu| {
                el.child(self.render_context_menu(menu, window, cx))
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
        self.context_menu = Some(ContextMenuState {
            position,
            target_pane: pane_id,
            target_entry,
            selected_index: None,
        });
    }

    pub fn toggle_theme(&mut self) {
        if self.theme.is_dark {
            self.theme = Theme::zed_light();
        } else {
            self.theme = Theme::zed_dark();
        }
    }
}
