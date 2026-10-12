# Values and Places

A value is what an atomic holds: a `Copy` value, encoded in one word or two. A
place is where it lives: an [`Atomic`], which threads share by reference. A
value nests inside another, as deep as its words allow, while an atomic is never
a value, so places sit side by side instead, each with its own word.

## Values Nest

A struct of an enum, beside an `Option` of a newtype over a ranged integer, is a
value as each of its fields is. It packs into one word as each field's
[`REPRS`][`Atom::REPRS`] says, and every load and store moves all of it in one
instruction:

```rs
{{#include ../../crates/atomix/tests/book/values_and_places.rs:nesting}}
```

[Deriving Atom](deriving-atom.md) lays each level out, and [Fields](fields.md)
changes one field of the word alone.

## An Atomic Is Never a Value

An atomic is a place, and a value is `Copy`, so holding a place as a value would
split it with each copy. No atomic holds an atomic, since an atomic is no
[`Atom`], and the error lists what an atomic value is:

```rs
{{#include ../../crates/atomix/tests/compile_fail/atomic_of_an_atomic.rs}}
```

```text
{{#include ../../crates/atomix/tests/compile_fail/atomic_of_an_atomic.stderr:1:9}}
```

To keep several atomics together, put them side by side, as below; to point at
one, store a `NonNull` to it. The derive refuses a field that is an atomic, a
cell or a lock the same way, once.

## Places Side by Side

Atomics that change apart sit side by side: in an array, which a `static` builds
with `[const { AtomicU64::new(0) }; N]`, since an atomic is not `Copy`; in a
boxed slice, as many as known at run time; or in a struct of atomics,
`#[repr(C)]` where another process lays it out too, as
[Working with Other Crates](working-with-other-crates.md) shows:

```rs
{{#include ../../crates/atomix/tests/book/values_and_places.rs:side_by_side}}
```

Side by side, atomics share a cache line, and a core that writes one takes the
line from every other core that reads or writes its neighbours. atomix has no
padding of its own: crossbeam-utils' [`CachePadded`], aligned to 128 bytes on
`aarch64` and `x86_64`, gives each atomic a line, and takes atomix's atomics as
it takes core's. The `counters` example times both:

```rs
{{#include ../../crates/atomix/examples/counters.rs:counts}}
```

```sh
cargo run --release -p atomix-rs --example counters
```

Counts side by side slow each other as threads are added, and counts in their
own `CachePadded` do not, as [Performance](performance.md#under-contention)
measures on one to eight threads.

## Exclusive Access

A `&mut` to an atomic means no other thread can see it, before it is shared or
once it is shared no more, so the value is read and written there with no atomic
instruction: [`get`][`Atomic::get`], [`set`][`Atomic::set`] and
[`with_mut`][`Atomic::with_mut`] for any value, and
[`into_inner`][`Atomic::into_inner`] to take it out. A primitive that is its own
repr, every pattern of whose bits is a value, lends its place both ways:
[`get_mut`][`Atomic::get_mut`] and [`from_mut`][`Atomic::from_mut`] for one,
[`get_mut_slice`][`Atomic::get_mut_slice`] and
[`from_mut_slice`][`Atomic::from_mut_slice`] for a slice of them:

```rs
{{#include ../../crates/atomix/tests/book/values_and_places.rs:exclusive}}
```

[`Atom::REPRS`]: api/atomix/trait.Atom.html#associatedconstant.REPRS
[`Atom`]: api/atomix/trait.Atom.html
[`Atomic::from_mut_slice`]: api/atomix/struct.Atomic.html#method.from_mut_slice
[`Atomic::from_mut`]: api/atomix/struct.Atomic.html#method.from_mut
[`Atomic::get_mut_slice`]: api/atomix/struct.Atomic.html#method.get_mut_slice
[`Atomic::get_mut`]: api/atomix/struct.Atomic.html#method.get_mut
[`Atomic::get`]: api/atomix/struct.Atomic.html#method.get
[`Atomic::into_inner`]: api/atomix/struct.Atomic.html#method.into_inner
[`Atomic::set`]: api/atomix/struct.Atomic.html#method.set
[`Atomic::with_mut`]: api/atomix/struct.Atomic.html#method.with_mut
[`Atomic`]: api/atomix/struct.Atomic.html
[`CachePadded`]: https://docs.rs/crossbeam-utils/0.8.23/crossbeam_utils/struct.CachePadded.html
