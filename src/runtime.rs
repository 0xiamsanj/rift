use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;

use tokio::runtime::{Builder, Handle, Runtime};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .enable_all()
            .thread_name("rift-tokio")
            .build()
            .expect("failed to start Tokio runtime")
    })
}

/// Clone of the global runtime handle. Unlike `tokio::task::spawn*`, this
/// works from any thread — including GPUI executor threads, which carry no
/// Tokio context (calling those there panics with "no reactor running").
pub fn handle() -> Handle {
    runtime().handle().clone()
}

/// `tokio::task::spawn_blocking` without requiring a runtime context.
pub fn spawn_blocking<F, R>(f: F) -> tokio::task::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    handle().spawn_blocking(f)
}

/// `tokio::spawn` without requiring a runtime context.
pub fn spawn<F>(f: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    handle().spawn(f)
}

/// `tokio::time::timeout` without requiring a runtime context. The timed
/// future hops onto the runtime so timers resolve there.
pub async fn timeout<F>(duration: Duration, fut: F) -> Result<F::Output, String>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    handle()
        .spawn(async move {
            tokio::time::timeout(duration, fut)
                .await
                .map_err(|_| "timed out".to_string())
        })
        .await
        .map_err(|e| format!("Task execution error: {e}"))?
}
