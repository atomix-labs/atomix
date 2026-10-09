//! `#[derive(Atom)]` on a newtype over a reference or a function pointer: it takes the value's
//! sealed repr, so its `Option` is free, its orderings keep a reference's floor, and no bits make
//! one.

#![feature(const_trait_impl)]
#![cfg(feature = "derive")]
#![cfg(not(loom))]

#[cfg(test)]
mod tests {
    use core::mem::size_of;

    use atomix::ordering::{Acquire, Relaxed, Release};
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
}
