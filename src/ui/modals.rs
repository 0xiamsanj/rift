use crate::remote::profile::ConnectionProfile;
use super::pane_state::PaneId;

#[derive(Debug, Clone)]
pub enum ActiveModal {
    None,
    Connect(ConnectModalState),
    NewFolder(NewFolderModalState),
    Rename(RenameModalState),
    ConfirmDelete(ConfirmDeleteModalState),
    HostKeyTrust(HostKeyModalState),
    Conflict(ConflictModalState),
    AddBookmark(AddBookmarkModalState),
    BookmarksList,
    CommandPalette(CommandPaletteModalState),
}

#[derive(Debug, Clone)]
pub struct CommandPaletteModalState {
    pub query: String,
    pub selected_index: usize,
}

#[derive(Debug, Clone)]
pub struct ConnectModalState {
    pub profiles: Vec<ConnectionProfile>,
    pub selected_profile_id: Option<String>,
    pub host_input: String,
    pub port_input: String,
    pub username_input: String,
    pub password_input: String,
    pub key_path_input: String,
    pub auth_type: usize, // 0: Password, 1: Private Key, 2: Agent
    pub remember_credentials: bool,
    pub save_as_profile_name: String,
    pub error_msg: Option<String>,
    pub is_connecting: bool,
    pub focused_field: usize, // 0: Host, 1: Port, 2: Username, 3: Password, 4: Key Path, 5: Profile Name
}

impl Default for ConnectModalState {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            selected_profile_id: None,
            host_input: "127.0.0.1".to_string(),
            port_input: "22".to_string(),
            username_input: "user".to_string(),
            password_input: String::new(),
            key_path_input: String::new(),
            auth_type: 0,
            remember_credentials: true,
            save_as_profile_name: String::new(),
            error_msg: None,
            is_connecting: false,
            focused_field: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewFolderModalState {
    pub target_pane: PaneId,
    pub folder_name: String,
}

#[derive(Debug, Clone)]
pub struct RenameModalState {
    pub target_pane: PaneId,
    pub original_name: String,
    pub new_name: String,
}

#[derive(Debug, Clone)]
pub struct ConfirmDeleteModalState {
    pub target_pane: PaneId,
    pub item_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HostKeyModalState {
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub key_type: String,
}

#[derive(Debug, Clone)]
pub struct ConflictModalState {
    pub filename: String,
    pub source_size: u64,
    pub dest_size: u64,
}

#[derive(Debug, Clone)]
pub struct AddBookmarkModalState {
    pub path: String,
    pub label: String,
    pub is_remote: bool,
    pub is_session_only: bool,
}
