//! Implements simple `Future` based `Task`s.
use alloc::boxed::Box;
use core::sync::atomic::{AtomicU64, Ordering};
use core::task::{Context, Poll};
use core::{future, pin::Pin};

pub mod executor;
pub mod keyboard;
pub mod simple_executor;

pub use executor::{spawn_task, Executor};

/// Stores a unique ID that is used by executors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(u64);

/// Trait alias for convenience.
pub trait Future = future::Future<Output = ()> + Send + Sync;

impl TaskId {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// A task. Contains a `Future` and a `TaskId`.
pub struct Task {
    id: TaskId,
    future: Pin<Box<dyn Future>>,
}

impl Task {
    /// Creates a new `Task`.
    pub fn new(future: impl Future + 'static) -> Task {
        Task {
            id: TaskId::new(),
            future: Box::pin(future),
        }
    }

    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}
