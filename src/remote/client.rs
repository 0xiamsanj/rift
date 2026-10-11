use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{Local, TimeZone};
use parking_lot::Mutex;
use ssh2::{OpenFlags, OpenType, Session, Sftp};
use tracing::info;

use crate::fs::types::{FileEntry, FileKind, FileMetadata, SortColumn, SortDirection};
use crate::fs::LocalFs;

use super::known_hosts::{HostKeyStatus, KnownHostsManager};
use super::profile::{AuthMethod, ConnectionProfile};

pub struct SftpSession {
    pub profile: ConnectionProfile,
    _session: Session,
    sftp: Sftp,
}

// Wrap in thread-safe handle
#[derive(Clone)]
pub struct RemoteClient {
    inner: Arc<Mutex<Option<SftpSession>>>,
    pub current_profile: Arc<Mutex<Option<ConnectionProfile>>>,
}

impl RemoteClient {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            current_profile: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.inner.lock().is_some()
    }

    pub fn disconnect(&self) {
        let mut guard = self.inner.lock();
        if let Some(s) = guard.take() {
            info!(host = %s.profile.host, "Disconnected SFTP session");
        }
        *self.current_profile.lock() = None;
    }

    /// Establishes an SSH connection and verifies the host key.
    /// Returns either Ok(status) or Err(message).
    pub async fn connect(
        &self,
        profile: ConnectionProfile,
        password_or_passphrase: Option<String>,
        trust_unknown_host: bool,
    ) -> Result<HostKeyStatus, String> {
        let prof_clone = profile.clone();
        let pass_clone = password_or_passphrase.clone();

        let res = tokio::task::spawn_blocking(move || {
            let addr = format!("{}:{}", prof_clone.host, prof_clone.port);
            let tcp = TcpStream::connect_timeout(
                &addr
                    .parse()
                    .or_else(|_| {
                        use std::net::ToSocketAddrs;
                        addr.to_socket_addrs()
                            .map_err(|e| e)?
                            .next()
                            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Host address resolution failed"))
                    })
                    .map_err(|e| format!("Failed to connect to {addr}: {e}"))?,
                Duration::from_secs(10),
            )
            .map_err(|e| format!("Connection to {addr} timed out: {e}"))?;

            let mut session = Session::new().map_err(|e| format!("SSH session init failed: {e}"))?;
            session.set_tcp_stream(tcp);
            session
                .handshake()
                .map_err(|e| format!("SSH handshake failed: {e}"))?;

            // Host key verification
            let host_status = KnownHostsManager::verify_host(&session, &prof_clone.host, prof_clone.port);

            match &host_status {
                HostKeyStatus::Mismatch { fingerprint, .. } => {
                    return Err(format!(
                        "WARNING: Remote host key mismatch! Potential security risk. Fingerprint: {fingerprint}"
                    ));
                }
                HostKeyStatus::Unknown { .. } if !trust_unknown_host => {
                    // Return status so UI can prompt user to trust or abort
                    return Ok((session, prof_clone, host_status, false));
                }
                HostKeyStatus::Unknown { .. } if trust_unknown_host => {
                    // Save trusted key
                    let _ = KnownHostsManager::trust_host_key(&session, &prof_clone.host, prof_clone.port);
                }
                _ => {}
            }

            // Authentication
            match &prof_clone.auth_method {
                AuthMethod::Password => {
                    let pass = pass_clone.unwrap_or_default();
                    session
                        .userauth_password(&prof_clone.username, &pass)
                        .map_err(|e| format!("Authentication failed: {e}"))?;
                }
                AuthMethod::PrivateKey { key_path, .. } => {
                    let path = Path::new(key_path);
                    let passphrase = pass_clone.as_deref();
                    session
                        .userauth_pubkey_file(&prof_clone.username, None, path, passphrase)
                        .map_err(|e| format!("Key authentication failed: {e}"))?;
                }
                AuthMethod::Agent => {
                    let mut agent = session.agent().map_err(|e| format!("SSH Agent error: {e}"))?;
                    agent.connect().map_err(|e| format!("Failed to connect to SSH agent: {e}"))?;
                    agent.list_identities().map_err(|e| format!("SSH Agent identities error: {e}"))?;
                    let mut authed = false;
                    for identity in agent.identities().unwrap_or_default() {
                        if agent.userauth(&prof_clone.username, &identity).is_ok() {
                            authed = true;
                            break;
                        }
                    }
                    if !authed {
                        return Err("SSH agent had no matching key for authentication".to_string());
                    }
                }
            }

            if !session.authenticated() {
                return Err("SSH authentication was rejected".to_string());
            }

            let _sftp = session.sftp().map_err(|e| format!("Failed to start SFTP subsystem: {e}"))?;
            Ok((session, prof_clone, host_status, true))
        })
        .await
        .map_err(|e| format!("Task execution error: {e}"))??;

        let (session, prof, status, authenticated) = res;
        if authenticated {
            let mut guard = self.inner.lock();
            let sftp = session.sftp().map_err(|e| format!("SFTP subsystem error: {e}"))?;
            *guard = Some(SftpSession {
                profile: prof.clone(),
                _session: session,
                sftp,
            });
            *self.current_profile.lock() = Some(prof);
            Ok(HostKeyStatus::Trusted)
        } else {
            Ok(status)
        }
    }

    /// Lists directory entries on the remote SFTP server.
    pub async fn list_dir(
        &self,
        remote_path: String,
        sort_by: SortColumn,
        direction: SortDirection,
        filter: Option<String>,
        directories_first: bool,
    ) -> Result<Vec<FileEntry>, String> {
        let client_lock = self.inner.clone();

        tokio::task::spawn_blocking(move || {
            let guard = client_lock.lock();
            let session = guard.as_ref().ok_or_else(|| "Not connected to SFTP".to_string())?;

            let p = Path::new(&remote_path);
            let raw_entries = session
                .sftp
                .readdir(p)
                .map_err(|e| format!("Failed to list remote directory '{}': {e}", remote_path))?;

            let mut entries = Vec::new();
            for (path, stat) in raw_entries {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string());

                if name == "." || name == ".." {
                    continue;
                }

                if let Some(q) = &filter {
                    let q_lower = q.trim().to_lowercase();
                    if !q_lower.is_empty() && !name.to_lowercase().contains(&q_lower) {
                        continue;
                    }
                }

                let kind = if stat.is_dir() {
                    FileKind::Directory
                } else if stat.is_file() {
                    FileKind::File
                } else {
                    FileKind::Symlink
                };

                let extension = if kind == FileKind::Directory {
                    String::new()
                } else {
                    path.extension()
                        .map(|e| e.to_string_lossy().to_string())
                        .unwrap_or_default()
                };

                let modified = stat
                    .mtime
                    .and_then(|t| Local.timestamp_opt(t as i64, 0).single());

                let permissions_str = match stat.perm {
                    Some(perm) => format!("{:o}", perm & 0o777),
                    None => "-".to_string(),
                };

                entries.push(FileEntry {
                    name,
                    path: path.clone(),
                    kind,
                    metadata: FileMetadata {
                        size: stat.size.unwrap_or(0),
                        modified,
                        is_readonly: false,
                        permissions_str,
                    },
                    extension,
                });
            }

            LocalFs::sort_entries(&mut entries, sort_by, direction, directories_first);
            Ok(entries)
        })
        .await
        .map_err(|e| format!("Async worker error: {e}"))?
    }

    /// Creates a directory on remote server.
    pub async fn create_dir(&self, remote_path: String) -> Result<(), String> {
        let client_lock = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = client_lock.lock();
            let session = guard.as_ref().ok_or_else(|| "Not connected".to_string())?;
            session
                .sftp
                .mkdir(Path::new(&remote_path), 0o755)
                .map_err(|e| format!("Failed to create remote directory: {e}"))
        })
        .await
        .map_err(|e| format!("Task execution error: {e}"))?
    }

    /// Deletes a file on remote server.
    pub async fn remove_file(&self, remote_path: String) -> Result<(), String> {
        let client_lock = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = client_lock.lock();
            let session = guard.as_ref().ok_or_else(|| "Not connected".to_string())?;
            session
                .sftp
                .unlink(Path::new(&remote_path))
                .map_err(|e| format!("Failed to delete remote file: {e}"))
        })
        .await
        .map_err(|e| format!("Task execution error: {e}"))?
    }

    /// Deletes a directory on remote server.
    pub async fn remove_dir(&self, remote_path: String) -> Result<(), String> {
        let client_lock = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = client_lock.lock();
            let session = guard.as_ref().ok_or_else(|| "Not connected".to_string())?;
            session
                .sftp
                .rmdir(Path::new(&remote_path))
                .map_err(|e| format!("Failed to delete remote directory: {e}"))
        })
        .await
        .map_err(|e| format!("Task execution error: {e}"))?
    }

    /// Renames or moves a remote file/directory.
    pub async fn rename(&self, src: String, dst: String) -> Result<(), String> {
        let client_lock = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = client_lock.lock();
            let session = guard.as_ref().ok_or_else(|| "Not connected".to_string())?;
            session
                .sftp
                .rename(Path::new(&src), Path::new(&dst), None)
                .map_err(|e| format!("Failed to rename remote item: {e}"))
        })
        .await
        .map_err(|e| format!("Task execution error: {e}"))?
    }

    /// Stat a remote path to see if it exists and get its size.
    pub async fn stat(&self, remote_path: String) -> Result<Option<u64>, String> {
        let client_lock = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = client_lock.lock();
            let session = guard.as_ref().ok_or_else(|| "Not connected".to_string())?;
            match session.sftp.stat(Path::new(&remote_path)) {
                Ok(stat) => Ok(Some(stat.size.unwrap_or(0))),
                Err(_) => Ok(None),
            }
        })
        .await
        .map_err(|e| format!("Task execution error: {e}"))?
    }

    /// Uploads a local file to remote server with a progress callback.
    pub async fn upload_file<F>(
        &self,
        local_path: PathBuf,
        remote_path: String,
        mut on_progress: F,
    ) -> Result<u64, String>
    where
        F: FnMut(u64) + Send + 'static,
    {
        let client_lock = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = client_lock.lock();
            let session = guard.as_ref().ok_or_else(|| "Not connected".to_string())?;

            let mut file = std::fs::File::open(&local_path)
                .map_err(|e| format!("Failed to open local file {}: {e}", local_path.display()))?;

            let mut remote_file = session
                .sftp
                .open_mode(
                    Path::new(&remote_path),
                    OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE,
                    0o644,
                    OpenType::File,
                )
                .map_err(|e| format!("Failed to open remote file {remote_path}: {e}"))?;

            let mut buffer = [0u8; 64 * 1024]; // 64KB chunk
            let mut total_written = 0u64;

            loop {
                let bytes_read = file
                    .read(&mut buffer)
                    .map_err(|e| format!("Local read error: {e}"))?;
                if bytes_read == 0 {
                    break;
                }
                remote_file
                    .write_all(&buffer[..bytes_read])
                    .map_err(|e| format!("Remote write error: {e}"))?;
                total_written += bytes_read as u64;
                on_progress(total_written);
            }

            Ok(total_written)
        })
        .await
        .map_err(|e| format!("Task execution error: {e}"))?
    }

    /// Downloads a remote file to local with a progress callback.
    pub async fn download_file<F>(
        &self,
        remote_path: String,
        local_path: PathBuf,
        mut on_progress: F,
    ) -> Result<u64, String>
    where
        F: FnMut(u64) + Send + 'static,
    {
        let client_lock = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let guard = client_lock.lock();
            let session = guard.as_ref().ok_or_else(|| "Not connected".to_string())?;

            let mut remote_file = session
                .sftp
                .open_mode(Path::new(&remote_path), OpenFlags::READ, 0o644, OpenType::File)
                .map_err(|e| format!("Failed to open remote file {remote_path}: {e}"))?;

            if let Some(parent) = local_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let mut local_file = std::fs::File::create(&local_path)
                .map_err(|e| format!("Failed to create local file {}: {e}", local_path.display()))?;

            let mut buffer = [0u8; 64 * 1024]; // 64KB chunk
            let mut total_read = 0u64;

            loop {
                let bytes_read = remote_file
                    .read(&mut buffer)
                    .map_err(|e| format!("Remote read error: {e}"))?;
                if bytes_read == 0 {
                    break;
                }
                local_file
                    .write_all(&buffer[..bytes_read])
                    .map_err(|e| format!("Local write error: {e}"))?;
                total_read += bytes_read as u64;
                on_progress(total_read);
            }

            Ok(total_read)
        })
        .await
        .map_err(|e| format!("Task execution error: {e}"))?
    }
}
