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

    /// Drops the oldest finished items beyond the `keep` most-recent ones
    /// (active items are never touched), cleaning their cancel tokens.
    /// Returns the removed ids for UI mirror lists.
    pub fn prune_finished(&self, keep: usize) -> Vec<String> {
        fn is_done(i: &TransferItem) -> bool {
            !matches!(i.state, TransferState::Queued | TransferState::Running)
        }
        let mut items = self.items.lock();
        let finished = items.iter().filter(|i| is_done(i)).count();
        if finished <= keep {
            return Vec::new();
        }
        let mut to_drop = finished - keep;
        let mut removed = Vec::new();
        items.retain(|i| {
            if is_done(i) && to_drop > 0 {
                to_drop -= 1;
                removed.push(i.id.clone());
                false
            } else {
                true
            }
        });
        drop(items);
        let mut tokens = self.cancel_tokens.lock();
        for id in &removed {
            tokens.remove(id);
        }
        removed
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

        self.run_transfer_task(
            id,
            remote_client,
            TransferOp::Upload {
                local_src,
                remote_dst,
            },
        );
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

        self.run_transfer_task(
            id,
            remote_client,
            TransferOp::Download {
                remote_src,
                local_dst,
            },
        );
    }
    /// What a transfer task moves across the wire. Item bookkeeping is
    /// built by the `enqueue_*` callers; slots, cancellation, progress,
    /// and completion reporting run identically for both directions.
    #[allow(clippy::too_many_arguments)]
    fn run_transfer_task(&self, id: String, remote_client: RemoteClient, op: TransferOp) {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_tokens.lock().insert(id.clone(), cancel_flag.clone());

        let items_lock = self.items.clone();
        let event_tx = self.event_tx.clone();
        let cancel_tokens = self.cancel_tokens.clone();
        let limiter = self.concurrency_limiter.clone();
        let cancel_op = cancel_flag.clone();
        let cancel_cb = cancel_flag.clone();
        let label: &'static str = match op {
            TransferOp::Upload { .. } => "Upload",
            TransferOp::Download { .. } => "Download",
        };

        crate::runtime::spawn(async move {
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
            let mut last_send_time = Instant::now();

            let tx_clone = event_tx.clone();
            let id_clone = id.clone();
            let items_lock_clone = items_lock.clone();

            let io_res = match op {
                TransferOp::Upload {
                    local_src,
                    remote_dst,
                } => {
                    remote_client
                        .upload_file(local_src, remote_dst, cancel_op.clone(), move |bytes| {
                            report_progress(
                                bytes,
                                &cancel_cb,
                                &mut last_progress_time,
                                &mut last_bytes,
                                &mut last_send_time,
                                &tx_clone,
                                &id_clone,
                                &items_lock_clone,
                            );
                        })
                        .await
                }
                TransferOp::Download {
                    remote_src,
                    local_dst,
                } => {
                    remote_client
                        .download_file(
                            remote_src,
                            local_dst,
                            cancel_op.clone(),
                            move |bytes| {
                                report_progress(
                                    bytes,
                                    &cancel_cb,
                                    &mut last_progress_time,
                                    &mut last_bytes,
                                    &mut last_send_time,
                                    &tx_clone,
                                    &id_clone,
                                    &items_lock_clone,
                                );
                            },
                        )
                        .await
                }
            };

            let final_state = match io_res {
                Ok(_) => TransferState::Completed,
                Err(e) => {
                    if cancel_op.load(Ordering::SeqCst) {
                        TransferState::Cancelled
                    } else {
                        error!("{label} failed: {e}");
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

/// Shared per-chunk progress bookkeeping: updates the item every chunk,
/// but emits UI events at most ~10/s (each event triggers a repaint).
#[allow(clippy::too_many_arguments)]
fn report_progress(
    bytes: u64,
    cancel_cb: &AtomicBool,
    last_progress_time: &mut Instant,
    last_bytes: &mut u64,
    last_send_time: &mut Instant,
    tx: &mpsc::UnboundedSender<TransferEvent>,
    id: &str,
    items: &Arc<Mutex<Vec<TransferItem>>>,
) {
    if cancel_cb.load(Ordering::SeqCst) {
        return;
    }
    let now = Instant::now();
    let elapsed_sub = now.duration_since(*last_progress_time).as_secs_f64();
    let speed = if elapsed_sub >= 0.5 {
        let delta = bytes.saturating_sub(*last_bytes);
        *last_progress_time = now;
        *last_bytes = bytes;
        (delta as f64 / elapsed_sub) as u64
    } else {
        0
    };

    {
        let mut guard = items.lock();
        if let Some(it) = guard.iter_mut().find(|i| i.id == id) {
            it.transferred_bytes = bytes;
            if speed > 0 {
                it.speed_bps = speed;
            }
        }
    }

    if now.duration_since(*last_send_time).as_secs_f64() >= 0.1 {
        *last_send_time = now;
        let _ = tx.send(TransferEvent::ProgressUpdated {
            id: id.to_string(),
            transferred_bytes: bytes,
            speed_bps: speed,
        });
    }
}

#[derive(Debug)]
enum TransferOp {
    Upload {
        local_src: PathBuf,
        remote_dst: String,
    },
    Download {
        remote_src: String,
        local_dst: PathBuf,
    },
}

