use std::time::{Duration, Instant};

use rift::runtime;

/// Regression test for the startup crash ("there is no reactor running").
/// GPUI executor threads carry no Tokio context, so every runtime call the
/// app makes must go through the global bridge. These plain `#[test]`s
/// (deliberately *not* `#[tokio::test]`) prove the bridge works with no
/// ambient runtime — the exact condition that used to panic.
#[test]
fn test_bridge_spawn_blocking_without_runtime() {
    let joined = runtime::spawn_blocking(|| {
        std::thread::current()
            .name()
            .unwrap_or_default()
            .to_string()
    });
    let name = runtime::handle().block_on(joined).unwrap();
    assert!(
        name.contains("rift-tokio"),
        "ran on pool thread, got {name}"
    );
}

#[test]
fn test_bridge_spawn_without_runtime() {
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = done.clone();
    let task = runtime::spawn(async move {
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
        7u32
    });
    let value = runtime::handle().block_on(task).unwrap();
    assert_eq!(value, 7);
    assert!(done.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn test_bridge_timeout_without_runtime() {
    let t = Instant::now();
    let fast: Result<u32, String> = runtime::handle()
        .block_on(runtime::timeout(Duration::from_secs(5), async { 1u32 }));
    assert_eq!(fast, Ok(1));
    assert!(t.elapsed() < Duration::from_secs(5));

    let slow: Result<u32, String> =
        runtime::handle().block_on(runtime::timeout(Duration::from_millis(50), async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            2u32
        }));
    assert!(slow.is_err());
}
