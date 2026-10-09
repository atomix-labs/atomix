//! A shared reference and a function pointer are each held whole, in one word, or a wide
//! reference's two: each obeys the `Atom` laws, keeps its provenance, as Miri checks under
//! `-Zmiri-strict-provenance` by reading through each reference decoded and calling each function
//! pointer, and decodes from no bits; an `Option` of one is free; a `static` holds one built in a
//! constant; a reference stored with `Release` by one thread is read whole through its `Acquire`
//! load by another; and a thin one's place is a raw pointer's, which C may write.

#![feature(const_trait_impl)]
// Loom's cells exist only inside a model, and a `const` `new` not at all.
#![cfg(not(loom))]

#[cfg(test)]
mod testing;

#[cfg(test)]
mod tests {
    use core::ffi::c_int;
    use core::fmt::Debug;
    use core::hash::{BuildHasher, BuildHasherDefault};
    use core::mem::size_of;
    use core::ptr::{self, NonNull};
    use std::hash::DefaultHasher;
    use std::thread;

    use atomix_core::ordering::{AcqRel, Acquire, CoreRmwOrdering, Relaxed, Release, SeqCst};
    use atomix_core::validity::{Total, TotalZeroNiche, ZeroNiche, ZeroValid};
    use atomix_core::{Atom, Atomic, BitTest, Load, MaskBitwise, Primitive, ReprOf};

    use crate::testing::law::{assert_holds, decodes_as_promised, keeps_provenance, round_trips};

    /// What a trading service reads on every order: replaced whole, never changed in place.
    #[derive(Debug, PartialEq)]
    struct Config {
        /// How many price levels a quote shows.
        depth: u32,
        /// The venue orders route to.
        venue: &'static str,
    }

    /// The configuration before any is loaded.
    static DEFAULT: Config = Config { depth: 10, venue: "XNAS" };

    /// The service's configuration, read on every order.
    static CONFIG: Atomic<&'static Config> = Atomic::new(&DEFAULT);

    /// The type of a hook, which measures a symbol.
    type Hook = fn(&str) -> usize;

    /// The type of a function that strips a symbol's venue, its result borrowing its argument.
    type Strip = for<'a> fn(&'a str) -> &'a str;

    /// The type of `sum`: unsafe, variadic and C at once.
    type Sum = unsafe extern "C" fn(c_int, ...) -> c_int;

    /// The repr of a reference to a `Config`.
    type ConfigRepr<'a> = <&'a Config as Atom>::Repr;

    /// The repr of a hook.
    type HookRepr = <Hook as Atom>::Repr;

    /// A hook to measure a symbol with, or none.
    static HOOK: Atomic<Option<Hook>> = Atomic::new(None);

    /// A symbol's length in bytes: the hook the tests install.
    fn parse(symbol: &str) -> usize {
        symbol.len()
    }

    /// The symbol `symbol` is, after its venue prefix: a function pointer whose result borrows its
    /// argument.
    fn strip_venue(symbol: &str) -> &str {
        symbol.split_once(':').map_or(symbol, |(_, rest)| rest)
    }

    /// Twice `value`, through the C ABI.
    extern "C" fn twice(value: c_int) -> c_int {
        value.wrapping_mul(2)
    }

    /// The sum of the `count` `c_int`s that follow `count`: a variadic function of the test's own,
    /// since Windows' C library inlines its `printf` family, exporting no symbol to link.
    ///
    /// # Safety
    /// `count` `c_int`s follow `count`.
    #[expect(unsafe_code, reason = "a variadic function reads its arguments unsafely")]
    unsafe extern "C" fn sum(count: c_int, mut arguments: ...) -> c_int {
        (0..count).fold(0, |total, _| {
            // SAFETY: the caller passes `count` `c_int`s, and this reads `count` of them.
            total.wrapping_add(unsafe { arguments.next_arg::<c_int>() })
        })
    }

    #[test]
    fn a_config_and_a_hook_are_read_and_replaced_whole() {
        let next = Config { depth: 20, venue: "XLON" };
        assert_eq!(CONFIG.load(Acquire), &DEFAULT, "the default before any is loaded");
        CONFIG.store(Box::leak(Box::new(next)), Release);
        let config: &'static Config = CONFIG.load(Acquire);
        assert_eq!(config.depth, 20, "the replacement, read through the reference loaded");

        assert!(HOOK.load(Relaxed).is_none(), "no hook before one is installed");
        HOOK.store(Some(parse), Release);
        let hook = HOOK.load(Relaxed).expect("the hook installed");
        assert_eq!(hook("AAPL"), 4, "the hook, called through the pointer loaded");
    }

    #[test]
    fn a_reference_a_function_pointer_and_an_option_of_either_are_one_word() {
        assert_eq!(size_of::<Atomic<&Config>>(), size_of::<usize>(), "a reference");
        assert_eq!(size_of::<Atomic<Option<&Config>>>(), size_of::<usize>(), "an option of one");
        assert_eq!(size_of::<Atomic<fn(&str) -> usize>>(), size_of::<usize>(), "a function");
        assert_eq!(size_of::<Atomic<Option<fn()>>>(), size_of::<usize>(), "an option of one");
    }

    #[test]
    fn a_reference_obeys_the_laws_and_keeps_its_provenance() {
        let config = Config { depth: 5, venue: "XPAR" };
        assert_holds(round_trips(&config));
        assert_holds(round_trips(Some(&config)));
        assert_holds(round_trips(None::<&Config>));
        assert_holds(keeps_provenance(&config, |config: &Config| config.depth));
        assert_holds(keeps_provenance(Some(&config), |config| config.map(|config| config.venue)));
        assert_holds(decodes_as_promised::<ZeroNiche, &Config>((&config).to_repr()));
        assert_holds(decodes_as_promised::<ZeroValid, Option<&Config>>((&config).to_repr()));
        assert_holds(decodes_as_promised::<ZeroValid, Option<&Config>>(Primitive::from_bits(0)));
    }

    #[test]
    fn a_function_pointer_obeys_the_laws_and_keeps_its_provenance() {
        let hook: Hook = parse;
        assert_holds(keeps_provenance(hook, |hook| hook("IBM")));
        assert_holds(keeps_provenance(Some(hook), |hook| hook.map(|hook| hook("GE"))));
        assert_holds(decodes_as_promised::<TotalZeroNiche, Hook>(hook.to_repr()));
        assert_holds(decodes_as_promised::<Total, Option<Hook>>(Primitive::from_bits(0)));
    }

    #[test]
    fn no_bits_make_a_reference_or_a_function_pointer() {
        let config = Config { depth: 5, venue: "XPAR" };
        let forged: ConfigRepr<'_> = Primitive::from_bits(0xDEAD_BEE8);
        assert_eq!(<&Config as Atom>::from_repr(forged), None, "bits make null, no reference");
        let repr = (&config).to_repr();
        let moved = repr.with_packed_bits(repr.packed_bits() ^ 0x40);
        assert_eq!(<&Config as Atom>::from_repr(moved), None, "nor does moving its address");
        assert_eq!(
            <&Config as Atom>::from_repr(repr.clear_packed_bits(0)),
            Some(&config),
            "clearing no bits keeps it"
        );

        let forged: HookRepr = Primitive::from_bits(0xDEAD_BEE8);
        assert!(<Hook as Atom>::from_repr(forged).is_none(), "nor a function");
        let hook: Hook = parse;
        // Toggled, not set: Miri may place a function at an odd address.
        let moved = hook.to_repr().with_packed_bits(hook.to_repr().packed_bits() ^ 1);
        assert!(<Hook as Atom>::from_repr(moved).is_none(), "whose code's alignment holds no tag");
    }

    #[test]
    fn a_tag_set_in_the_bits_the_pointees_alignment_leaves_clear_keeps_the_address() {
        let config = Config { depth: 5, venue: "XPAR" };
        let address = (&config).to_repr().packed_bits();
        assert_eq!(address & 0b111, 0, "a `Config` is aligned to 8, three low bits clear");
        let tagged = (&config).to_repr().with_packed_bits(address | 0b101);
        assert_eq!(tagged.packed_bits(), address | 0b101, "the tags set, the address kept");
    }

    #[test]
    fn a_tagged_repr_decodes_once_its_tags_clear() {
        let config = Config { depth: 5, venue: "XPAR" };
        let repr = (&config).to_repr();
        let tagged = repr.with_packed_bits(repr.packed_bits() | 0b101);
        assert_eq!(<&Config as Atom>::from_repr(tagged), None, "a tagged repr is no reference");
        let cleared = tagged.clear_packed_bits(0b111);
        assert_eq!(<&Config as Atom>::from_repr(cleared), Some(&config), "until its tags clear");
    }

    #[test]
    fn a_bit_past_the_tags_makes_null() {
        let config = Config { depth: 5, venue: "XPAR" };
        let repr = (&config).to_repr();
        let moved = repr.with_packed_bits(repr.packed_bits() ^ 0b1000);
        assert_eq!(moved, Primitive::from_bits(0), "set or cleared, as the address held it");
        let cleared = repr.with_packed_bits(repr.packed_bits() | 1).clear_packed_bits(1 << 40 | 1);
        assert!(
            cleared == repr || cleared.is_bits(0),
            "a clear past the tags keeps it, or nulls it"
        );
    }

    #[test]
    fn a_tagged_null_decodes_as_no_value() {
        let null: ConfigRepr<'_> = Primitive::from_bits(0);
        let tagged = null.with_packed_bits(0b11);
        assert_eq!(<Option<&Config>>::from_repr(tagged), None, "neither `None` nor a reference");
    }

    /// The cell of `repr`, through the bound every repr has, as a caller of the hidden primitive
    /// operations reaches one.
    fn cell_of<R: Primitive>(repr: R) -> R::Cell {
        R::into_cell(repr)
    }

    #[test]
    fn a_mask_past_the_tags_changes_the_tags_alone() {
        let config = Config { depth: 5, venue: "XPAR" };
        let repr = (&config).to_repr();
        let cell = cell_of(repr);
        let before =
            <ConfigRepr<'_> as MaskBitwise>::fetch_or_mask(&cell, usize::MAX, AcqRel::CORE);
        assert_eq!(before, repr, "the reference before");
        let set = <ConfigRepr<'_> as Load>::load(&cell, Acquire::CORE);
        assert_eq!(set.packed_bits(), repr.packed_bits() | 0b111, "only the tags set");
        <ConfigRepr<'_> as MaskBitwise>::fetch_and_mask(&cell, 0, AcqRel::CORE);
        <ConfigRepr<'_> as MaskBitwise>::fetch_xor_mask(&cell, 1 << 40, AcqRel::CORE);
        let _ = <ConfigRepr<'_> as BitTest>::test_and_toggle_bit(&cell, 40, AcqRel::CORE);
        let after = <ConfigRepr<'_> as Load>::load(&cell, Acquire::CORE);
        assert_eq!(<&Config as Atom>::from_repr(after), Some(&config), "the address as it was");
    }

    #[test]
    fn an_operation_that_hands_out_bits_changes_the_tags_alone() {
        let config = Config { depth: 5, venue: "XPAR" };
        let repr = (&config).to_repr();
        let cell = cell_of(repr);
        <ConfigRepr<'_> as MaskBitwise>::or_mask(&cell, usize::MAX, Relaxed::CORE);
        let bits = <ConfigRepr<'_> as Load>::load_bits(&cell, Relaxed::CORE);
        assert_eq!(bits, repr.packed_bits() | 0b111, "only the tags set");
        <ConfigRepr<'_> as MaskBitwise>::xor_mask(&cell, usize::MAX, Relaxed::CORE);
        <ConfigRepr<'_> as MaskBitwise>::and_mask(&cell, 0, Relaxed::CORE);
        let after = <ConfigRepr<'_> as Load>::load(&cell, Acquire::CORE);
        assert_eq!(<&Config as Atom>::from_repr(after), Some(&config), "the address as it was");
    }

    #[test]
    fn a_reference_stored_by_an_exchange_is_read_whole() {
        let (first, second) =
            (Config { depth: 1, venue: "XNAS" }, Config { depth: 2, venue: "XLON" });
        let config = Atomic::new(&first);
        config.store_rmw(&second, Release);
        assert_eq!(config.load(Acquire).depth, 2, "the second, read through the reference");
    }

    #[test]
    fn a_repr_compares_hashes_and_prints_as_its_pointer() {
        let config = Config { depth: 5, venue: "XPAR" };
        let (repr, again) = ((&config).to_repr(), (&config).to_repr());
        assert_eq!(repr, again, "two reprs of one reference are equal");
        assert_ne!(repr, Primitive::from_bits(0), "and differ from null");
        let hasher = BuildHasherDefault::<DefaultHasher>::default();
        assert_eq!(hasher.hash_one(repr), hasher.hash_one(again), "and hash alike");
        let printed = format!("{repr:?}");
        assert_eq!(printed, format!("ReprOf({:?})", ptr::from_ref(&config)), "as its pointer");
    }

    /// Compiles only where `T` may cross threads and be shared between them.
    const fn crosses_threads<T: Send + Sync>() {}

    // A repr crosses threads as its value does.
    const _: () = {
        crosses_threads::<ReprOf<&'static Config>>();
        crosses_threads::<ReprOf<Hook>>();
        crosses_threads::<Atomic<&'static Config>>();
    };

    #[test]
    fn a_reference_prints_as_what_it_points_to() {
        let config = Atomic::new(&DEFAULT);
        assert_eq!(format!("{config:?}"), format!("{DEFAULT:?}"), "the config");
    }

    /// A box the writer leaked, for the test to free once the threads end.
    struct Leaked(NonNull<Config>);

    // SAFETY: a `Config` may cross threads, and only the test's thread frees it, once.
    #[expect(unsafe_code, reason = "hands a leaked box back to the thread that frees it")]
    unsafe impl Send for Leaked {}

    #[test]
    fn a_config_published_by_one_thread_is_read_whole_by_another() {
        let first = Config { depth: 1, venue: "XNAS" };
        let published = Atomic::new(&first);
        let boxed = thread::scope(|scope| {
            let writer = scope.spawn(|| {
                (2..=40)
                    .map(|depth| {
                        // Written here, and read by the other thread through the reference
                        // published: its `Release` store orders these writes before its reads.
                        let next =
                            NonNull::from(Box::leak(Box::new(Config { depth, venue: "XNYS" })));
                        // SAFETY: the box lives until both threads have ended.
                        #[expect(unsafe_code, reason = "borrows a box freed after the threads")]
                        published.store(unsafe { next.as_ref() }, Release);
                        Leaked(next)
                    })
                    .collect::<Vec<_>>()
            });
            scope.spawn(|| {
                let mut seen = 0;
                while seen < 40 {
                    let config = published.load(Acquire);
                    assert!(config.depth >= seen, "depths only grow");
                    assert!(config.venue == "XNAS" || config.venue == "XNYS", "written whole");
                    seen = config.depth;
                }
            });
            writer.join().expect("the writer does not panic")
        });
        for Leaked(next) in boxed {
            // SAFETY: each box was leaked once, and is freed once, after both threads, the last to
            // read it, ended.
            #[expect(unsafe_code, reason = "frees each box the writer leaked and published")]
            drop(unsafe { Box::from_raw(next.as_ptr()) });
        }
    }

    #[test]
    fn a_reference_to_a_local_lives_as_long_as_its_atomic() {
        let (low, high) = (Config { depth: 1, venue: "XNAS" }, Config { depth: 2, venue: "XNAS" });
        let current = Atomic::new(&low);
        thread::scope(|scope| {
            scope.spawn(|| current.store(&high, Release));
        });
        assert_eq!(current.load(Acquire).depth, 2, "the local the other thread stored");
        assert_eq!(current.swap(&low, AcqRel), &high, "the swap returns it");
    }

    #[test]
    fn a_compare_exchange_compares_addresses_not_values() {
        let (one, twin) = (Config { depth: 1, venue: "XNAS" }, Config { depth: 1, venue: "XNAS" });
        let current = Atomic::new(&one);
        assert_eq!(current.compare_exchange(&twin, &twin, AcqRel, Acquire), Err(&one), "a twin");
        let found = current.compare_exchange(&one, &twin, AcqRel, Acquire).expect("one itself");
        assert!(ptr::eq(found, ptr::from_ref(&one)), "the reference found, at `one`'s address");
        let replaced = current.update(SeqCst, SeqCst, |_| &one);
        assert!(ptr::eq(replaced, ptr::from_ref(&twin)), "update's value before");
    }

    #[test]
    fn every_function_pointer_is_held_and_called() {
        let measure: Atomic<Hook> = Atomic::new(parse);
        assert_eq!(measure.load(Relaxed)("MSFT"), 4, "a higher-ranked signature");
        let strip: Atomic<Strip> = Atomic::new(strip_venue);
        assert_eq!(strip.load(Relaxed)("XNAS:AAPL"), "AAPL", "one whose result borrows");
        let doubled: Atomic<extern "C" fn(c_int) -> c_int> = Atomic::new(twice);
        assert_eq!(doubled.load(Acquire)(21), 42, "the C ABI");
        let added: Atomic<Option<Sum>> = Atomic::new(Some(sum));
        let loaded = added.load(Relaxed).expect("the variadic function stored");
        // SAFETY: the three `c_int`s that `count` names follow it.
        #[expect(unsafe_code, reason = "calls the unsafe variadic function loaded")]
        let total = unsafe { loaded(3, 1, 2, 39) };
        assert_eq!(total, 42, "unsafe, variadic and C at once");
        let local: Atomic<Hook> = Atomic::new(|symbol| symbol.len().wrapping_add(1));
        assert_eq!(local.load(Relaxed)("A"), 2, "a closure that captures nothing");
    }

    #[test]
    fn a_function_pointer_compare_exchange_takes_current_from_a_load() {
        let hook: Atomic<Option<Hook>> = Atomic::new(None);
        assert_eq!(hook.compare_exchange(None, Some(parse), AcqRel, Acquire), Ok(None), "set");
        let current = hook.load(Acquire);
        let previous = hook.compare_exchange(current, None, AcqRel, Acquire);
        assert!(previous.is_ok_and(|previous| previous.is_some()), "replaced, by the one loaded");
    }

    /// A C library's handle, as C lays it out: a callback beside a count, the callback null until
    /// one is registered.
    #[repr(C)]
    struct Handle {
        /// What C calls on each event, or null: `void (*on_event)(int)`.
        on_event: Option<extern "C" fn(c_int) -> c_int>,
        /// The events seen.
        events: u64,
    }

    #[test]
    fn a_function_pointers_place_is_a_raw_pointers_which_c_may_write() {
        let mut handle = Handle { on_event: None, events: 0 };
        let slot = (&raw mut handle.on_event).cast();
        // SAFETY: `slot` is the handle's callback, aligned as a pointer, live for the view's
        // lifetime, reached only through the view meanwhile, and null or a function of this
        // signature, as C keeps it.
        #[expect(unsafe_code, reason = "views C's callback slot as an atomic")]
        let callback = unsafe { Atomic::<Option<extern "C" fn(c_int) -> c_int>>::from_ptr(slot) };
        assert!(callback.load(Acquire).is_none(), "no callback registered yet");
        callback.store(Some(twice), Release);
        assert_eq!(callback.as_ptr().cast(), slot, "the view's place is the slot");
        let on_event = handle.on_event.expect("registered through the view");
        assert_eq!(on_event(4), 8, "and C calls it in place");
        assert_eq!(handle.events, 0, "beside the count, untouched");
    }

    #[test]
    fn a_references_place_is_a_raw_pointers() {
        let config = Config { depth: 3, venue: "XPAR" };
        let mut slot: *const Config = ptr::from_ref(&DEFAULT);
        {
            // SAFETY: `slot` is a pointer's place, aligned and live for the view's lifetime,
            // reached only through it meanwhile, and holds a reference that lives as long as
            // `config`, which every write keeps.
            #[expect(unsafe_code, reason = "views a raw pointer's place as an atomic reference")]
            let view = unsafe { Atomic::<&Config>::from_ptr((&raw mut slot).cast()) };
            assert_eq!(view.swap(&config, AcqRel), &DEFAULT, "the reference the slot held");
        }
        assert!(ptr::eq(slot, &raw const config), "and the slot holds the one stored");
    }

    /// A venue's name, through a trait object.
    #[cfg(wide)]
    trait Venue: Sync + Debug {
        /// The venue's market identifier code.
        fn code(&self) -> &'static str;
    }

    /// The Nasdaq.
    #[cfg(wide)]
    #[derive(Debug)]
    struct Nasdaq;

    #[cfg(wide)]
    impl Venue for Nasdaq {
        fn code(&self) -> &'static str {
            "XNAS"
        }
    }

    /// The London Stock Exchange.
    #[cfg(wide)]
    #[derive(Debug)]
    struct London;

    #[cfg(wide)]
    impl Venue for London {
        fn code(&self) -> &'static str {
            "XLON"
        }
    }

    #[cfg(wide)]
    #[test]
    fn a_pair_holds_two_references_or_a_reference_beside_a_function_pointer() {
        let (bid, ask) = (Config { depth: 1, venue: "XNAS" }, Config { depth: 2, venue: "XNYS" });
        let best = Atomic::new((&bid, &ask));
        assert_eq!(best.load_rmw(Acquire), (&bid, &ask), "both references");
        let swapped = best.compare_exchange((&bid, &ask), (&ask, &bid), AcqRel, Acquire);
        assert_eq!(swapped, Ok((&bid, &ask)), "both exchanged at once");
        assert_holds(keeps_provenance((&bid, &ask), |(bid, ask)| (bid.depth, ask.venue)));
        let hook: Hook = parse;
        assert_holds(keeps_provenance((&bid, hook), |(bid, hook)| hook(bid.venue)));
        let pair: Atomic<(&Config, Hook)> = Atomic::new((&bid, hook));
        // A reference beside a function takes the stronger floor, the reference's.
        assert_eq!(pair.load_rmw(Acquire).1("XLON"), 4, "the function, beside the reference");
    }

    #[cfg(wide)]
    #[test]
    fn a_wide_reference_is_held_in_two_words_and_keeps_its_provenance() {
        // A constant exposes no provenance, so a `static` holds a wide reference only as `None`.
        static NAME: Atomic<Option<&'static str>> = Atomic::new(None);
        NAME.store_rmw(Some("XNAS"), Release);
        assert_eq!(NAME.load_rmw(Acquire), Some("XNAS"), "a `str`, stored at run time");

        let prices = [100_u64, 101, 102];
        let levels = Atomic::from(&prices[..]);
        assert_eq!(levels.load_rmw(Acquire), &prices[..], "a slice, its length whole");
        let found = levels.compare_exchange(&prices[..1], &prices[1..], AcqRel, Acquire);
        assert_eq!(found, Err(&prices[..]), "the same pointer, another length: no match");
        let before = levels.update(AcqRel, Acquire, |levels| &levels[1..]);
        assert_eq!(before.len(), 3, "the slice before");
        assert_eq!(levels.load_rmw(Acquire), &prices[1..], "and after");

        let venue: Atomic<&dyn Venue> = Atomic::new(&Nasdaq);
        venue.update(AcqRel, Acquire, |_| &London);
        assert_eq!(venue.load_rmw(Acquire).code(), "XLON", "a trait object, called through");
        assert_holds(keeps_provenance(&prices[..], |levels: &[u64]| levels.iter().sum::<u64>()));
        assert_holds(keeps_provenance::<&dyn Venue, _, _>(&London, Venue::code));
        assert_holds(round_trips(Some("XLON")));
    }
}
