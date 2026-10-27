//! A feed posts quotes to a book's levels while takers race to take them: each quote is one word,
//! taken by one field's test-and-clear and freed by another's clear. Prints how much was posted
//! and taken, which match.
//!
//! ```text
//! cargo run --release -p atomix --features derive --example quote-book
//! ```

#![cfg_attr(not(loom), expect(clippy::print_stdout, reason = "an example prints what it counted"))]
#![cfg_attr(not(loom), expect(clippy::expect_used, reason = "no taker panics"))]

#[cfg(not(loom))]
mod quote_book;

/// Loom's atomics exist only inside a model: `tests/model.rs` models this book.
#[cfg(loom)]
fn main() {}

#[cfg(not(loom))]
fn main() {
    use core::num::NonZero;
    use std::thread;

    use atomix::AtomicBool;
    use atomix::ordering::{Acquire, Release};
    use quote_book::{Book, Side};

    /// The book's levels.
    const LEVELS: usize = 16;
    /// The threads that take quotes.
    const TAKERS: usize = 3;
    /// How many posts the feed tries.
    const POSTS: usize = 1_000_000;

    let book = Book::new(LEVELS);
    // Whether the feed is done, so each taker stops once it has swept the book after the last post.
    let done = AtomicBool::new(false);
    let (posted, taken) = thread::scope(|scope| {
        let takers: Vec<_> = (0..TAKERS)
            .map(|_| {
                scope.spawn(|| {
                    let mut taken = 0_u64;
                    loop {
                        // ORDERING: Acquire, so a taker that sees the feed done sees its last post.
                        let last_sweep = done.load(Acquire);
                        for quote in (0..LEVELS).filter_map(|level| book.take(level)) {
                            taken = taken.wrapping_add(u64::from(quote.quantity));
                        }
                        if last_sweep {
                            return taken;
                        }
                    }
                })
            })
            .collect();

        let levels = (0..LEVELS).cycle();
        let sides = [Side::Bid, Side::Ask].into_iter().cycle();
        let quantities = (1..=1_000).filter_map(NonZero::new).cycle();
        let mut posted = 0_u64;
        for ((level, side), quantity) in levels.zip(sides).zip(quantities).take(POSTS) {
            if book.post(level, side, quantity) {
                posted = posted.wrapping_add(u64::from(quantity.get()));
            }
        }
        // ORDERING: Release, publishing every post to the takers' last sweeps.
        done.store(true, Release);
        let taken = takers.into_iter().map(|taker| taker.join().expect("no taker panics"));
        (posted, taken.fold(0, u64::wrapping_add))
    });

    assert_eq!(taken, posted, "every quote posted was taken, once");
    println!("{TAKERS} takers took each quote the feed posted, {taken} in all");
}
