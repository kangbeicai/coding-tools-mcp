//! Small runtime abstraction used by the UI-independent core.
//!
//! Desktop builds keep using Tauri's runtime so existing lifecycle behaviour is
//! unchanged. Headless builds deliberately avoid linking Tauri and use a
//! dedicated Tokio runtime for synchronous tool entry points and background
//! process/session tasks.

use std::future::Future;

#[cfg(feature = "desktop")]
pub use tauri::async_runtime::JoinHandle;

#[cfg(not(feature = "desktop"))]
pub use tokio::task::JoinHandle;

#[cfg(feature = "desktop")]
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tauri::async_runtime::spawn(future)
}

#[cfg(feature = "desktop")]
pub fn block_on<F: Future>(future: F) -> F::Output {
    tauri::async_runtime::block_on(future)
}

#[cfg(not(feature = "desktop"))]
fn runtime() -> &'static tokio::runtime::Runtime {
    use std::sync::LazyLock;

    static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("coding-tools-core")
            .build()
            .expect("failed to initialize Coding Tools Tokio runtime")
    });
    &RUNTIME
}

#[cfg(not(feature = "desktop"))]
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    runtime().spawn(future)
}

#[cfg(not(feature = "desktop"))]
pub fn block_on<F: Future>(future: F) -> F::Output {
    runtime().block_on(future)
}
