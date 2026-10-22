//! `#[derive(Atom)]` over a reference or a function pointer: a newtype takes the value's sealed
//! repr, so its `Option` is free, its orderings keep a reference's floor, and no bits make one; a
//! pointer word keeps its tags in the low bits the pointee's alignment leaves clear, as it keeps a
//! `NonNull`'s, and takes the floor too, its tag fields every ordering; and two words hold a
//! reference beside an integer word or another reference, and a function pointer beside an id.

#![feature(const_trait_impl)]
#![cfg(feature = "derive")]
#![cfg(not(loom))]

#[cfg(test)]
mod tests {
    use core::mem::size_of;
    use core::ptr;

    use atomix::ordering::{AcqRel, Acquire, Relaxed, Release};
    use atomix::{Atom, Atomic, Primitive};

    /// A configuration a service reads on every order.
    #[derive(Debug, PartialEq)]
    struct Config {
        /// How many price levels a quote shows.
        depth: u32,
    }

    /// The configuration in force, a newtype over a reference.
    #[derive(Clone, Copy, Debug, PartialEq, Atom)]
    struct Current<'a>(&'a Config);

    /// A hook that prices an order, a newtype over a function pointer.
    #[derive(Clone, Copy, Atom)]
    struct Pricer(fn(&str) -> u64);

    /// The configuration before any is loaded.
    static DEFAULT: Config = Config { depth: 10 };

    /// The configuration in force.
    static CURRENT: Atomic<Option<Current<'static>>> = Atomic::new(Some(Current(&DEFAULT)));

    /// The configuration that replaces the default.
    static NEXT: Config = Config { depth: 20 };

    /// A price of every symbol: its length.
    fn by_length(symbol: &str) -> u64 {
        symbol.len().try_into().expect("a symbol fits a u64")
    }

    #[test]
    fn a_newtype_over_a_reference_or_a_function_pointer_takes_its_sealed_repr() {
        assert_eq!(size_of::<Atomic<Option<Current<'_>>>>(), size_of::<usize>(), "one word");
        assert_eq!(CURRENT.load(Acquire), Some(Current(&DEFAULT)), "built in a constant");
        let next = Config { depth: 20 };
        let current = Atomic::new(Current(&next));
        current.store(Current(&DEFAULT), Release);
        assert_eq!(current.load(Acquire).0.depth, 10, "read through the reference");
        let forged = <Current<'_> as Atom>::from_repr(Primitive::from_bits(0x1000));
        assert_eq!(forged, None, "no bits make one");

        let pricer = Atomic::new(Pricer(by_length));
        assert_eq!((pricer.load(Relaxed).0)("AAPL"), 4, "a function pointer, any ordering");
    }

    /// The configuration in force, or none, and whether it is stale: a pointer word over a
    /// reference, its tag in the low bits a `Config`'s alignment, 4, leaves clear.
    #[derive(Clone, Copy, Debug, PartialEq, Atom)]
    struct Published {
        /// The configuration.
        config: Option<&'static Config>,
        /// Whether a newer one is on its way.
        stale: bool,
    }

    #[test]
    fn a_pointer_word_keeps_its_tags_beside_a_reference() {
        let published = Atomic::new(Published { config: Some(&DEFAULT), stale: false });
        assert_eq!(size_of_val(&published), size_of::<usize>(), "in one word");
        // A tag field takes every ordering where it hands out no more than itself: it publishes
        // nothing of the reference, and reads none.
        assert!(!published.fields().stale.test_and_set(Relaxed), "marked stale once");
        assert!(published.fields().stale.load(Relaxed), "read alone");
        published.fields().stale.clear(Relaxed);
        published.fields().stale.store(true, Relaxed);
        let read = published.load(Acquire);
        assert_eq!(read, Published { config: Some(&DEFAULT), stale: true }, "beside the config");
        // Where it returns the word, its read acquires the reference, and its write, which puts
        // the reference back, needs no `Release`.
        let before = published.fields().stale.update(Acquire, Acquire, |stale| !stale);
        assert_eq!(before, read, "the word before, its reference among it");
        published.fields().stale.set(Relaxed);
        let config = published.fields().config.load(Acquire).expect("a config is published");
        assert!(ptr::eq(config, &raw const DEFAULT), "the reference, its tag cleared");
        let replaced =
            published.update(AcqRel, Acquire, |_| Published { config: Some(&NEXT), stale: false });
        assert!(replaced.stale, "the stale config replaced");
        assert_eq!(
            published.load(Acquire).config.map(|config| config.depth),
            Some(20),
            "by the next"
        );
        published.store(Published { config: None, stale: true }, Release);
        assert_eq!(published.load(Acquire).config, None, "and none, its tag set beside null");
        published.store_rmw(Published { config: Some(&DEFAULT), stale: false }, Release);
        assert_eq!(
            published.load(Acquire).config.map(|config| config.depth),
            Some(10),
            "stored by an exchange"
        );
    }

    /// A node of a list, which a reference with a lifetime points to.
    #[derive(Debug)]
    struct Node {
        /// What it holds.
        value: u64,
    }

    /// The head of a list of nodes that live as long as `'a`, and whether it is closed: a pointer
    /// word with a lifetime, laid out per instance in the word it states.
    #[derive(Clone, Copy, Debug, Atom)]
    #[atom(repr = u64)]
    struct Head<'a> {
        /// The first node, or none.
        top: Option<&'a Node>,
        /// Whether pushes are refused.
        closed: bool,
    }

    #[test]
    fn a_pointer_word_over_a_reference_with_a_lifetime_lives_as_long_as_it() {
        let node = Node { value: 7 };
        let head = Atomic::new(Head { top: None, closed: false });
        head.store(Head { top: Some(&node), closed: false }, Release);
        head.fields().closed.set(Relaxed);
        let read = head.load(Acquire);
        assert_eq!(read.top.map(|top| top.value), Some(7), "the node, read through");
        assert!(read.closed, "beside the mark");
    }

    /// A quote's resting order.
    #[cfg(wide)]
    #[derive(Debug)]
    struct Order {
        /// Its price.
        price: u64,
    }

    /// The best bid and ask, both resting orders, and whether the book is live: two references in
    /// two words, the tag in the first's low bits.
    #[cfg(wide)]
    #[derive(Clone, Copy, Debug, Atom)]
    struct Best {
        /// The best bid.
        bid: &'static Order,
        /// The best ask.
        ask: &'static Order,
        /// Whether the book trades.
        live: bool,
    }

    /// The configuration in force and how often it changed: a reference beside an integer word.
    #[cfg(wide)]
    #[derive(Clone, Copy, Debug, Atom)]
    #[atom(repr = u128)]
    struct Versioned {
        /// The configuration.
        config: &'static Config,
        /// How many times it was replaced.
        version: u64,
    }

    /// A handler of one feed, by its id: a function pointer beside an integer word, since no
    /// alignment of code leaves its low bits clear.
    #[cfg(wide)]
    #[derive(Clone, Copy, Atom)]
    #[atom(repr = u128)]
    struct Handler {
        /// What handles the feed's messages.
        handle: fn(u64) -> u64,
        /// The feed's id.
        feed: u32,
    }

    /// Twice `value`: a handler.
    #[cfg(wide)]
    fn twice(value: u64) -> u64 {
        value.wrapping_mul(2)
    }

    #[cfg(wide)]
    #[test]
    fn two_words_hold_a_reference_beside_an_integer_another_reference_or_a_function_pointer() {
        static BID: Order = Order { price: 100 };
        static ASK: Order = Order { price: 101 };
        let best = Atomic::new(Best { bid: &BID, ask: &ASK, live: false });
        // Two words have no bitwise instruction, so a tag changes in a compare-exchange loop, whose
        // write puts both references back: its read acquires, and its write needs no `Release`.
        best.fields().live.update(Acquire, Acquire, |_| true);
        let read = best.load_rmw(Acquire);
        assert_eq!((read.bid.price, read.ask.price, read.live), (100, 101, true), "both, live");

        let versioned = Atomic::new(Versioned { config: &DEFAULT, version: 0 });
        versioned.update(AcqRel, Acquire, |seen| Versioned {
            config: &NEXT,
            version: seen.version.wrapping_add(1),
        });
        let read = versioned.load_rmw(Acquire);
        assert_eq!((read.config.depth, read.version), (20, 1), "the config, counted");
        assert_eq!(
            versioned.fields().version.load_rmw(Relaxed),
            1,
            "the count alone, any ordering"
        );

        let handler = Atomic::new(Handler { handle: twice, feed: 7 });
        let read = handler.load_rmw(Relaxed);
        assert_eq!(((read.handle)(21), read.feed), (42, 7), "a function, any ordering");
    }
}
