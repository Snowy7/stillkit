//! [`Waker`]: ends the UI thread's idle wait from another thread, so an app whose workers
//! (a socket reader, a file watcher, a renderer) produce results can sleep in
//! `host::wait` with no timer and still react the moment a result lands.
//!
//! ```ignore
//! let waker = stillkit::waker();               // on the UI thread
//! std::thread::spawn(move || { tx.send(work()); waker.wake(); });
//! loop { host::wait(); while let Ok(r) = rx.try_recv() { apply(r) } /* events, frame */ }
//! ```
//!
//! Wakes coalesce: many `wake` calls between two waits post one native message. The flag
//! re-arms as each wait returns, before the app drains its channels, so a result sent while
//! the app drains posts again and ends the next wait; one sent earlier is drained now.

use std::cell::OnceCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::platform::current::host::Poster;

/// Ends the UI thread's [`wait`](crate::platform::current::host::wait) from any thread.
/// Cheap to clone; made on the UI thread by [`waker`].
#[derive(Clone)]
pub struct Waker(Arc<Shared>);

struct Shared {
    /// A wake was posted and the UI thread hasn't started a wait since.
    pending: AtomicBool,
    post: Poster,
}

thread_local! {
    static CURRENT: OnceCell<Arc<Shared>> = const { OnceCell::new() };
}

/// The calling (UI) thread's waker, made on first use.
pub fn waker() -> Waker {
    Waker(CURRENT.with(|c| c.get_or_init(|| Arc::new(Shared { pending: AtomicBool::new(false), post: Poster::new() })).clone()))
}

impl Waker {
    /// Makes the UI thread's current or next wait return. Safe from any thread.
    pub fn wake(&self) {
        if !self.0.pending.swap(true, Ordering::AcqRel) {
            self.0.post.post();
        }
    }
}

impl std::fmt::Debug for Waker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waker").field("pending", &self.0.pending.load(Ordering::Relaxed)).finish()
    }
}

/// Called by each platform's wait as it returns, before the app drains: later wakes post again.
pub(crate) fn rearm() {
    CURRENT.with(|c| {
        if let Some(shared) = c.get() {
            shared.pending.store(false, Ordering::Release);
        }
    });
}
