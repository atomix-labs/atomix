//! What one operation costs on one thread: atomix's, against core's and its peers' in the same
//! orderings, and against the same bits written by hand over core's atomics.
//!
//! Each group answers one question. `integer`: whether a `u64` costs what core's does, through
//! each crate that holds one. `fieldless_enum`: what a load costs where it decodes with a check and
//! where it does not. `packed`: what a struct costs whole, held in one instruction or behind a
//! lock. `fields`: what one field costs, against its mask by hand and against a compare-exchange
//! loop on the whole word. `tagged_pointer`: what a pointer's tags cost, against `AtomicPtr` and
//! masks. `two_words`: what 16 bytes cost, against portable-atomic's `u128` and crossbeam's lock. A
//! group's rows run one generic closure over a type named for each crate, so each row times the
//! same harness around its operation. What contention costs is `contention`'s.
//!
//! ```text
//! cargo bench -p atomix-rs --features derive --bench operations --no-run
//! taskset -c 16 cargo bench -p atomix-rs --features derive --bench operations
//! ```

#[cfg(not(loom))]
use divan::main as run_benches;

/// Loom's atomics exist only inside a model, so a loom build has nothing to time.
#[cfg(loom)]
fn main() {}

#[cfg(not(loom))]
fn main() {
    run_benches();
}

/// A `u64` through each crate that holds one, in the same orderings: an `Acquire` load, a `Release`
/// store, and `AcqRel` for the rest, a compare-exchange failing with `Acquire`.
///
/// crossbeam's `AtomicCell` fixes these orderings itself, and takes none.
#[cfg(not(loom))]
mod integer {
    use core::hint::black_box;
    use core::sync::atomic::{AtomicU64 as CoreU64, Ordering};

    use atomic::Atomic as AtomicCrateAtomic;
    use atomig::Atomic as AtomigAtomic;
    use atomix::AtomicU64;
    use atomix::ordering::{AcqRel, Acquire, Release};
    use crossbeam_utils::atomic::AtomicCell;
    use divan::{Bencher, bench};
    use portable_atomic::AtomicU64 as PortableU64;

    /// A `u64` atomic of one crate, each operation in the orderings the module names.
    trait U64Atomic {
        /// An atomic holding `value`.
        fn new(value: u64) -> Self;
        /// Reads the value.
        fn load(&self) -> u64;
        /// Writes `value`.
        fn store(&self, value: u64);
        /// Writes `value`, and returns the one before.
        fn swap(&self, value: u64) -> u64;
        /// Adds `delta`, and returns the value before.
        fn fetch_add(&self, delta: u64) -> u64;
        /// Writes `new` where the value is `current`, and returns the value found.
        fn compare_exchange(&self, current: u64, new: u64) -> Result<u64, u64>;
    }

    /// Declares a wrapper for each atomic that takes core's `Ordering`,
    /// and implements [`U64Atomic`] for it.
    macro_rules! with_core_orderings {
        ($($(#[$doc:meta])* $wrapper:ident($atomic:ty);)+) => {$(
            $(#[$doc])*
            struct $wrapper($atomic);

            impl U64Atomic for $wrapper {
                fn new(value: u64) -> Self {
                    Self(<$atomic>::new(value))
                }

                fn load(&self) -> u64 {
                    self.0.load(Ordering::Acquire)
                }

                fn store(&self, value: u64) {
                    self.0.store(value, Ordering::Release);
                }

                fn swap(&self, value: u64) -> u64 {
                    self.0.swap(value, Ordering::AcqRel)
                }

                fn fetch_add(&self, delta: u64) -> u64 {
                    self.0.fetch_add(delta, Ordering::AcqRel)
                }

                fn compare_exchange(&self, current: u64, new: u64) -> Result<u64, u64> {
                    self.0.compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire)
                }
            }
        )+};
    }

    with_core_orderings! {
        /// core's `AtomicU64`.
        Core(CoreU64);
        /// portable-atomic's `AtomicU64`.
        PortableAtomic(PortableU64);
        /// The `atomic` crate's `Atomic<u64>`.
        AtomicCrate(AtomicCrateAtomic<u64>);
        /// atomig's `Atomic<u64>`.
        Atomig(AtomigAtomic<u64>);
    }

    /// atomix's `AtomicU64`.
    struct Atomix(AtomicU64);

    impl U64Atomic for Atomix {
        fn new(value: u64) -> Self {
            Self(AtomicU64::new(value))
        }

        fn load(&self) -> u64 {
            self.0.load(Acquire)
        }

        fn store(&self, value: u64) {
            self.0.store(value, Release);
        }

        fn swap(&self, value: u64) -> u64 {
            self.0.swap(value, AcqRel)
        }

        fn fetch_add(&self, delta: u64) -> u64 {
            self.0.fetch_add(delta, AcqRel)
        }

        fn compare_exchange(&self, current: u64, new: u64) -> Result<u64, u64> {
            self.0.compare_exchange(current, new, AcqRel, Acquire)
        }
    }

    /// crossbeam's `AtomicCell<u64>`, which goes through core's `AtomicU64`.
    struct Crossbeam(AtomicCell<u64>);

    impl U64Atomic for Crossbeam {
        fn new(value: u64) -> Self {
            Self(AtomicCell::new(value))
        }

        fn load(&self) -> u64 {
            self.0.load()
        }

        fn store(&self, value: u64) {
            self.0.store(value);
        }

        fn swap(&self, value: u64) -> u64 {
            self.0.swap(value)
        }

        fn fetch_add(&self, delta: u64) -> u64 {
            self.0.fetch_add(delta)
        }

        fn compare_exchange(&self, current: u64, new: u64) -> Result<u64, u64> {
            self.0.compare_exchange(current, new)
        }
    }

    /// A load.
    #[bench(types = [Core, Atomix, PortableAtomic, Crossbeam, AtomicCrate, Atomig])]
    fn load<A: U64Atomic>(bencher: Bencher<'_, '_>) {
        let atomic = A::new(7);
        bencher.bench_local(|| black_box(&atomic).load());
    }

    /// A store.
    #[bench(types = [Core, Atomix, PortableAtomic, Crossbeam, AtomicCrate, Atomig])]
    fn store<A: U64Atomic>(bencher: Bencher<'_, '_>) {
        let atomic = A::new(7);
        bencher.bench_local(|| black_box(&atomic).store(black_box(8)));
    }

    /// An exchange.
    #[bench(types = [Core, Atomix, PortableAtomic, Crossbeam, AtomicCrate, Atomig])]
    fn swap<A: U64Atomic>(bencher: Bencher<'_, '_>) {
        let atomic = A::new(7);
        bencher.bench_local(|| black_box(&atomic).swap(black_box(8)));
    }

    /// An add that returns the value before.
    #[bench(types = [Core, Atomix, PortableAtomic, Crossbeam, AtomicCrate, Atomig])]
    fn fetch_add<A: U64Atomic>(bencher: Bencher<'_, '_>) {
        let atomic = A::new(7);
        bencher.bench_local(|| black_box(&atomic).fetch_add(black_box(1)));
    }

    /// A compare-exchange that lands each time, its `current` the value the last one wrote.
    #[bench(types = [Core, Atomix, PortableAtomic, Crossbeam, AtomicCrate, Atomig])]
    fn compare_exchange<A: U64Atomic>(bencher: Bencher<'_, '_>) {
        let atomic = A::new(0);
        let mut current = 0_u64;
        bencher.bench_local(|| {
            let new = current.wrapping_add(1);
            let found = black_box(&atomic).compare_exchange(current, new);
            current = new;
            found
        });
        assert_eq!(atomic.load(), current, "each exchange landed");
    }
}

/// A fieldless enum's `Acquire` load.
///
/// atomix's decodes with no check, since its reprs are the enum's; atomig's compares the byte
/// against the variants and panics on any other; `atomic_enum` keeps a `usize` and matches it,
/// panicking on any other; and core's `AtomicU8`, decoded by hand,
/// takes every byte past the last variant as it.
#[cfg(not(loom))]
mod fieldless_enum {
    use core::hint::black_box;
    use core::sync::atomic::{AtomicU8 as CoreU8, Ordering};

    use atomig::{Atom as AtomigAtom, Atomic as AtomigAtomic};
    use atomix::ordering::Acquire;
    use atomix::{Atom, Atomic};
    use divan::{Bencher, bench};

    /// A task's state, one byte, as atomix and atomig hold it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom, AtomigAtom)]
    #[repr(u8)]
    enum State {
        /// Waiting for work.
        Idle,
        /// Queued to run.
        Queued,
        /// Running.
        Running,
        /// Done.
        Done,
    }

    /// The same states, which `#[atomic_enum]` writes an atomic for, and derives `Clone`, `Copy`
    /// and `Debug` on itself.
    mod by_atomic_enum {
        use atomic_enum::atomic_enum;

        /// A task's state, as `atomic_enum` holds it.
        #[atomic_enum]
        #[derive(PartialEq, Eq)]
        pub(super) enum State {
            /// Waiting for work.
            Idle,
            /// Queued to run.
            Queued,
            /// Running.
            Running,
            /// Done.
            Done,
        }
    }

    /// An atomic of a task's state in one crate, holding `Running`.
    trait StateAtomic {
        /// The state it holds.
        type State;
        /// An atomic holding `Running`.
        fn new() -> Self;
        /// Reads the state.
        fn load(&self) -> Self::State;
    }

    /// atomix's `Atomic<State>`, a `u8`.
    struct Atomix(Atomic<State>);

    impl StateAtomic for Atomix {
        type State = State;

        fn new() -> Self {
            Self(Atomic::new(State::Running))
        }

        fn load(&self) -> State {
            self.0.load(Acquire)
        }
    }

    /// atomig's `Atomic<State>`, a `u8`.
    struct Atomig(AtomigAtomic<State>);

    impl StateAtomic for Atomig {
        type State = State;

        fn new() -> Self {
            Self(AtomigAtomic::new(State::Running))
        }

        fn load(&self) -> State {
            self.0.load(Ordering::Acquire)
        }
    }

    /// `atomic_enum`'s atomic, a `usize`.
    struct AtomicEnum(by_atomic_enum::AtomicState);

    impl StateAtomic for AtomicEnum {
        type State = by_atomic_enum::State;

        fn new() -> Self {
            Self(by_atomic_enum::AtomicState::new(by_atomic_enum::State::Running))
        }

        fn load(&self) -> by_atomic_enum::State {
            self.0.load(Ordering::Acquire)
        }
    }

    /// core's `AtomicU8`, its byte decoded by hand.
    struct Core(CoreU8);

    impl StateAtomic for Core {
        type State = State;

        fn new() -> Self {
            Self(CoreU8::new(2))
        }

        fn load(&self) -> State {
            match self.0.load(Ordering::Acquire) {
                0 => State::Idle,
                1 => State::Queued,
                2 => State::Running,
                _ => State::Done,
            }
        }
    }

    /// A load, decoded.
    #[bench(types = [Core, Atomix, Atomig, AtomicEnum])]
    fn load<A: StateAtomic>(bencher: Bencher<'_, '_>) {
        let atomic = A::new();
        bencher.bench_local(|| black_box(&atomic).load());
    }
}

/// A resting quote of three fields, whole.
///
/// atomix packs it into a `u64` and decodes it with no check; core's `AtomicU64` holds the same
/// bits, shifted and masked by hand. crossbeam's `AtomicCell` goes through an atomic only where the
/// value is aligned as one, so it takes its lock for a quote of 8 bytes aligned to 4, and the
/// `atomic` crate its spinlock, as it does for any value aligned below its size. Each holds the
/// same quote, laid out in C's order with no padding, which the `atomic` crate requires.
#[cfg(not(loom))]
mod packed {
    use core::hint::black_box;
    use core::sync::atomic::{AtomicU64 as CoreU64, Ordering};

    use atomic::Atomic as AtomicCrateAtomic;
    use atomix::ordering::{AcqRel, Acquire, Release};
    use atomix::{Atom, Atomic};
    use bytemuck::NoUninit;
    use crossbeam_utils::atomic::AtomicCell;
    use divan::{Bencher, bench};

    /// The side of the book a quote rests on, 16 bits wide in memory, so the quote has no padding.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom, NoUninit)]
    #[repr(u16)]
    enum Side {
        /// A bid.
        Bid,
        /// An ask.
        Ask,
    }

    /// A resting quote: 32 bits of price, 16 of quantity and a bit of side in atomix's `u64`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom, NoUninit)]
    #[repr(C)]
    struct Quote {
        /// The price, in ticks.
        price: u32,
        /// The quantity resting.
        quantity: u16,
        /// The side it rests on.
        side: Side,
    }

    /// A bid.
    const BID: Quote = Quote { price: 10_050, quantity: 300, side: Side::Bid };
    /// An ask.
    const ASK: Quote = Quote { price: 10_051, quantity: 200, side: Side::Ask };

    /// An atomic quote of one crate: an `Acquire` load, a `Release` store, and an `AcqRel`
    /// compare-exchange, where the crate takes an ordering.
    trait QuoteAtomic {
        /// An atomic holding `quote`.
        fn new(quote: Quote) -> Self;
        /// Reads the quote.
        fn load(&self) -> Quote;
        /// Writes `quote`.
        fn store(&self, quote: Quote);
        /// Writes `new` where the quote is `current`, and says whether it did.
        fn compare_exchange(&self, current: Quote, new: Quote) -> bool;
    }

    /// atomix's `Atomic<Quote>`, a `u64`.
    struct Atomix(Atomic<Quote>);

    impl QuoteAtomic for Atomix {
        fn new(quote: Quote) -> Self {
            Self(Atomic::new(quote))
        }

        fn load(&self) -> Quote {
            self.0.load(Acquire)
        }

        fn store(&self, quote: Quote) {
            self.0.store(quote, Release);
        }

        fn compare_exchange(&self, current: Quote, new: Quote) -> bool {
            self.0.compare_exchange(current, new, AcqRel, Acquire).is_ok()
        }
    }

    /// core's `AtomicU64`, the quote's bits laid out by hand as atomix lays them.
    struct Core(CoreU64);

    impl Core {
        /// The quote's bits: the price, the quantity above it, then the side.
        fn encode(quote: Quote) -> u64 {
            let side = u64::from(quote.side == Side::Ask);
            side << 48 | u64::from(quote.quantity) << 32 | u64::from(quote.price)
        }

        /// The quote `bits` hold.
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "the low bits of each field, as code by hand takes them"
        )]
        const fn decode(bits: u64) -> Quote {
            let side = if bits >> 48 & 1 == 0 { Side::Bid } else { Side::Ask };
            Quote { price: bits as u32, quantity: (bits >> 32) as u16, side }
        }
    }

    impl QuoteAtomic for Core {
        fn new(quote: Quote) -> Self {
            Self(CoreU64::new(Self::encode(quote)))
        }

        fn load(&self) -> Quote {
            Self::decode(self.0.load(Ordering::Acquire))
        }

        fn store(&self, quote: Quote) {
            self.0.store(Self::encode(quote), Ordering::Release);
        }

        fn compare_exchange(&self, current: Quote, new: Quote) -> bool {
            let (current, new) = (Self::encode(current), Self::encode(new));
            self.0.compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire).is_ok()
        }
    }

    /// crossbeam's `AtomicCell<Quote>`, behind its lock.
    struct Crossbeam(AtomicCell<Quote>);

    impl QuoteAtomic for Crossbeam {
        fn new(quote: Quote) -> Self {
            Self(AtomicCell::new(quote))
        }

        fn load(&self) -> Quote {
            self.0.load()
        }

        fn store(&self, quote: Quote) {
            self.0.store(quote);
        }

        fn compare_exchange(&self, current: Quote, new: Quote) -> bool {
            self.0.compare_exchange(current, new).is_ok()
        }
    }

    /// The `atomic` crate's `Atomic<Quote>`, behind its spinlock.
    struct AtomicCrate(AtomicCrateAtomic<Quote>);

    impl QuoteAtomic for AtomicCrate {
        fn new(quote: Quote) -> Self {
            Self(AtomicCrateAtomic::new(quote))
        }

        fn load(&self) -> Quote {
            self.0.load(Ordering::Acquire)
        }

        fn store(&self, quote: Quote) {
            self.0.store(quote, Ordering::Release);
        }

        fn compare_exchange(&self, current: Quote, new: Quote) -> bool {
            self.0.compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire).is_ok()
        }
    }

    /// A load, decoded.
    #[bench(types = [Core, Atomix, Crossbeam, AtomicCrate])]
    fn load<A: QuoteAtomic>(bencher: Bencher<'_, '_>) {
        let atomic = A::new(BID);
        bencher.bench_local(|| black_box(&atomic).load());
    }

    /// A store, encoded.
    #[bench(types = [Core, Atomix, Crossbeam, AtomicCrate])]
    fn store<A: QuoteAtomic>(bencher: Bencher<'_, '_>) {
        let atomic = A::new(BID);
        bencher.bench_local(|| black_box(&atomic).store(black_box(ASK)));
    }

    /// A compare-exchange that lands each time, between the bid and the ask.
    #[bench(types = [Core, Atomix, Crossbeam, AtomicCrate])]
    fn compare_exchange<A: QuoteAtomic>(bencher: Bencher<'_, '_>) {
        let atomic = A::new(BID);
        let mut quotes = (BID, ASK);
        bencher.bench_local(|| {
            let landed = black_box(&atomic).compare_exchange(quotes.0, quotes.1);
            quotes = (quotes.1, quotes.0);
            landed
        });
        assert_eq!(atomic.load(), quotes.0, "each exchange landed");
    }
}

/// One field of a packed quote.
///
/// atomix changes it with one instruction on the whole word; core's `AtomicU64` holds the same
/// bits, changed with the same instruction by hand; and a loop compare-exchanges the whole word, as
/// a crate that holds the quote only whole must. Each row is an `#[inline(never)]` function of one
/// signature, called from one closure, since two inlined closures differ by where `black_box`
/// spills alone; where two rows' functions are the same code, LLVM merges them,
/// and both rows call one.
#[cfg(not(loom))]
mod fields {
    use core::hint::black_box;
    use core::sync::atomic::{AtomicU64 as CoreU64, Ordering};

    use atomix::ordering::{AcqRel, Acquire, Release};
    use atomix::{Atom, Atomic, RangedU32};
    use divan::{Bencher, bench};

    /// A resting quote: a byte of flags, whether it may fill at bit 8, a price of 23 bits, then
    /// the quantity in the top 32 bits, where an add's carry leaves the word.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Quote {
        /// The quote's flags.
        flags: u8,
        /// Whether the quote may fill.
        live: bool,
        /// The price, in ticks.
        price: RangedU32<0, 0x7F_FFFF>,
        /// The quantity resting.
        quantity: u32,
    }

    /// A quote at rest, not live.
    const RESTING: Quote =
        Quote { flags: 0, live: false, price: RangedU32::new_saturating(10_050), quantity: 300 };
    /// `live`'s bit.
    const LIVE: u64 = 1 << 8;
    /// `quantity`'s bits.
    const QUANTITY: u64 = 0xFFFF_FFFF_0000_0000;
    /// `quantity`'s lowest bit.
    const QUANTITY_SHIFT: u32 = 32;
    /// The flag of a quote that only rests on the book.
    const POST_ONLY: u8 = 0b100;

    /// A quote's field operations in one form: `live`'s set and test-and-clear, an or of the
    /// flags, and the quantity's load, add and update.
    trait QuoteOperations {
        /// A resting quote.
        fn new() -> Self;
        /// Marks the quote live, with `Release`.
        fn set_live(&self);
        /// Clears `live`, with `AcqRel`, and says whether it was set.
        fn take_live(&self) -> bool;
        /// Adds `flags` to the flags, with `Release`.
        fn add_flags(&self, flags: u8);
        /// Reads the quantity, with `Acquire`.
        fn quantity(&self) -> u32;
        /// Adds `delta` to the quantity, with `AcqRel`, and returns the quantity before.
        fn add_quantity(&self, delta: u32) -> u32;
        /// Triples the quantity in a compare-exchange loop, with `AcqRel` and `Acquire`, and
        /// returns the quantity before.
        fn triple_quantity(&self) -> u32;
    }

    /// atomix's `Atomic<Quote>`, through its fields.
    struct Atomix(Atomic<Quote>);

    impl QuoteOperations for Atomix {
        fn new() -> Self {
            Self(Atomic::new(RESTING))
        }

        #[inline(never)]
        fn set_live(&self) {
            self.0.fields().live.set(Release);
        }

        #[inline(never)]
        fn take_live(&self) -> bool {
            self.0.fields().live.test_and_clear(AcqRel)
        }

        #[inline(never)]
        fn add_flags(&self, flags: u8) {
            self.0.fields().flags.or(flags, Release);
        }

        #[inline(never)]
        fn quantity(&self) -> u32 {
            self.0.fields().quantity.load(Acquire)
        }

        #[inline(never)]
        fn add_quantity(&self, delta: u32) -> u32 {
            self.0.fields().quantity.fetch_add(delta, AcqRel).quantity
        }

        #[inline(never)]
        fn triple_quantity(&self) -> u32 {
            let quantity = self.0.fields().quantity;
            quantity.update(AcqRel, Acquire, |quantity| quantity.wrapping_mul(3)).quantity
        }
    }

    /// core's `AtomicU64`, each field's bits changed by hand with the instruction atomix uses.
    struct ByHand(CoreU64);

    impl QuoteOperations for ByHand {
        fn new() -> Self {
            Self(CoreU64::new(RESTING.to_repr()))
        }

        #[inline(never)]
        fn set_live(&self) {
            self.0.fetch_or(LIVE, Ordering::Release);
        }

        #[inline(never)]
        fn take_live(&self) -> bool {
            self.0.fetch_and(!LIVE, Ordering::AcqRel) & LIVE != 0
        }

        #[inline(never)]
        fn add_flags(&self, flags: u8) {
            self.0.fetch_or(u64::from(flags), Ordering::Release);
        }

        #[inline(never)]
        fn quantity(&self) -> u32 {
            quantity(self.0.load(Ordering::Acquire))
        }

        #[inline(never)]
        fn add_quantity(&self, delta: u32) -> u32 {
            quantity(self.0.fetch_add(u64::from(delta) << QUANTITY_SHIFT, Ordering::AcqRel))
        }

        #[inline(never)]
        fn triple_quantity(&self) -> u32 {
            quantity(self.0.update(Ordering::AcqRel, Ordering::Acquire, |bits| {
                with_quantity(bits, quantity(bits).wrapping_mul(3))
            }))
        }
    }

    /// core's `AtomicU64`, each field changed by a compare-exchange loop on the whole word.
    struct Loop(CoreU64);

    impl QuoteOperations for Loop {
        fn new() -> Self {
            Self(CoreU64::new(RESTING.to_repr()))
        }

        #[inline(never)]
        fn set_live(&self) {
            self.0.update(Ordering::Release, Ordering::Relaxed, |bits| bits | LIVE);
        }

        #[inline(never)]
        fn take_live(&self) -> bool {
            self.0.update(Ordering::AcqRel, Ordering::Acquire, |bits| bits & !LIVE) & LIVE != 0
        }

        #[inline(never)]
        fn add_flags(&self, flags: u8) {
            self.0.update(Ordering::Release, Ordering::Relaxed, |bits| bits | u64::from(flags));
        }

        #[inline(never)]
        fn quantity(&self) -> u32 {
            quantity(self.0.load(Ordering::Acquire))
        }

        #[inline(never)]
        fn add_quantity(&self, delta: u32) -> u32 {
            quantity(self.0.update(Ordering::AcqRel, Ordering::Acquire, |bits| {
                with_quantity(bits, quantity(bits).wrapping_add(delta))
            }))
        }

        #[inline(never)]
        fn triple_quantity(&self) -> u32 {
            quantity(self.0.update(Ordering::AcqRel, Ordering::Acquire, |bits| {
                with_quantity(bits, quantity(bits).wrapping_mul(3))
            }))
        }
    }

    /// The quantity `bits` hold.
    #[expect(clippy::as_conversions, reason = "the top 32 bits, as code by hand takes them")]
    const fn quantity(bits: u64) -> u32 {
        (bits >> QUANTITY_SHIFT) as u32
    }

    /// `bits` with the quantity `quantity`.
    fn with_quantity(bits: u64, quantity: u32) -> u64 {
        bits & !QUANTITY | u64::from(quantity) << QUANTITY_SHIFT
    }

    /// `live` set.
    #[bench(types = [ByHand, Atomix, Loop])]
    fn set<A: QuoteOperations>(bencher: Bencher<'_, '_>) {
        let atomic = A::new();
        bencher.bench_local(|| black_box(&atomic).set_live());
    }

    /// `live` cleared, and whether it was set.
    #[bench(types = [ByHand, Atomix, Loop])]
    fn test_and_clear<A: QuoteOperations>(bencher: Bencher<'_, '_>) {
        let atomic = A::new();
        bencher.bench_local(|| black_box(&atomic).take_live());
    }

    /// A flag added.
    #[bench(types = [ByHand, Atomix, Loop])]
    fn or<A: QuoteOperations>(bencher: Bencher<'_, '_>) {
        let atomic = A::new();
        bencher.bench_local(|| black_box(&atomic).add_flags(black_box(POST_ONLY)));
    }

    /// The quantity read.
    #[bench(types = [ByHand, Atomix, Loop])]
    fn load<A: QuoteOperations>(bencher: Bencher<'_, '_>) {
        let atomic = A::new();
        bencher.bench_local(|| black_box(&atomic).quantity());
    }

    /// One added to the quantity, at the top of the word.
    #[bench(types = [ByHand, Atomix, Loop])]
    fn fetch_add<A: QuoteOperations>(bencher: Bencher<'_, '_>) {
        let atomic = A::new();
        bencher.bench_local(|| black_box(&atomic).add_quantity(black_box(1)));
    }

    /// The quantity tripled, which no instruction does, so every form loops.
    #[bench(types = [ByHand, Atomix, Loop])]
    fn update<A: QuoteOperations>(bencher: Bencher<'_, '_>) {
        let atomic = A::new();
        bencher.bench_local(|| black_box(&atomic).triple_quantity());
    }
}

/// A Treiber stack's head: a node's pointer, with a version of two bits and whether the stack is
/// closed in the low bits the node's alignment leaves clear.
///
/// atomix's store tests the pointer against those bits once, and branches to a cold refusal where
/// one is set; `AtomicPtr` with the tags or-ed in and masked off by hand tests nothing. Each row is
/// an `#[inline(never)]` function, as `fields`' are.
#[cfg(not(loom))]
mod tagged_pointer {
    use core::hint::black_box;
    use core::ptr::{self, NonNull};
    use core::sync::atomic::{AtomicPtr, Ordering};

    use atomix::ordering::{AcqRel, Acquire, Release};
    use atomix::{Atom, Atomic, RangedU8};
    use divan::{Bencher, bench};

    /// A node of a stack, aligned to 8, so a pointer to one leaves three low bits clear.
    #[repr(align(8))]
    struct Node {
        /// What the node holds, which gives each node an address of its own.
        #[expect(dead_code, reason = "a node is only pointed to")]
        value: u64,
    }

    /// The head: the top node, then in its pointer's clear bits a version
    /// and whether the stack is closed.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Head {
        /// The top node, or none.
        top: Option<NonNull<Node>>,
        /// The head's changes, counted in two bits.
        version: RangedU8<0, 3>,
        /// Whether the stack takes no more pushes.
        closed: bool,
    }

    /// The version's bits, by hand.
    const VERSION: usize = 0b11;
    /// `closed`'s bit, by hand.
    const CLOSED: usize = 0b100;

    /// A head in one form, each operation on an open head of version 3.
    trait HeadAtomic {
        /// An empty head.
        fn new() -> Self;
        /// Writes `top`, with `Release`.
        fn store(&self, top: NonNull<Node>);
        /// Reads the top, the version and whether the stack is closed, with `Acquire`.
        fn load(&self) -> (Option<NonNull<Node>>, usize, bool);
        /// Writes `new` where the top is `current`, with `AcqRel` or `Acquire`,
        /// and says whether it did.
        fn compare_exchange(&self, current: NonNull<Node>, new: NonNull<Node>) -> bool;
        /// Closes the stack, with `AcqRel`, and says whether it was closed.
        fn close(&self) -> bool;
    }

    /// atomix's `Atomic<Head>`, a pointer word.
    struct Atomix(Atomic<Head>);

    impl Atomix {
        /// The open head of version 3 whose top is `top`.
        const fn head(top: NonNull<Node>) -> Head {
            Head { top: Some(top), version: RangedU8::MAX, closed: false }
        }
    }

    impl HeadAtomic for Atomix {
        fn new() -> Self {
            Self(Atomic::new(Head { top: None, version: RangedU8::MIN, closed: false }))
        }

        #[inline(never)]
        fn store(&self, top: NonNull<Node>) {
            self.0.store(Self::head(top), Release);
        }

        #[inline(never)]
        fn load(&self) -> (Option<NonNull<Node>>, usize, bool) {
            let head = self.0.load(Acquire);
            (head.top, usize::from(head.version.get()), head.closed)
        }

        #[inline(never)]
        fn compare_exchange(&self, current: NonNull<Node>, new: NonNull<Node>) -> bool {
            let (current, new) = (Self::head(current), Self::head(new));
            self.0.compare_exchange(current, new, AcqRel, Acquire).is_ok()
        }

        #[inline(never)]
        fn close(&self) -> bool {
            self.0.fields().closed.test_and_set(AcqRel)
        }
    }

    /// core's `AtomicPtr`, the tags or-ed in and masked off by hand.
    struct ByHand(AtomicPtr<Node>);

    impl ByHand {
        /// `top`, of version 3 and open.
        fn tagged(top: NonNull<Node>) -> *mut Node {
            top.as_ptr().map_addr(|address| address | VERSION)
        }
    }

    impl HeadAtomic for ByHand {
        fn new() -> Self {
            Self(AtomicPtr::new(ptr::null_mut()))
        }

        #[inline(never)]
        fn store(&self, top: NonNull<Node>) {
            self.0.store(Self::tagged(top), Ordering::Release);
        }

        #[inline(never)]
        fn load(&self) -> (Option<NonNull<Node>>, usize, bool) {
            let read = self.0.load(Ordering::Acquire);
            let top = NonNull::new(read.map_addr(|address| address & !(VERSION | CLOSED)));
            (top, read.addr() & VERSION, read.addr() & CLOSED != 0)
        }

        #[inline(never)]
        fn compare_exchange(&self, current: NonNull<Node>, new: NonNull<Node>) -> bool {
            let (current, new) = (Self::tagged(current), Self::tagged(new));
            self.0.compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire).is_ok()
        }

        #[inline(never)]
        fn close(&self) -> bool {
            self.0.fetch_or(CLOSED, Ordering::AcqRel).addr() & CLOSED != 0
        }
    }

    /// A store of the head.
    #[bench(types = [ByHand, Atomix])]
    fn store<A: HeadAtomic>(bencher: Bencher<'_, '_>) {
        let node = Node { value: 7 };
        let atomic = A::new();
        bencher.bench_local(|| black_box(&atomic).store(black_box(NonNull::from_ref(&node))));
    }

    /// A load, its tags taken off the pointer.
    #[bench(types = [ByHand, Atomix])]
    fn load<A: HeadAtomic>(bencher: Bencher<'_, '_>) {
        let node = Node { value: 7 };
        let atomic = A::new();
        atomic.store(NonNull::from_ref(&node));
        bencher.bench_local(|| black_box(&atomic).load());
    }

    /// A compare-exchange that lands each time, between two nodes.
    #[bench(types = [ByHand, Atomix])]
    fn compare_exchange<A: HeadAtomic>(bencher: Bencher<'_, '_>) {
        let nodes = [Node { value: 7 }, Node { value: 8 }];
        let [first, second] = nodes.each_ref().map(NonNull::from_ref);
        let atomic = A::new();
        atomic.store(first);
        let mut tops = (first, second);
        bencher.bench_local(|| {
            let landed = black_box(&atomic).compare_exchange(tops.0, tops.1);
            tops = (tops.1, tops.0);
            landed
        });
        assert_eq!(atomic.load().0, Some(tops.0), "each exchange landed");
    }

    /// `closed` set, and whether it was: `lock bts`, or `ldsetal`, on the pointer.
    #[bench(types = [ByHand, Atomix])]
    fn test_and_set<A: HeadAtomic>(bencher: Bencher<'_, '_>) {
        let atomic = A::new();
        bencher.bench_local(|| black_box(&atomic).close());
    }
}

/// A Treiber stack's head in two words, its top beside a `u64` version, which no alignment leaves
/// room for in the pointer.
///
/// atomix holds it as a double word, or as an `AtomicU128` of the top's address and the version;
/// portable-atomic as its `AtomicU128`; and crossbeam's `AtomicCell` behind its lock, as it holds
/// every value of 16 bytes. A read and a write are the forms each crate runs on every floor:
/// atomix's `load_rmw` and `store_rmw`, a compare-exchange and a loop of them; portable-atomic's
/// `load` and `store`, which it dispatches at run time where the floor lacks LSE2 or AVX. atomix's
/// `load` and `store`, one instruction each, need either.
#[cfg(not(loom))]
#[cfg(any(target_arch = "aarch64", target_arch = "arm64ec", target_feature = "cmpxchg16b"))]
mod two_words {
    use core::hint::black_box;
    use core::ptr::{self, NonNull};
    use core::sync::atomic::Ordering;

    use atomix::ordering::{AcqRel, Acquire, Release};
    use atomix::{Atom, Atomic, AtomicU128};
    use crossbeam_utils::atomic::AtomicCell;
    use divan::{Bencher, bench};
    use portable_atomic::AtomicU128 as PortableU128;

    /// A node of a stack.
    struct Node {
        /// What the node holds, which gives each node an address of its own.
        #[expect(dead_code, reason = "a node is only pointed to")]
        value: u64,
    }

    /// The head: the top node, beside a version that counts the head's changes.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Atom)]
    struct Head {
        /// The top node, or none.
        top: Option<NonNull<Node>>,
        /// The head's changes.
        version: u64,
    }

    /// The head's 128 bits: the top's address, exposed, beside the version.
    #[expect(clippy::as_conversions, reason = "a 64-bit address, widened")]
    fn to_bits(head: Head) -> u128 {
        let address = head.top.map_or(0, |top| top.as_ptr().expose_provenance());
        u128::from(head.version) << 64 | address as u128
    }

    /// The head `bits` hold, its top taking back the provenance `to_bits` exposed.
    #[expect(
        clippy::as_conversions,
        reason = "the low 64 bits, the address, and the high, the version"
    )]
    const fn from_bits(bits: u128) -> Head {
        let top = NonNull::new(ptr::with_exposed_provenance_mut(bits as usize));
        Head { top, version: (bits >> 64) as u64 }
    }

    /// A head of one crate: an `Acquire` read, a `Release` write, and an `AcqRel` compare-exchange.
    trait HeadAtomic {
        /// An atomic holding `head`.
        fn new(head: Head) -> Self;
        /// Reads the head.
        fn read(&self) -> Head;
        /// Writes `head`.
        fn write(&self, head: Head);
        /// Writes `new` where the head is `current`, and says whether it did.
        fn compare_exchange(&self, current: Head, new: Head) -> bool;
    }

    /// atomix's `Atomic<Head>`, a double word.
    struct Atomix(Atomic<Head>);

    impl HeadAtomic for Atomix {
        fn new(head: Head) -> Self {
            Self(Atomic::new(head))
        }

        fn read(&self) -> Head {
            self.0.load_rmw(Acquire)
        }

        fn write(&self, head: Head) {
            self.0.store_rmw(head, Release);
        }

        fn compare_exchange(&self, current: Head, new: Head) -> bool {
            self.0.compare_exchange(current, new, AcqRel, Acquire).is_ok()
        }
    }

    /// atomix's `AtomicU128`, the head's bits.
    struct AtomixU128(AtomicU128);

    impl HeadAtomic for AtomixU128 {
        fn new(head: Head) -> Self {
            Self(AtomicU128::new(to_bits(head)))
        }

        fn read(&self) -> Head {
            from_bits(self.0.load_rmw(Acquire))
        }

        fn write(&self, head: Head) {
            self.0.store_rmw(to_bits(head), Release);
        }

        fn compare_exchange(&self, current: Head, new: Head) -> bool {
            let (current, new) = (to_bits(current), to_bits(new));
            self.0.compare_exchange(current, new, AcqRel, Acquire).is_ok()
        }
    }

    /// portable-atomic's `AtomicU128`, the head's bits.
    struct PortableAtomic(PortableU128);

    impl HeadAtomic for PortableAtomic {
        fn new(head: Head) -> Self {
            Self(PortableU128::new(to_bits(head)))
        }

        fn read(&self) -> Head {
            from_bits(self.0.load(Ordering::Acquire))
        }

        fn write(&self, head: Head) {
            self.0.store(to_bits(head), Ordering::Release);
        }

        fn compare_exchange(&self, current: Head, new: Head) -> bool {
            let (current, new) = (to_bits(current), to_bits(new));
            self.0.compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire).is_ok()
        }
    }

    /// crossbeam's `AtomicCell<Head>`, behind its lock.
    struct Crossbeam(AtomicCell<Head>);

    impl HeadAtomic for Crossbeam {
        fn new(head: Head) -> Self {
            Self(AtomicCell::new(head))
        }

        fn read(&self) -> Head {
            self.0.load()
        }

        fn write(&self, head: Head) {
            self.0.store(head);
        }

        fn compare_exchange(&self, current: Head, new: Head) -> bool {
            self.0.compare_exchange(current, new).is_ok()
        }
    }

    /// The head of version `version`, whose top is `node`.
    const fn head(node: &Node, version: u64) -> Head {
        Head { top: Some(NonNull::from_ref(node)), version }
    }

    /// A compare-exchange of both words that lands each time: `caspal`, or `lock cmpxchg16b`.
    #[bench(types = [Atomix, AtomixU128, PortableAtomic, Crossbeam])]
    fn compare_exchange<A: HeadAtomic>(bencher: Bencher<'_, '_>) {
        let node = Node { value: 7 };
        let mut current = head(&node, 0);
        let atomic = A::new(current);
        bencher.bench_local(|| {
            let new = Head { version: current.version.wrapping_add(1), ..black_box(current) };
            let landed = black_box(&atomic).compare_exchange(current, new);
            current = new;
            landed
        });
        assert_eq!(atomic.read(), current, "each exchange landed");
    }

    /// A read of both words.
    #[bench(types = [Atomix, AtomixU128, PortableAtomic, Crossbeam])]
    fn read<A: HeadAtomic>(bencher: Bencher<'_, '_>) {
        let node = Node { value: 7 };
        let atomic = A::new(head(&node, 1));
        bencher.bench_local(|| black_box(&atomic).read());
    }

    /// A write of both words.
    #[bench(types = [Atomix, AtomixU128, PortableAtomic, Crossbeam])]
    fn write<A: HeadAtomic>(bencher: Bencher<'_, '_>) {
        let node = Node { value: 7 };
        let atomic = A::new(head(&node, 1));
        bencher.bench_local(|| black_box(&atomic).write(black_box(head(&node, 2))));
    }

    /// atomix's read with one 16-byte load: `ldp`, or `vmovdqa`.
    #[cfg(any(target_feature = "lse2", target_feature = "avx"))]
    #[bench]
    fn load(bencher: Bencher<'_, '_>) {
        let node = Node { value: 7 };
        let atomic = Atomic::new(head(&node, 1));
        bencher.bench_local(|| black_box(&atomic).load(Acquire));
    }

    /// atomix's write with one 16-byte store: `stp`, or `vmovdqa`.
    #[cfg(any(target_feature = "lse2", target_feature = "avx"))]
    #[bench]
    fn store(bencher: Bencher<'_, '_>) {
        let node = Node { value: 7 };
        let atomic = Atomic::new(head(&node, 1));
        bencher.bench_local(|| black_box(&atomic).store(black_box(head(&node, 2)), Release));
    }
}
