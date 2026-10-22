//! What an operation costs while other cores contend for its cache line, on 1, 2, 4 and 8 threads
//! at once, each pinned to a core of its own.
//!
//! Four cases. A shared count every thread adds to: core's add against atomix's, the same
//! instruction. A count of each thread's own: side by side in one cache line, which each add takes
//! from the other cores, against each in a line of its own through crossbeam's `CachePadded`. A
//! 16-byte read by every thread: a `load`, one `ldp` or `vmovdqa`, which leaves the line shared,
//! against a `load_rmw`, a compare-exchange, which takes it, where both exist. And a claim: every
//! thread sets one bit of a shared word and clears it where it was the one to set it, through a
//! field's `test_and_set` against core's `fetch_or` by hand.
//!
//! Thread `i` pins itself to the `i`th core of the process's affinity mask, so `taskset` places the
//! run, on a mask of eight cores or more; macOS pins nothing, so there each thread runs where the
//! scheduler puts it. Each thread times its operations in rounds of a batch, after a warm-up and a
//! barrier, and keeps contending, untimed, until every thread is done, so no round is timed against
//! fewer threads than its row names. A row reports, in nanoseconds an operation, the fastest and
//! the slowest thread's median round, and the highest 99th percentile and maximum of any thread.
//! Each operation is a call through a pointer; the first row, an empty operation, times it alone.
//!
//! ```text
//! cargo bench -p atomix-rs --features derive --bench contention --no-run
//! taskset -c 20-27 cargo bench -p atomix-rs --features derive --bench contention
//! ```

/// Loom's atomics exist only inside a model, so a loom build has nothing to time; and the pinning
/// crate, `core_affinity`, does not build for `arm64ec`, whose winapi lacks a thread's context.
#[cfg(any(loom, target_arch = "arm64ec"))]
fn main() {}

#[cfg(not(any(loom, target_arch = "arm64ec")))]
fn main() -> Result<(), harness::BoxError> {
    harness::run()
}

/// The cases, the threads that run them, and the report.
#[cfg(not(any(loom, target_arch = "arm64ec")))]
mod harness {
    use core::error::Error;
    use core::hint::black_box;
    use core::sync::atomic::{AtomicU64 as CoreU64, AtomicUsize, Ordering};
    use core::time::Duration;
    use std::io::{self, Write};
    use std::sync::Barrier;
    use std::thread;
    use std::time::Instant;

    #[cfg(any(target_arch = "aarch64", target_feature = "cmpxchg16b"))]
    use atomix::AtomicU128;
    #[cfg(any(target_arch = "aarch64", target_feature = "cmpxchg16b"))]
    use atomix::ordering::Acquire;
    use atomix::ordering::{AcqRel, Relaxed, Release};
    use atomix::{Atom, Atomic, AtomicU64};
    use core_affinity::{CoreId, get_core_ids, set_for_current};
    use crossbeam_utils::CachePadded;

    /// A failure `main` reports.
    pub(super) type BoxError = Box<dyn Error + Send + Sync>;

    /// The thread counts each case runs at.
    const THREADS: [usize; 4] = [1, 2, 4, 8];
    /// The rounds each thread runs before it waits for the others, untimed.
    const WARM_UP: usize = 500;
    /// The rounds each thread times.
    const ROUNDS: usize = 5_000;
    /// The operations a round.
    const BATCH: u32 = 1_000;

    /// A resting quote whose `live` bit, bit 32, the claim takes.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Quote {
        /// The quantity resting.
        quantity: u32,
        /// Whether a thread holds the quote.
        live: bool,
    }

    /// `live`'s bit, by hand.
    const LIVE: u64 = 1 << 32;

    /// Eight counts in one cache line.
    #[repr(align(64))]
    struct Line([AtomicU64; 8]);

    /// Two forms of one atomic, in one cache line, so both share the place of the line's home in
    /// the interconnect, which moves a contended operation's time from run to run.
    #[repr(align(64))]
    struct Twins<A, B> {
        /// The form written by hand over core's atomic.
        by_hand: A,
        /// atomix's.
        atomix: B,
    }

    /// The count every thread adds to.
    static COUNT: Twins<CoreU64, AtomicU64> =
        Twins { by_hand: CoreU64::new(0), atomix: AtomicU64::new(0) };
    /// A count per thread, side by side in one line.
    static ADJACENT: Line = Line([const { AtomicU64::new(0) }; 8]);
    /// A count per thread, each in a line of its own.
    static PADDED: [CachePadded<AtomicU64>; 8] = [const { CachePadded::new(AtomicU64::new(0)) }; 8];
    /// The 16 bytes every thread reads.
    #[cfg(any(target_arch = "aarch64", target_feature = "cmpxchg16b"))]
    static WIDE: CachePadded<AtomicU128> = CachePadded::new(AtomicU128::new(7));
    /// The quote every thread claims.
    static QUOTE: Twins<CoreU64, Atomic<Quote>> = Twins {
        by_hand: CoreU64::new(300),
        atomix: Atomic::new(Quote { quantity: 300, live: false }),
    };

    /// A case: its name, and its operation, done by the thread of the index it is given.
    struct Case {
        /// The group and the form, as the report names them.
        name: &'static str,
        /// One operation.
        operation: fn(usize),
    }

    // ORDERING: each shared count's add and each claim's set are `AcqRel`, and each 16-byte read
    // `Acquire`, as `operations`' are, so a row reads beside its figure on one thread; a claim's
    // `Release` clear pairs with the next set's `Acquire`, handing the quote on. The own counts
    // publish nothing: `Relaxed`.
    /// Every case this target runs.
    const CASES: &[Case] = &[
        Case { name: "floor/empty", operation: |_| {} },
        Case {
            name: "shared-count/by-hand",
            operation: |_| {
                black_box(COUNT.by_hand.fetch_add(1, Ordering::AcqRel));
            },
        },
        Case {
            name: "shared-count/atomix",
            operation: |_| {
                black_box(COUNT.atomix.fetch_add(1, AcqRel));
            },
        },
        Case {
            name: "own-count/adjacent",
            operation: |index| {
                if let Some(count) = ADJACENT.0.get(index) {
                    black_box(count.fetch_add(1, Relaxed));
                }
            },
        },
        Case {
            name: "own-count/padded",
            operation: |index| {
                if let Some(count) = PADDED.get(index) {
                    black_box(count.fetch_add(1, Relaxed));
                }
            },
        },
        #[cfg(all(
            any(target_arch = "aarch64", target_feature = "cmpxchg16b"),
            any(target_feature = "lse2", target_feature = "avx")
        ))]
        Case {
            name: "wide-read/load",
            operation: |_| {
                black_box(WIDE.load(Acquire));
            },
        },
        #[cfg(any(target_arch = "aarch64", target_feature = "cmpxchg16b"))]
        Case {
            name: "wide-read/load-rmw",
            operation: |_| {
                black_box(WIDE.load_rmw(Acquire));
            },
        },
        Case {
            name: "claim/by-hand",
            operation: |_| {
                if QUOTE.by_hand.fetch_or(LIVE, Ordering::AcqRel) & LIVE == 0 {
                    QUOTE.by_hand.fetch_and(!LIVE, Ordering::Release);
                }
            },
        },
        Case {
            name: "claim/atomix",
            operation: |_| {
                if !QUOTE.atomix.fields().live.test_and_set(AcqRel) {
                    QUOTE.atomix.fields().live.clear(Release);
                }
            },
        },
    ];

    /// Runs every case at each thread count, and writes a line for each.
    pub(super) fn run() -> Result<(), BoxError> {
        let cores = get_core_ids().ok_or("the affinity mask cannot be read")?;
        let most = THREADS.into_iter().max().unwrap_or_default();
        if cores.len() < most {
            let found = cores.len();
            return Err(format!(
                "{most} threads need {most} cores in the affinity mask, which has {found}: place \
                 the run with `taskset -c`, as `taskset -c 20-27`"
            )
            .into());
        }
        let mut out = io::stdout().lock();
        for case in CASES {
            for threads in THREADS {
                let timings = contend(case, cores.iter().copied().take(threads))?;
                let medians = timings.iter().map(|rounds| nearest_rank(rounds, 500));
                let (fastest, slowest) = (medians.clone().min(), medians.max());
                let tail = timings.iter().map(|rounds| nearest_rank(rounds, 990)).max();
                let maximum = timings.iter().filter_map(|rounds| rounds.last()).max().copied();
                let [fastest, slowest, tail, maximum] = [fastest, slowest, tail, maximum]
                    .map(|round| per_operation(round.unwrap_or_default()));
                writeln!(
                    out,
                    "{} t={threads}: median {fastest:.2}..{slowest:.2} ns, p99 {tail:.2} ns, max \
                     {maximum:.2} ns",
                    case.name
                )?;
            }
        }
        Ok(())
    }

    /// Runs `case` on a thread pinned to each of `cores` at once,
    /// and returns each thread's rounds, sorted.
    fn contend(
        case: &Case, cores: impl ExactSizeIterator<Item = CoreId>,
    ) -> Result<Vec<Vec<Duration>>, BoxError> {
        let (count, finished) = (cores.len(), AtomicUsize::new(0));
        let barrier = Barrier::new(count);
        thread::scope(|scope| {
            let threads: Vec<_> = cores
                .enumerate()
                .map(|(index, core)| {
                    let (barrier, finished) = (&barrier, &finished);
                    scope.spawn(move || {
                        // macOS sets no thread's affinity, so there each runs where it is put.
                        let pinned = cfg!(target_os = "macos") || set_for_current(core);
                        for _ in 0..WARM_UP {
                            round(case, index);
                        }
                        barrier.wait();
                        let mut rounds: Vec<_> = (0..ROUNDS).map(|_| round(case, index)).collect();
                        // ORDERING: `Relaxed`: the count publishes nothing; the join orders the
                        // rounds.
                        finished.fetch_add(1, Ordering::Relaxed);
                        while finished.load(Ordering::Relaxed) < count {
                            round(case, index);
                        }
                        rounds.sort_unstable();
                        (pinned, rounds)
                    })
                })
                .collect();
            threads
                .into_iter()
                .map(|thread| match thread.join() {
                    Ok((true, rounds)) => Ok(rounds),
                    Ok((false, _)) => Err("a thread could not pin itself to its core".into()),
                    Err(_panic_payload) => {
                        Err(format!("a thread of `{}` panicked", case.name).into())
                    },
                })
                .collect()
        })
    }

    /// Runs one round of `case`'s operation by the thread `index`, and returns how long it took.
    fn round(case: &Case, index: usize) -> Duration {
        let start = Instant::now();
        for _ in 0..BATCH {
            (case.operation)(black_box(index));
        }
        start.elapsed()
    }

    /// The round at `per_mille` of the sorted `rounds`.
    fn nearest_rank(rounds: &[Duration], per_mille: usize) -> Duration {
        let rank = rounds.len().saturating_mul(per_mille) / 1_000;
        rounds.get(rank.min(rounds.len().saturating_sub(1))).copied().unwrap_or_default()
    }

    /// What one operation of a round took, in nanoseconds.
    fn per_operation(round: Duration) -> f64 {
        round.as_secs_f64() * 1e9 / f64::from(BATCH)
    }
}
