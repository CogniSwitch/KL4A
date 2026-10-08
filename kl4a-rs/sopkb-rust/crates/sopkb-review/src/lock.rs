//! Per-bundle write lock. **New engineering requirement, not a Python port** --
//! Python's `ThreadingHTTPServer` does zero locking around the read-modify-write of
//! `items.json`/`reviews.json` (a latent race there), but Tauri commands run truly
//! concurrently, turning that latent bug into a live one. docs/port/PORT_PLAN.md P-W1:
//! "Without it the port is strictly worse than the Python, because a desktop UI can
//! fire commands faster than a human clicking through a browser." Introduced here
//! (Phase 6) since this is the first phase with a real read-modify-write cycle;
//! `sopkb-workbench`'s Tauri command layer (Phase 9) reuses this same lock so mining
//! and review actions against the same bundle also serialize against each other.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<PathBuf, Arc<Mutex<()>>>> {
    LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Canonicalizes when possible so differently-spelled paths to the same bundle (`./b`
/// vs an absolute path) share one lock; falls back to the raw path if canonicalization
/// fails (e.g. the directory doesn't exist yet).
fn lock_key(bundle_dir: &Path) -> PathBuf {
    std::fs::canonicalize(bundle_dir).unwrap_or_else(|_| bundle_dir.to_path_buf())
}

/// Runs `f` while holding the lock for `bundle_dir`, blocking any other thread doing
/// the same for the *same* bundle; unrelated bundles never block each other. Closure-
/// based (rather than returning a guard) to avoid a self-referential-struct lifetime
/// problem with a `MutexGuard` borrowed from inside an `Arc` we don't otherwise keep
/// alive by reference.
pub fn with_bundle_lock<T>(bundle_dir: &Path, f: impl FnOnce() -> T) -> T {
    let key = lock_key(bundle_dir);
    let bundle_mutex = {
        let mut reg = registry().lock().unwrap_or_else(|e| e.into_inner());
        reg.entry(key).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
    };
    let _guard = bundle_mutex.lock().unwrap_or_else(|e| e.into_inner());
    f()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    #[test]
    fn same_bundle_serializes_concurrent_writers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let concurrent = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let path = path.clone();
                let concurrent = concurrent.clone();
                let max_concurrent = max_concurrent.clone();
                thread::spawn(move || {
                    with_bundle_lock(&path, || {
                        let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                        max_concurrent.fetch_max(now, Ordering::SeqCst);
                        thread::sleep(std::time::Duration::from_millis(5));
                        concurrent.fetch_sub(1, Ordering::SeqCst);
                    });
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(max_concurrent.load(Ordering::SeqCst), 1, "critical sections must never overlap for the same bundle");
    }

    #[test]
    fn different_bundles_do_not_block_each_other() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let a_started = Arc::new(std::sync::Barrier::new(2));
        let a_started2 = a_started.clone();
        let path_a = dir_a.path().to_path_buf();
        let path_b = dir_b.path().to_path_buf();

        let handle = thread::spawn(move || {
            with_bundle_lock(&path_a, || {
                a_started2.wait();
                thread::sleep(std::time::Duration::from_millis(50));
            });
        });
        a_started.wait();
        // While thread A still holds bundle A's lock, bundle B's lock must be free.
        let completed = Arc::new(AtomicUsize::new(0));
        let completed2 = completed.clone();
        with_bundle_lock(&path_b, move || {
            completed2.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(completed.load(Ordering::SeqCst), 1);
        handle.join().unwrap();
    }
}
