//! Rust's answer to Python's `ThreadPoolExecutor(max_workers=N).map(fn, items)`,
//! which this port's LLM-fan-out call sites (`sopkb_mining::okf_author::mine_with_author`,
//! `sopkb_workbench::heading_restructure::build_heading_index`) both need and which
//! nothing in `std` provides directly. Real oss-launch inlines a fresh
//! `ThreadPoolExecutor` at each call site rather than sharing a wrapper -- the
//! wrapper exists here only because the underlying THREADING MECHANICS (a bounded
//! worker pool, order-preserving results) are non-trivial boilerplate in Rust in a
//! way they are not in Python, not because either call site's own logic is shared.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Runs `f(index, &items[index])` for every item, spread across up to
/// `max_workers` OS threads pulling from one shared work queue (a plain atomic
/// counter -- work-stealing, not static partitioning, so one slow item never
/// leaves other workers idle with unclaimed work). Results are returned in
/// the SAME ORDER as `items`, regardless of which thread finished which index
/// first -- matching `executor.map`'s own ordering guarantee, which every
/// caller of this port's ported functions already depends on (e.g. matching a
/// per-chunk offset or a per-section ordinal back up with its own input).
///
/// `f` itself decides how to represent a per-item failure (e.g. `Result<T, E>`
/// as `R`) -- this function has no concept of failure of its own and never
/// short-circuits early, exactly like `executor.map` collecting every result
/// (successful or not) before the caller inspects them.
pub fn parallel_map<T, R, F>(items: &[T], max_workers: usize, f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(usize, &T) -> R + Sync,
{
    let total = items.len();
    if total == 0 {
        return Vec::new();
    }
    let worker_count = max_workers.min(total).max(1);
    let next_index = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<R>>> = (0..total).map(|_| Mutex::new(None)).collect();

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| loop {
                let i = next_index.fetch_add(1, Ordering::SeqCst);
                if i >= total {
                    break;
                }
                let result = f(i, &items[i]);
                *results[i].lock().unwrap() = Some(result);
            });
        }
    });

    results
        .into_iter()
        .map(|m| m.into_inner().unwrap().expect("every index is visited exactly once by exactly one worker"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    #[test]
    fn preserves_input_order_regardless_of_completion_order() {
        // Item 0 sleeps longest, so if this returned in COMPLETION order rather
        // than INPUT order, index 0's result would land last, not first.
        let items = [30u64, 20, 10, 0];
        let results = parallel_map(&items, 4, |_i, &sleep_ms| {
            std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
            sleep_ms
        });
        assert_eq!(results, vec![30, 20, 10, 0]);
    }

    #[test]
    fn respects_a_worker_count_smaller_than_the_item_count() {
        let items: Vec<u32> = (0..20).collect();
        let concurrent = AtomicU32::new(0);
        let max_concurrent = AtomicU32::new(0);
        let results = parallel_map(&items, 3, |_i, &v| {
            let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
            max_concurrent.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(5));
            concurrent.fetch_sub(1, Ordering::SeqCst);
            v * 2
        });
        assert_eq!(results, (0..20).map(|v| v * 2).collect::<Vec<_>>());
        assert!(max_concurrent.load(Ordering::SeqCst) <= 3, "never more than max_workers ran at once");
    }

    #[test]
    fn worker_count_is_clamped_to_the_item_count() {
        // Asking for 100 workers over 2 items must not panic or spawn more
        // threads than there is work for.
        let items = [1, 2];
        let results = parallel_map(&items, 100, |_i, &v| v * 10);
        assert_eq!(results, vec![10, 20]);
    }

    #[test]
    fn empty_input_returns_empty_output_without_spawning_anything() {
        let items: [u32; 0] = [];
        let results = parallel_map(&items, 6, |_i, &v: &u32| v);
        assert!(results.is_empty());
    }

    #[test]
    fn every_item_is_visited_exactly_once() {
        let items: Vec<u32> = (0..50).collect();
        let visits = std::sync::Mutex::new(vec![0u32; 50]);
        parallel_map(&items, 6, |i, &_v| {
            visits.lock().unwrap()[i] += 1;
        });
        assert!(visits.into_inner().unwrap().iter().all(|&count| count == 1));
    }
}
