//! Tokio runtime used by synchronous tool entry points and background tasks.

use std::future::Future;

pub use tokio::task::JoinHandle;
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

pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    runtime().spawn(future)
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    runtime().block_on(future)
}
