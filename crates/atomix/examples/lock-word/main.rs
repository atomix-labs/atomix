//! Threads take turns at a lock word, each under an owner's id, and count what the lock guards:
//! a count whose load and store, each Relaxed, would lose a count wherever two threads overlapped,
//! so the total shows the lock kept them apart. Prints the count and the turns a second.
//!
//! ```text
//! cargo run --release -p atomix-rs --features derive --example lock-word
//! ```

#![cfg_attr(not(loom), expect(clippy::print_stdout, reason = "an example prints what it measured"))]
#![cfg_attr(
    not(loom),
    expect(clippy::expect_used, reason = "no holder panics, so no lock is poisoned")
)]

#[cfg(not(loom))]
mod lock_word;

/// Loom's atomics exist only inside a model: `tests/model.rs` models this lock.
#[cfg(loom)]
fn main() {}

#[cfg(not(loom))]
fn main() {
    use std::thread;
    use std::time::Instant;

    use atomix::ordering::Relaxed;
    use atomix::{AtomicU64, RangedU64};
    use lock_word::{LockWord, OwnerId};

    /// The threads that take turns.
    const THREADS: u32 = 4;
    /// How many turns each takes.
    const TURNS: u32 = 1_000_000;

    let lock = LockWord::new();
    let count = AtomicU64::new(0);
    let start = Instant::now();
    thread::scope(|scope| {
        for thread in 0..THREADS {
            let owner = OwnerId(RangedU64::MIN.saturating_add(u64::from(thread)));
            let (lock, count) = (&lock, &count);
            scope.spawn(move || {
                for _ in 0..TURNS {
                    let _guard = lock.lock(owner).expect("no holder panics");
                    // Two accesses, not one read-modify-write: the lock keeps them together.
                    count.store(count.load(Relaxed).wrapping_add(1), Relaxed);
                }
            });
        }
    });
    let seconds = start.elapsed().as_secs_f64();

    let turns = u64::from(THREADS).saturating_mul(u64::from(TURNS));
    assert_eq!(count.load(Relaxed), turns, "no turn overlapped another");
    let rate = f64::from(THREADS) * f64::from(TURNS) / seconds;
    println!("{THREADS} threads took {turns} turns, none lost: {rate:.0} turns a second");
}
