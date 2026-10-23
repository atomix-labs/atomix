//! The listings of the chapter References and Function Pointers.

#[test]
// ANCHOR: reference
fn a_reference_is_stored_whole_and_read_through() {
    use std::thread;

    use atomix::Atomic;
    use atomix::ordering::{Acquire, Release};

    /// What a trading service reads on every order: replaced whole, never changed in place.
    struct Config {
        depth: u32,
    }

    let (narrow, wide) = (Config { depth: 10 }, Config { depth: 20 });
    // The atomic borrows both configs, so it lives no longer than either.
    let config = Atomic::new(&narrow);
    thread::scope(|scope| {
        scope.spawn(|| config.store(&wide, Release));
    });
    assert_eq!(config.load(Acquire).depth, 20, "the config the other thread stored, read through");
    assert_eq!(size_of_val(&config), 8, "one word, the reference's pointer");
    assert_eq!(size_of::<Atomic<Option<&Config>>>(), 8, "and `None` is null, so an `Option` too");
}
// ANCHOR_END: reference

#[test]
// ANCHOR: generic
fn generic_code_that_reads_relaxed_bounds_its_value() {
    use atomix::ordering::{HoldsNoReference, Relaxed};
    use atomix::{Atom, Atomic, Load};

    /// A count of any type that holds no reference, read `Relaxed`: a reference would ask
    /// `Acquire`, so the bound keeps one out.
    fn peek<T: Atom<Repr: Load> + HoldsNoReference>(count: &Atomic<T>) -> T {
        count.load(Relaxed)
    }

    assert_eq!(peek(&Atomic::new(7_u64)), 7, "a count read back");
    assert_eq!(peek(&Atomic::new([1_u8, 2])), [1, 2], "an array of bytes too");
}
// ANCHOR_END: generic

#[test]
// ANCHOR: encoded
fn a_reference_is_a_value_of_its_own() {
    use atomix::ordering::Acquire;
    use atomix::{Atom, Atomic};

    let depths = [10_u64, 20];
    // `iter` yields a `&u64` for each, a value of its own, whose `to_repr` encodes the reference:
    // `copied` first, for the `u64`'s.
    let reprs: Vec<u64> = depths.iter().copied().map(Atom::to_repr).collect();
    assert_eq!(reprs, [10, 20], "each depth's own repr");
    // `Atomic::new(&depths[0])` likewise holds the reference, not the `u64`.
    let depth = Atomic::new(&depths[0]);
    assert_eq!(*depth.load(Acquire), 10, "read through the reference");
}
// ANCHOR_END: encoded

#[test]
// ANCHOR: function
fn a_function_pointer_takes_every_ordering() {
    use atomix::Atomic;
    use atomix::ordering::{AcqRel, Acquire, Relaxed};

    /// How a symbol is measured.
    type Measure = fn(&str) -> usize;

    /// The measure, or none until one is installed.
    static HOOK: Atomic<Option<Measure>> = Atomic::new(None);

    // Where the atomic's type is known, a function's name coerces to the pointer.
    HOOK.store(Some(str::len), Relaxed);
    assert_eq!(HOOK.load(Relaxed).map(|measure| measure("AAPL")), Some(4), "the hook, called");
    // A closure that captures nothing coerces too.
    let before = HOOK.swap(Some(|symbol| symbol.len().wrapping_add(1)), AcqRel);
    assert_eq!(before.map(|measure| measure("MSFT")), Some(4), "the hook it replaced");
    assert_eq!(HOOK.load(Acquire).map(|measure| measure("MSFT")), Some(5), "and the new one");
}
// ANCHOR_END: function

#[test]
// ANCHOR: tags
fn a_reference_keeps_its_tags_in_the_bits_its_alignment_leaves_clear() {
    use atomix::ordering::{Acquire, Relaxed, Release};
    use atomix::{Atom, Atomic};

    /// What a trading service reads on every order, aligned to 4 by its `u32`: two low bits clear.
    struct Config {
        depth: u32,
    }

    /// The config in force, and whether a newer one is on its way: one word, the flag in bit 0.
    #[derive(Clone, Copy, Atom)]
    struct Published<'a> {
        config: &'a Config,
        stale: bool,
    }

    let (current, next) = (Config { depth: 10 }, Config { depth: 20 });
    let published = Atomic::new(Published { config: &current, stale: false });
    assert_eq!(size_of_val(&published), 8, "one word");
    // The flag publishes nothing of the config, so it takes every ordering: `lock bts`, or `ldset`.
    assert!(!published.fields().stale.test_and_set(Relaxed), "marked stale once");
    // The reference, read through the word, takes the floor: its tag cleared by one mask.
    assert_eq!(published.fields().config.load(Acquire).depth, 10, "the config marked stale");
    published.store(Published { config: &next, stale: false }, Release);
    let read = published.load(Acquire);
    assert_eq!((read.config.depth, read.stale), (20, false), "replaced whole");

    // A tuple is laid out as the struct of its fields: the same word.
    let pair = Atomic::new((&current, true));
    assert_eq!(size_of_val(&pair), 8, "a reference and its flag, in one word");
    assert_eq!(pair.fields().0.load(Acquire).depth, 10, "the reference, its tag cleared");
}
// ANCHOR_END: tags

#[cfg(wide)]
#[test]
// ANCHOR: function_tags
fn a_function_pointer_keeps_its_tags_in_a_word_of_their_own() {
    use atomix::ordering::{AcqRel, Acquire, Relaxed};
    use atomix::{Atom, Atomic};

    /// What handles a feed's messages, and the feed's id: two words, the id in the second.
    #[derive(Clone, Copy, Atom)]
    #[atom(repr = u128)]
    struct Handler {
        handle: fn(u64) -> u64,
        feed: u32,
    }

    /// Twice `value`: a handler.
    fn twice(value: u64) -> u64 {
        value.wrapping_mul(2)
    }

    let handler = Atomic::new(Handler { handle: |value| value, feed: 7 });
    handler.update(AcqRel, Acquire, |seen| Handler { handle: twice, feed: seen.feed });
    // `load_rmw`, as two words read on every target; see Two Words.
    let read = handler.load_rmw(Relaxed);
    assert_eq!(((read.handle)(21), read.feed), (42, 7), "the new handler, beside its feed");
}
// ANCHOR_END: function_tags

#[test]
// ANCHOR: places
fn a_function_pointers_place_is_a_raw_pointers() {
    use core::ffi::c_int;

    use atomix::Atomic;
    use atomix::ordering::{Acquire, Release};

    /// The callback, as C declares it: `int (*on_event)(int)`, null until one is registered.
    type Callback = Option<extern "C" fn(c_int) -> c_int>;

    /// A C library's handle: a callback beside a count of events.
    #[repr(C)]
    struct Handle {
        on_event: Callback,
        events: u64,
    }

    /// Twice `value`: a callback.
    extern "C" fn twice(value: c_int) -> c_int {
        value.wrapping_mul(2)
    }

    let mut handle = Handle { on_event: None, events: 0 };
    let slot = (&raw mut handle.on_event).cast();
    // SAFETY: `slot` is the handle's callback, aligned as a pointer, live while `callback` is,
    // reached only through it meanwhile, and null or a function of this signature, as C keeps it.
    #[expect(unsafe_code, reason = "views C's callback slot as an atomic")]
    let callback = unsafe { Atomic::<Callback>::from_ptr(slot) };
    assert!(callback.load(Acquire).is_none(), "no callback registered yet");
    callback.store(Some(twice), Release);
    assert_eq!(callback.as_ptr().cast(), slot, "the atomic's place is the slot");
    assert_eq!(handle.on_event.map(|on_event| on_event(4)), Some(8), "C calls it in place");
    assert_eq!(handle.events, 0, "the count beside the slot, untouched");
}
// ANCHOR_END: places
