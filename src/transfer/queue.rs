use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use chrono::Local;
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tracing::error;
use uuid::Uuid;

use crate::remote::RemoteClient;

use super::types::{TransferDirection, TransferItem, TransferState};

#[derive(Debug, Clone)]
pub enum TransferEvent {
    ItemAdded(TransferItem),
    ProgressUpdated {
        id: String,
        transferred_bytes: u64,
        speed_bps: u64,
    },
    ItemFinished {
        id: String,
        state: TransferState,
    },
    QueueFinished,
}

pub struct TransferManager {
    items: Arc<Mutex<Vec<TransferItem>>>,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
    max_concurrency: usize,
    concurrency_limiter: Arc<tokio::sync::Semaphore>,
    cancel_tokens: Arc<Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>>,
}

impl TransferManager {
    pub fn new(event_tx: mpsc::UnboundedSender<TransferEvent>) -> Self {
        Self::with_max_concurrency(event_tx, 2)
    }

    pub fn with_max_concurrency(event_tx: mpsc::UnboundedSender<TransferEvent>, max: usize) -> Self {
        let max = max.max(1);
        Self {
            items: Arc::new(Mutex::new(Vec::new())),
            event_tx,
            max_concurrency: max,
            concurrency_limiter: Arc::new(tokio::sync::Semaphore::new(max)),
            cancel_tokens: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }

    pub fn get_items(&self) -> Vec<TransferItem> {
        self.items.lock().clone()
    }

    pub fn clear_completed(&self) {
        let mut items = self.items.lock();
        items.retain(|i| matches!(i.state, TransferState::Queued | TransferState::Running));
        // Drop cancel tokens for finished items so the map cannot grow
        // without bound over a long session.
        let live: std::collections::HashSet<String> = items.iter().map(|i| i.id.clone()).collect();
        self.cancel_tokens.lock().retain(|id, _| live.contains(id));
    }

    pub fn cancel_transfer(&self, id: &str) {
        if let Some(token) = self.cancel_tokens.lock().get(id) {
            token.store(true, Ordering::SeqCst);
        }
        let mut items = self.items.lock();
        if let Some(item) = items.iter_mut().find(|i| i.id == id) {
            if item.state == TransferState::Queued || item.state == TransferState::Running {
                item.state = TransferState::Cancelled;
                let _ = self.event_tx.send(TransferEvent::ItemFinished {
                    id: id.to_string(),
                    state: TransferState::Cancelled,
                });
            }
        }
    }

    /// Enqueues an upload transfer.
    pub fn enqueue_upload(
        &self,
        local_src: PathBuf,
        remote_dst: String,
        remote_client: RemoteClient,
    ) {
        let name = local_src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());

        let size = std::fs::metadata(&local_src).map(|m| m.len()).unwrap_or(0);
        let id = Uuid::new_v4().to_string();

        let item = TransferItem {
            id: id.clone(),
            name,
            source_path: local_src.to_string_lossy().to_string(),
            target_path: remote_dst.clone(),
            direction: TransferDirection::LocalToRemote,
            total_bytes: size,
            transferred_bytes: 0,
            speed_bps: 0,
            state: TransferState::Queued,
            started_at: None,
            error_msg: None,
        };

        self.items.lock().push(item.clone());
        let _ = self.event_tx.send(TransferEvent::ItemAdded(item));

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_tokens.lock().insert(id.clone(), cancel_flag.clone());

        let items_lock = self.items.clone();
        let event_tx = self.event_tx.clone();
        let cancel_tokens = self.cancel_tokens.clone();
        let limiter = self.concurrency_limiter.clone();
        let cancel_op = cancel_flag.clone();
        let cancel_cb = cancel_flag.clone();

        tokio::spawn(async move {
            // Bound concurrent transfers; the item stays Queued until a slot frees up.
            let _permit = limiter
                .acquire_owned()
                .await
                .expect("transfer semaphore closed");

            // A queued item may have been cancelled while waiting for a slot.
            if cancel_op.load(Ordering::SeqCst) {
                cancel_tokens.lock().remove(&id);
                return;
            }

            {
                let mut guard = items_lock.lock();
                if let Some(it) = guard.iter_mut().find(|i| i.id == id) {
                    if it.state != TransferState::Queued {
                        cancel_tokens.lock().remove(&id);
                        return;
                    }
                    it.state = TransferState::Running;
                    it.started_at = Some(Local::now());
                }
            }

            let mut last_progress_time = Instant::now();
            let mut last_bytes = 0u64;

            let tx_clone = event_tx.clone();
            let id_clone = id.clone();
            let items_lock_clone = items_lock.clone();

            let upload_res = remote_client
                .upload_file(local_src, remote_dst, cancel_op.clone(), move |written| {
                    if cancel_cb.load(Ordering::SeqCst) {
                        return;
                    }
                    let now = Instant::now();
                    let elapsed_sub = now.duration_since(last_progress_time).as_secs_f64();
                    let speed = if elapsed_sub >= 0.5 {
                        let delta = written.saturating_sub(last_bytes);
                        last_progress_time = now;
                        last_bytes = written;
                        (delta as f64 / elapsed_sub) as u64
                    } else {
                        0
                    };

                    {
                        let mut guard = items_lock_clone.lock();
                        if let Some(it) = guard.iter_mut().find(|i| i.id == id_clone) {
                            it.transferred_bytes = written;
                            if speed > 0 {
                                it.speed_bps = speed;
                            }
                        }
                    }

                    let _ = tx_clone.send(TransferEvent::ProgressUpdated {
                        id: id_clone.clone(),
                        transferred_bytes: written,
                        speed_bps: speed,
                    });
                })
                .await;

            let final_state = match upload_res {
                Ok(_) => TransferState::Completed,
                Err(e) => {
                    if cancel_op.load(Ordering::SeqCst) {
                        TransferState::Cancelled
                    } else {
                        error!("Upload failed: {e}");
                        TransferState::Failed(e)
                    }
                }
            };

            // A user cancellation wins over a racy worker completion, and the
            // reported event always matches the stored item state.
            let report_state = {
                let mut guard = items_lock.lock();
                match guard.iter_mut().find(|i| i.id == id) {
                    Some(it) if it.state == TransferState::Cancelled => TransferState::Cancelled,
                    Some(it) => {
                        it.state = final_state.clone();
                        if let TransferState::Failed(ref msg) = final_state {
                            it.error_msg = Some(msg.clone());
                        }
                        final_state.clone()
                    }
                    None => final_state.clone(),
                }
            };
            cancel_tokens.lock().remove(&id);

            let _ = event_tx.send(TransferEvent::ItemFinished {
                id,
                state: report_state,
            });
        });
    }

    /// Enqueues a download transfer.
    pub fn enqueue_download(
        &self,
        remote_src: String,
        local_dst: PathBuf,
        remote_client: RemoteClient,
        file_size: u64,
    ) {
        let name = Path::new(&remote_src)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());

        let id = Uuid::new_v4().to_string();

        let item = TransferItem {
            id: id.clone(),
            name,
            source_path: remote_src.clone(),
            target_path: local_dst.to_string_lossy().to_string(),
            direction: TransferDirection::RemoteToLocal,
            total_bytes: file_size,
            transferred_bytes: 0,
            speed_bps: 0,
            state: TransferState::Queued,
            started_at: None,
            error_msg: None,
        };

        self.items.lock().push(item.clone());
        let _ = self.event_tx.send(TransferEvent::ItemAdded(item));

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_tokens.lock().insert(id.clone(), cancel_flag.clone());

        let items_lock = self.items.clone();
        let event_tx = self.event_tx.clone();
        let cancel_tokens = self.cancel_tokens.clone();
        let limiter = self.concurrency_limiter.clone();
        let cancel_op = cancel_flag.clone();
        let cancel_cb = cancel_flag.clone();

        tokio::spawn(async move {
            // Bound concurrent transfers; the item stays Queued until a slot frees up.
            let _permit = limiter
                .acquire_owned()
                .await
                .expect("transfer semaphore closed");

            // A queued item may have been cancelled while waiting for a slot.
            if cancel_op.load(Ordering::SeqCst) {
                cancel_tokens.lock().remove(&id);
                return;
            }

            {
                let mut guard = items_lock.lock();
                if let Some(it) = guard.iter_mut().find(|i| i.id == id) {
                    if it.state != TransferState::Queued {
                        cancel_tokens.lock().remove(&id);
                        return;
                    }
                    it.state = TransferState::Running;
                    it.started_at = Some(Local::now());
                }
            }

            let mut last_progress_time = Instant::now();
            let mut last_bytes = 0u64;

            let tx_clone = event_tx.clone();
            let id_clone = id.clone();
            let items_lock_clone = items_lock.clone();

            let download_res = remote_client
                .download_file(remote_src, local_dst, cancel_op.clone(), move |read_bytes| {
                    if cancel_cb.load(Ordering::SeqCst) {
                        return;
                    }
                    let now = Instant::now();
                    let elapsed_sub = now.duration_since(last_progress_time).as_secs_f64();
                    let speed = if elapsed_sub >= 0.5 {
                        let delta = read_bytes.saturating_sub(last_bytes);
                        last_progress_time = now;
                        last_bytes = read_bytes;
                        (delta as f64 / elapsed_sub) as u64
                    } else {
                        0
                    };

                    {
                        let mut guard = items_lock_clone.lock();
                        if let Some(it) = guard.iter_mut().find(|i| i.id == id_clone) {
                            it.transferred_bytes = read_bytes;
                            if speed > 0 {
                                it.speed_bps = speed;
                            }
                        }
                    }

                    let _ = tx_clone.send(TransferEvent::ProgressUpdated {
                        id: id_clone.clone(),
                        transferred_bytes: read_bytes,
                        speed_bps: speed,
                    });
                })
                .await;

            let final_state = match download_res {
                Ok(_) => TransferState::Completed,
                Err(e) => {
                    if cancel_op.load(Ordering::SeqCst) {
                        TransferState::Cancelled
                    } else {
                        error!("Download failed: {e}");
                        TransferState::Failed(e)
                    }
                }
            };

            // A user cancellation wins over a racy worker completion, and the
            // reported event always matches the stored item state.
            let report_state = {
                let mut guard = items_lock.lock();
                match guard.iter_mut().find(|i| i.id == id) {
                    Some(it) if it.state == TransferState::Cancelled => TransferState::Cancelled,
                    Some(it) => {
                        it.state = final_state.clone();
                        if let TransferState::Failed(ref msg) = final_state {
                            it.error_msg = Some(msg.clone());
                        }
                        final_state.clone()
                    }
                    None => final_state.clone(),
                }
            };
            cancel_tokens.lock().remove(&id);

            let _ = event_tx.send(TransferEvent::ItemFinished {
                id,
                state: report_state,
            });
        });
    }
}
