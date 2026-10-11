use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferDirection {
    LocalToRemote,
    RemoteToLocal,
    LocalToLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferState {
    Queued,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictPolicy {
    Ask,
    Overwrite,
    Skip,
    Rename,
    OverwriteAll,
    SkipAll,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferItem {
    pub id: String,
    pub name: String,
    pub source_path: String,
    pub target_path: String,
    pub direction: TransferDirection,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub speed_bps: u64,
    pub state: TransferState,
    pub started_at: Option<DateTime<Local>>,
    pub error_msg: Option<String>,
}

impl TransferItem {
    pub fn progress_ratio(&self) -> f32 {
        if self.total_bytes == 0 {
            if self.state == TransferState::Completed {
                1.0
            } else {
                0.0
            }
        } else {
            (self.transferred_bytes as f32 / self.total_bytes as f32).clamp(0.0, 1.0)
        }
    }

    pub fn formatted_progress(&self) -> String {
        let percent = (self.progress_ratio() * 100.0) as u32;
        format!("{}%", percent)
    }
}
