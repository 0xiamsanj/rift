use tokio::sync::mpsc;

use rift::fs::types::{format_bytes, format_speed};
use rift::transfer::types::{TransferDirection, TransferItem, TransferState};
use rift::transfer::queue::{TransferEvent, TransferManager};

#[test]
fn test_byte_formatting() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    assert_eq!(format_bytes(1024 * 1024 * 1024 * 1024), "1.00 TB");
}

#[test]
fn test_speed_formatting() {
    assert_eq!(format_speed(0), "0 B/s");
    assert_eq!(format_speed(1024), "1.0 KB/s");
    assert_eq!(format_speed(5 * 1024 * 1024), "5.0 MB/s");
}

#[test]
fn test_transfer_item_progress() {
    let mut item = TransferItem {
        id: "test-id-1".to_string(),
        name: "test.zip".to_string(),
        source_path: "/local/test.zip".to_string(),
        target_path: "/remote/test.zip".to_string(),
        direction: TransferDirection::LocalToRemote,
        total_bytes: 1000,
        transferred_bytes: 500,
        speed_bps: 100 * 1024,
        state: TransferState::Running,
        started_at: None,
        error_msg: None,
    };

    assert_eq!(item.progress_ratio(), 0.5);
    assert_eq!(item.formatted_progress(), "50%");

    item.total_bytes = 0;
    assert_eq!(item.progress_ratio(), 0.0);
}

#[test]
fn test_transfer_manager_respects_max_concurrency_setting() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let default_manager = TransferManager::new(tx.clone());
    assert_eq!(default_manager.max_concurrency(), 2);

    let (tx2, _rx2) = mpsc::unbounded_channel::<TransferEvent>();
    let wide = TransferManager::with_max_concurrency(tx2, 8);
    assert_eq!(wide.max_concurrency(), 8);

    // Zero/clamped input always yields at least one slot.
    let (tx3, _rx3) = mpsc::unbounded_channel::<TransferEvent>();
    let clamped = TransferManager::with_max_concurrency(tx3, 0);
    assert_eq!(clamped.max_concurrency(), 1);
}

#[tokio::test]
async fn test_transfer_manager_cancellation() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let manager = TransferManager::new(tx);

    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("upload.bin");
    std::fs::write(&file_path, vec![0u8; 1024 * 100]).unwrap();

    let client = rift::remote::RemoteClient::new();
    manager.enqueue_upload(
        file_path,
        "/remote/upload.bin".to_string(),
        client,
    );

    // Should receive ItemAdded event
    if let Some(event) = rx.recv().await {
        match event {
            TransferEvent::ItemAdded(item) => {
                assert_eq!(item.name, "upload.bin");
                manager.cancel_transfer(&item.id);
            }
            _ => panic!("Expected ItemAdded event"),
        }
    }

    let items = manager.get_items();
    assert_eq!(items.len(), 1);
    
    // Clear completed/cancelled
    manager.clear_completed();
    let remaining = manager.get_items();
    assert_eq!(remaining.len(), 0);
}
