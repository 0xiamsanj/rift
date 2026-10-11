use directories::BaseDirs;
use ssh2::{CheckResult, KnownHostFileKind, Session};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyStatus {
    Trusted,
    Unknown {
        host: String,
        port: u16,
        fingerprint: String,
        key_type: String,
    },
    Mismatch {
        host: String,
        port: u16,
        fingerprint: String,
    },
    Error(String),
}

pub struct KnownHostsManager;

impl KnownHostsManager {
    pub fn default_known_hosts_path() -> Option<PathBuf> {
        let base = BaseDirs::new()?;
        let ssh_dir = base.home_dir().join(".ssh");
        let _ = fs::create_dir_all(&ssh_dir);
        Some(ssh_dir.join("known_hosts"))
    }

    /// Checks the session host key against known_hosts.
    pub fn verify_host(sess: &Session, host: &str, port: u16) -> HostKeyStatus {
        let mut known_hosts = match sess.known_hosts() {
            Ok(kh) => kh,
            Err(e) => return HostKeyStatus::Error(format!("Failed to initialize known_hosts: {e}")),
        };

        if let Some(path) = Self::default_known_hosts_path() {
            if path.exists() {
                let _ = known_hosts.read_file(&path, KnownHostFileKind::OpenSSH);
            }
        }

        let (key, key_type) = match sess.host_key() {
            Some((k, kt)) => (k, format!("{:?}", kt)),
            None => return HostKeyStatus::Error("Server did not present a host key".to_string()),
        };

        let fingerprint = match sess.host_key_hash(ssh2::HashType::Sha256) {
            Some(h) => format!("SHA256:{}", hex::encode(h)),
            None => "unknown".to_string(),
        };

        let check_res = known_hosts.check_port(host, port, key);
        match check_res {
            CheckResult::Match => {
                info!(host = %host, "SSH host key matched known_hosts");
                HostKeyStatus::Trusted
            }
            CheckResult::NotFound => {
                info!(host = %host, fingerprint = %fingerprint, "Host key not found in known_hosts");
                HostKeyStatus::Unknown {
                    host: host.to_string(),
                    port,
                    fingerprint,
                    key_type,
                }
            }
            CheckResult::Mismatch => {
                warn!(host = %host, fingerprint = %fingerprint, "HOST KEY MISMATCH DETECTED!");
                HostKeyStatus::Mismatch {
                    host: host.to_string(),
                    port,
                    fingerprint,
                }
            }
            CheckResult::Failure => {
                HostKeyStatus::Error("Failed to check host key in known_hosts database".to_string())
            }
        }
    }

    /// Adds and saves a verified host key to the known_hosts file.
    pub fn trust_host_key(sess: &Session, host: &str, port: u16) -> Result<(), String> {
        let mut known_hosts = sess
            .known_hosts()
            .map_err(|e| format!("Failed to initialize known_hosts: {e}"))?;

        let path = Self::default_known_hosts_path()
            .ok_or_else(|| "Could not locate .ssh directory".to_string())?;

        if path.exists() {
            let _ = known_hosts.read_file(&path, KnownHostFileKind::OpenSSH);
        }

        let (key, key_type) = sess
            .host_key()
            .ok_or_else(|| "Host key not available".to_string())?;

        let host_entry = if port == 22 {
            host.to_string()
        } else {
            format!("[{}]:{}", host, port)
        };

        known_hosts
            .add(&host_entry, key, "added by rift", key_type.into())
            .map_err(|e| format!("Failed to add host key to list: {e}"))?;

        known_hosts
            .write_file(&path, KnownHostFileKind::OpenSSH)
            .map_err(|e| format!("Failed to write known_hosts to {}: {e}", path.display()))?;

        info!("Host key for {} written to {}", host_entry, path.display());
        Ok(())
    }
}
