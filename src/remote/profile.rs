use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{error, info};
use uuid::Uuid;

use super::credentials::CredentialStore;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    Password,
    PrivateKey {
        key_path: String,
        has_passphrase: bool,
    },
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: AuthMethod,
    pub remember_credentials: bool,
    pub remote_default_dir: Option<String>,
}

impl Default for ConnectionProfile {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "New Connection".to_string(),
            host: String::new(),
            port: 22,
            username: String::new(),
            auth_method: AuthMethod::Password,
            remember_credentials: false,
            remote_default_dir: Some("/".to_string()),
        }
    }
}

pub struct ProfileStore;

impl ProfileStore {
    fn config_file_path() -> Option<PathBuf> {
        let proj = ProjectDirs::from("com", "rift", "Rift")?;
        let config_dir = proj.config_dir();
        fs::create_dir_all(config_dir).ok()?;
        Some(config_dir.join("profiles.json"))
    }

    pub fn load_profiles() -> Vec<ConnectionProfile> {
        let path = match Self::config_file_path() {
            Some(p) => p,
            None => return Vec::new(),
        };

        if !path.exists() {
            return Vec::new();
        }

        match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
                error!("Failed to parse profiles from {}: {e}", path.display());
                Vec::new()
            }),
            Err(e) => {
                error!("Failed to read profiles file: {e}");
                Vec::new()
            }
        }
    }

    pub fn save_profile(profile: &ConnectionProfile) -> Result<(), String> {
        let path = Self::config_file_path().ok_or("Failed to determine config directory")?;
        let mut profiles = Self::load_profiles();

        if let Some(pos) = profiles.iter().position(|p| p.id == profile.id) {
            profiles[pos] = profile.clone();
        } else {
            profiles.push(profile.clone());
        }

        let serialized = serde_json::to_string_pretty(&profiles)
            .map_err(|e| format!("Serialization error: {e}"))?;
        fs::write(&path, serialized).map_err(|e| format!("Failed to write profiles: {e}"))?;
        info!("Saved profile '{}' ({})", profile.name, profile.id);
        Ok(())
    }

    pub fn delete_profile(profile_id: &str) -> Result<(), String> {
        let path = Self::config_file_path().ok_or("Failed to determine config directory")?;
        let mut profiles = Self::load_profiles();

        profiles.retain(|p| p.id != profile_id);

        let serialized = serde_json::to_string_pretty(&profiles)
            .map_err(|e| format!("Serialization error: {e}"))?;
        fs::write(&path, serialized).map_err(|e| format!("Failed to update profiles: {e}"))?;

        // Also clean up any stored credentials in keyring
        let _ = CredentialStore::delete_password(profile_id);
        info!("Deleted profile {}", profile_id);
        Ok(())
    }
}
