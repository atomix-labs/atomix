//! A book of quotes: a feed posts a quote to each price level, and takers race to take each whole.
//!
//! A level holds one quote at a time. A taker takes it by clearing its `live` bit, one `lock btr`
//! or `ldclr` that only one taker finds set; reads its quantity; then frees the level by clearing
//! the quantity, one `lock and` or `ldclr`. The feed posts only to a level whose quote is taken and
//! freed, so the quote a taker reads is the one it took. The quote is all in its word, so each
//! operation is `Relaxed`: nothing beside the word is published.

use core::num::NonZero;

use atomix::ordering::Relaxed;
use atomix::{Atom, Atomic};

// ANCHOR: quote
/// The side of the book a quote rests on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
pub(crate) enum Side {
    /// To buy.
    Bid,
    /// To sell.
    Ask,
}

/// A quote at a level: 32 bits of quantity, a bit of side, then whether a taker may take it, in a
/// `u64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
pub(crate) struct Quote {
    /// How much it offers, zero once a taker frees its level.
    pub(crate) quantity: u32,
    /// The side it rests on.
    pub(crate) side: Side,
    /// Whether a taker may take it.
    pub(crate) live: bool,
}
// ANCHOR_END: quote

/// A book of quotes, one level a price.
#[derive(Debug)]
pub(crate) struct Book {
    /// Each level's quote.
    levels: Box<[Atomic<Quote>]>,
}

impl Book {
    /// A book of `levels` empty levels.
    pub(crate) fn new(levels: usize) -> Self {
        let empty = Quote { quantity: 0, side: Side::Bid, live: false };
        Self { levels: (0..levels).map(|_| Atomic::new(empty)).collect() }
    }

    // ANCHOR: post
    /// Posts `quantity` on `side` at `level`, where the level's last quote is taken and freed, and
    /// returns whether it posted.
    ///
    /// # Panics
    /// Where `level` lies past the book.
    #[track_caller]
    pub(crate) fn post(&self, level: usize, side: Side, quantity: NonZero<u32>) -> bool {
        let quote = Quote { quantity: quantity.get(), side, live: true };
        // ORDERING: Relaxed: the quote is all in the word, which a taker's test-and-clear reads
        // whole.
        let posted = self.level(level).try_update(Relaxed, Relaxed, |last| {
            (!last.live && last.quantity == 0).then_some(quote)
        });
        posted.is_ok()
    }
    // ANCHOR_END: post

    // ANCHOR: take
    /// Takes the quote at `level`, where one is live, and frees the level for the next.
    ///
    /// # Panics
    /// Where `level` lies past the book.
    #[track_caller]
    pub(crate) fn take(&self, level: usize) -> Option<Quote> {
        let place = self.level(level);
        // ORDERING: Relaxed, as in `post`.
        if !place.fields().live.test_and_clear(Relaxed) {
            return None;
        }
        // ORDERING: Relaxed: the level is this taker's until its quantity is zero, so the word
        // still holds the quote whose bit this taker cleared.
        let taken = place.load(Relaxed);
        // ORDERING: Relaxed: it publishes nothing; the next post's exchange reads this clear, or
        // a later write, before it lands.
        place.fields().quantity.and(0, Relaxed);
        Some(taken)
    }
    // ANCHOR_END: take

    /// The quote at `level`.
    ///
    /// # Panics
    /// Where `level` lies past the book.
    #[track_caller]
    #[cfg_attr(
        not(test),
        expect(clippy::indexing_slicing, reason = "a level past the book is the caller's bug")
    )]
    fn level(&self, level: usize) -> &Atomic<Quote> {
        &self.levels[level]
    }
}
