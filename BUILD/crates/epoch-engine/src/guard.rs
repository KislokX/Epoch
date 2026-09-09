//! Taking a lock without turning one panic into a dead window.
//!
//! ## The failure this removes
//!
//! `std::sync::Mutex` poisons itself when a thread panics while holding it, and every later
//! `lock().expect(…)` then panics too. Epoch's locks are held across IPC commands, so a single
//! panic anywhere — one bad unwrap in a provider, one arithmetic slip in a projection —
//! **spread**: the next command that touched the same lock died, then the next, and a window
//! that was still on screen answered nothing forever. One fault became a permanent one.
//!
//! There were 53 of those, and the `World`'s own mutex was already the exception: it recovered
//! by hand, with a comment saying why. This is that comment made into the only way to take a
//! lock.
//!
//! ## Why recovering is right *here*, and would not be everywhere
//!
//! Poisoning exists because a panic mid-write can leave data half-updated, and continuing to
//! read it is a real hazard. That argument holds for a value with an invariant spanning several
//! fields. It does not hold for what Epoch keeps behind locks: a provider registry, an MCP
//! bridge, a map of open terminals, a door token, a cache of measurements. Every one of them is
//! replaced wholesale rather than edited in place, and the worst a recovered guard can hand back
//! is a stale entry that the next write overwrites.
//!
//! Against that: a World that stops answering, with no message, because something unrelated
//! panicked twenty minutes ago. The trade is not close.
//!
//! **This is not a way to ignore panics.** A poisoned lock still means something failed and was
//! reported where it failed; this only stops the failure travelling.

use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Take a mutex, recovering it if a panic left it poisoned.
pub fn guard<T>(lock: &Mutex<T>) -> MutexGuard<'_, T> {
    lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Read a lock, recovering it if a panic left it poisoned.
pub fn reading<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Write a lock, recovering it if a panic left it poisoned.
pub fn writing<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_lock_a_panic_poisoned_still_opens() {
        // The whole point, in one test: something panicked while holding it, and everything
        // after it carried on. Before this, the second line was the one that killed the window.
        let held = Arc::new(Mutex::new(vec!["before".to_string()]));
        let doomed = Arc::clone(&held);
        let _ = std::thread::spawn(move || {
            let _open = doomed.lock().expect("a fresh lock");
            panic!("something went wrong over here");
        })
        .join();

        assert!(held.lock().is_err(), "the lock really is poisoned");

        {
            let mut open = guard(&held);
            assert_eq!(open.as_slice(), ["before"]);
            open.push("after".into());
        }
        // **Scoped, and the braces are the test.** Written without them, the guard above was
        // still alive here and this line waited on a lock the same thread already held —
        // `std::sync::Mutex` is not reentrant, and that is the exact defect this project spent
        // a day removing from a model turn (commit 47bbec1). It hung the whole suite.
        assert_eq!(guard(&held).len(), 2);
    }

    #[test]
    fn a_read_write_lock_recovers_the_same_way() {
        let held = Arc::new(RwLock::new(1_u32));
        let doomed = Arc::clone(&held);
        let _ = std::thread::spawn(move || {
            let _open = doomed.write().expect("a fresh lock");
            panic!("and over here");
        })
        .join();

        assert_eq!(*reading(&held), 1);
        *writing(&held) = 2;
        assert_eq!(*reading(&held), 2);
    }
}
