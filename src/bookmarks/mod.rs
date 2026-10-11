use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub label: String,
    pub path: String,
    pub is_remote: bool,
    pub profile_id: Option<String>,
    pub is_session_only: bool,
}

pub struct BookmarkStore {
    pub session_bookmarks: Vec<Bookmark>,
    /// Cached copy of the persistent bookmarks file. The bookmarks dialog
    /// renders on every frame, so it must read from this cache instead of
    /// hitting the filesystem per render. Refresh via `refresh_saved_cache`.
    pub cached_saved_bookmarks: Vec<Bookmark>,
}

impl Default for BookmarkStore {
    fn default() -> Self {
        Self::new()
    }
}

impl BookmarkStore {
    pub fn new() -> Self {
        let cached_saved_bookmarks = Self::load_saved_bookmarks();
        Self {
            session_bookmarks: Vec::new(),
            cached_saved_bookmarks,
        }
    }

    /// Re-reads the persistent bookmarks file into the render cache.
    pub fn refresh_saved_cache(&mut self) {
        self.cached_saved_bookmarks = Self::load_saved_bookmarks();
    }

    fn config_path() -> Option<PathBuf> {
        let proj = ProjectDirs::from("com", "rift", "Rift")?;
        let config_dir = proj.config_dir();
        fs::create_dir_all(config_dir).ok()?;
        Some(config_dir.join("bookmarks.json"))
    }

    pub fn load_saved_bookmarks() -> Vec<Bookmark> {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return Vec::new(),
        };

        if !path.exists() {
            return Vec::new();
        }

        match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
                error!("Failed to parse bookmarks: {e}");
                Vec::new()
            }),
            Err(e) => {
                error!("Failed to read bookmarks file: {e}");
                Vec::new()
            }
        }
    }

    pub fn save_persistent_bookmark(bookmark: Bookmark) -> Result<(), String> {
        let path = Self::config_path().ok_or("Config directory not found")?;
        let mut bookmarks = Self::load_saved_bookmarks();

        if let Some(pos) = bookmarks.iter().position(|b| b.id == bookmark.id) {
            bookmarks[pos] = bookmark;
        } else {
            bookmarks.push(bookmark);
        }

        let serialized = serde_json::to_string_pretty(&bookmarks)
            .map_err(|e| format!("Serialization error: {e}"))?;
        fs::write(&path, serialized).map_err(|e| format!("Write error: {e}"))?;
        Ok(())
    }

    pub fn delete_saved_bookmark(id: &str) -> Result<(), String> {
        let path = Self::config_path().ok_or("Config directory not found")?;
        let mut bookmarks = Self::load_saved_bookmarks();
        bookmarks.retain(|b| b.id != id);

        let serialized = serde_json::to_string_pretty(&bookmarks)
            .map_err(|e| format!("Serialization error: {e}"))?;
        fs::write(&path, serialized).map_err(|e| format!("Write error: {e}"))?;
        Ok(())
    }

    pub fn add_session_bookmark(&mut self, label: String, path: String, is_remote: bool, profile_id: Option<String>) {
        self.session_bookmarks.push(Bookmark {
            id: Uuid::new_v4().to_string(),
            label,
            path,
            is_remote,
            profile_id,
            is_session_only: true,
        });
    }

    pub fn remove_session_bookmark(&mut self, id: &str) {
        self.session_bookmarks.retain(|b| b.id != id);
    }
}
