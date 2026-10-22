//! What a count costs when threads share its cache line: one count all threads add to, a count
//! each side by side, which still share a line, and a count each in crossbeam's `CachePadded`, on a
//! line of its own. Prints the nanoseconds an add takes in each.
//!
//! The threads are not pinned to cores, so the figures move from run to run; the bench
//! `contention` pins each, for figures that hold still.
//!
//! ```text
//! cargo run --release -p atomix-rs --example counters
//! ```

#![expect(clippy::print_stdout, reason = "an example prints what it measured")]

use core::array;
use std::thread;
use std::time::Instant;

use atomix::AtomicU64;
use atomix::ordering::Relaxed;
use crossbeam_utils::CachePadded;

/// The threads that count.
const THREADS: usize = 4;
/// How many adds each thread makes.
const ADDS: u32 = 10_000_000;

// ANCHOR: counts
/// A count for each thread, twice over.
struct Counts {
    /// Side by side, so the counts share a cache line.
    side_by_side: [AtomicU64; THREADS],
    /// Each in crossbeam's `CachePadded`, on a cache line of its own.
    padded: [CachePadded<AtomicU64>; THREADS],
}
// ANCHOR_END: counts

/// Prints what an add costs where each of `THREADS` threads adds through `add`, which takes the
/// thread's index.
fn time(name: &str, add: impl Fn(usize) + Sync) {
    let start = Instant::now();
    thread::scope(|scope| {
        for index in 0..THREADS {
            let add = &add;
            scope.spawn(move || (0..ADDS).for_each(|_| add(index)));
        }
    });
    let nanoseconds = start.elapsed().as_secs_f64() * 1e9 / f64::from(ADDS);
    println!("{name:>28}: {nanoseconds:5.1} ns an add, on each of {THREADS} threads");
}

#[expect(clippy::indexing_slicing, reason = "each thread's index is below `THREADS`")]
fn main() {
    // ORDERING: Relaxed throughout: each count is read once its threads are joined.
    let shared = AtomicU64::new(0);
    time("one shared count", |_| {
        shared.fetch_add(1, Relaxed);
    });
    let counts = Counts {
        side_by_side: array::from_fn(|_| AtomicU64::new(0)),
        padded: array::from_fn(|_| CachePadded::default()),
    };
    time("a count each, side by side", |index| {
        counts.side_by_side[index].fetch_add(1, Relaxed);
    });
    time("a count each, padded", |index| {
        counts.padded[index].fetch_add(1, Relaxed);
    });

    let side_by_side_total: u64 = counts.side_by_side.iter().map(|count| count.load(Relaxed)).sum();
    let padded_total: u64 = counts.padded.iter().map(|count| count.load(Relaxed)).sum();
    assert_eq!(side_by_side_total, shared.load(Relaxed), "every add counted, side by side");
    assert_eq!(padded_total, shared.load(Relaxed), "and padded");
}
