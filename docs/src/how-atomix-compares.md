# How atomix Compares

atomix sits beside std's atomics; portable-atomic, which carries them to every
target; three crates that hold a value of your own type, crossbeam-utils'
`AtomicCell`, atomic and atomig; radium and atomic_enum, which wrap std's for
one purpose each; and two that keep tags in a pointer, crossbeam-epoch and
tagptr. Each section says what the other does, from its own documentation and
source, where it and atomix differ, and where it is the better choice. The
versions read are those current on 2026-10-08: std 1.99, and the nightly of
2026-09-28 for what is unstable; and from crates.io, portable-atomic 1.15.0,
crossbeam-utils 0.8.23, atomic 0.6.1, atomig 0.4.3, radium 1.1.1, atomic_enum
0.3.0, crossbeam-epoch 0.9.21, tagptr 0.2.0, atomic_float 1.1.0, atomic-traits
0.4.0 and arc-swap 1.9.2. Where a crate's documentation and its source disagree,
the section says so. Each figure in nanoseconds is from the run
[Performance](performance.md) describes, on one Graviton4, where core, atomix,
portable-atomic, `AtomicCell`, atomic, atomig and atomic_enum each ran the same
operation in the same build. A cell that is wrong, or out of date, is a bug: a
pull request fixing it, with the line that shows it, is welcome.

What each holds:

|                               | Your own type                                   | `Option` in the same width           | One field of a word      | Tagged pointers                    | Two words                          | 128-bit                               |
| ----------------------------- | ----------------------------------------------- | ------------------------------------ | ------------------------ | ---------------------------------- | ---------------------------------- | ------------------------------------- |
| atomix                        | any `Atom`; structs and enums by derive         | yes, in any spare repr               | yes, one instruction     | yes: typed fields, provenance kept | yes                                | `aarch64`; `x86_64` with `cmpxchg16b` |
| std                           | no: `bool`, integers, pointers                  | no                                   | untyped masks            | untyped masks on `AtomicPtr`       | no                                 | nightly, and never on `x86_64` Linux  |
| portable-atomic               | no: std's, and floats                           | no                                   | untyped masks, `bit_set` | untyped masks on `AtomicPtr`       | `AtomicU128`, integers only        | yes, detected at run time             |
| crossbeam-utils' `AtomicCell` | any `T`                                         | where 1, 2, 4 or 8 aligned bytes     | no                       | no                                 | behind a lock                      | behind a lock                         |
| atomic                        | any `T: NoUninit`                               | `Option<NonZero>` alone              | no                       | no                                 | with `nightly`, not `x86_64` Linux | with `nightly`, not `x86_64` Linux    |
| atomig                        | up to 64 bits; one-field structs, C-like enums  | `Option<NonZero>`, `Option<NonNull>` | no                       | no                                 | no                                 | no                                    |
| radium                        | no: std's, and portable-atomic's with a feature | no                                   | no                       | no                                 | no                                 | portable-atomic's, with a feature     |
| atomic_enum                   | C-like enums, in a `usize`                      | no                                   | no                       | no                                 | no                                 | no                                    |
| crossbeam-epoch's `Atomic`    | any `T`, behind a heap pointer                  | no: null is its own                  | the tag's bits           | yes: untyped, truncated to fit     | no                                 | no                                    |
| tagptr                        | a pointer and a tag                             | no                                   | the tag's bits           | yes: untyped, unchecked            | no                                 | no                                    |

What each promises:

|                               | No hidden compare-exchange loop or lock  | Orderings                               | `const` `new` | loom              | Strict-provenance Miri                 | `no_std`       | Builds on                                                      |
| ----------------------------- | ---------------------------------------- | --------------------------------------- | ------------- | ----------------- | -------------------------------------- | -------------- | -------------------------------------------------------------- |
| atomix                        | yes: refused as it compiles, loops named | types: a wrong one does not compile     | yes           | yes               | yes; two words expose their provenance | yes            | nightly; `aarch64` and `x86_64`, 64-bit, little-endian; any OS |
| std                           | no: `fetch_max`, a used `fetch_or`       | an enum: a lint, or a panic at run time | yes           | loom's own copies | yes                                    | yes, in `core` | stable; every target                                           |
| portable-atomic               | no: core's loops, a seqlock fallback     | an enum: a panic at run time            | yes           | no                | yes, from rustc 1.91                   | yes            | stable 1.34; every target                                      |
| crossbeam-utils' `AtomicCell` | no: a seqlock for other sizes            | fixed: `Acquire`, `Release`, `AcqRel`   | yes           | no                | no: pointers lose provenance           | yes            | stable 1.60; every target                                      |
| atomic                        | no: a spinlock for other sizes           | an enum: a panic at run time            | yes           | no                | yes: it holds no pointers              | yes            | stable; every target                                           |
| atomig                        | no lock; core's loops                    | an enum: a panic at run time            | no            | no                | yes                                    | yes            | stable 1.60; every target                                      |
| radium                        | core's loops                             | an enum, which its `Cell`s ignore       | no            | no                | yes                                    | yes            | stable 1.60; every target                                      |
| atomic_enum                   | yes                                      | an enum: a panic at run time            | yes           | no                | yes                                    | yes            | stable; every target                                           |
| crossbeam-epoch's `Atomic`    | core's loops on a tag                    | an enum: a panic at run time            | `null` alone  | yes               | no: integers cast to pointers          | with `alloc`   | stable 1.61; every target                                      |
| tagptr                        | core's loops on a tag                    | an enum                                 | `null` alone  | no                | no: integers cast to pointers          | yes            | stable; every target                                           |

"Core's loops" are the operations core itself lowers to a compare-exchange loop
under a plain name: on `x86_64`, `fetch_max`, `fetch_min` and `fetch_nand`, and
`fetch_and`, `fetch_or` and `fetch_xor` whose result is used; on `aarch64` with
LSE, `fetch_nand`. Each crate that forwards to core's atomics shares them.
atomix has none of these names where the target would loop: it has no
`fetch_nand`, and `fetch_max` does not compile on `x86_64`, where the refusal
names `update`. Built without `+lse`, every crate's read-modify-write on
`aarch64` is an LL/SC loop, or on Linux an outline call that may run one, as
[What Each Operation Costs](what-each-operation-costs.md) says.

## `std::sync::atomic`

std has [`AtomicBool`][std-atomic-bool], an atomic for each integer up to 64
bits, and [`AtomicPtr`][std-atomic-ptr], each an alias of an unstable generic
`Atomic<T>` whose `T` is one of core's own primitives. Since 1.91, `AtomicPtr`'s
`fetch_or`, `fetch_and` and `fetch_xor` change a pointer's low bits and keep its
provenance, which is how a tag is set on stable; since 1.95, `update` and
`try_update` name the compare-exchange loop, and since 1.99 `fetch_update` is
deprecated in their favour. Orderings are the `Ordering` enum: a
`load(Ordering::Release)` written out is refused by the deny-by-default lint
`invalid_atomic_ordering`, and one passed in as a value panics at run time.
`AtomicU128` is unstable, under `integer_atomics`, and does not exist on
`x86_64` Linux at any `-C target-cpu`, whose target allows atomics of 64 bits at
most.

atomix's `AtomicU64` is `Atomic<u64>`, and where both have an operation, it is
core's code, but in the two places
[The Same Code as Core](what-each-operation-costs.md#the-same-code-as-core)
names: `tests/codegen.rs` holds each of atomix's operations to its twin over
core's atomics, instruction for instruction, and on one thread, measured on a
Graviton4, each costs what core's does. Beside the integers it holds any value
that implements `Atom`, with orderings as types and no operation that loops
under a plain name. A field of a packed struct or a pointer's tag is a typed
place, where std has a mask on the whole word; and atomix has two words, and 128
bits on `x86_64` Linux, which std has not. On `x86_64`, where a bit's test lands
in an `Option` or sits at the lowest or the top bit, LLVM writes core's
`fetch_or` by hand as a compare-exchange loop, and atomix's `bit_set` and a
field's `test_and_set` stay one `lock bts`.

std is the better choice where integers, `bool` and pointers are all a program
stores, on stable Rust or a target atomix does not build for: it needs no
dependency, and every crate on this page builds on it.

## `portable-atomic`

[portable-atomic] has std's atomic types, and `AtomicU128`, `AtomicF32` and
`AtomicF64`, on every target with 16-, 32- or 64-bit pointers, microcontrollers
without atomics among them. Where a width has no instruction, its default
`fallback` feature puts a global seqlock of 67 stripes behind the same names,
and `is_lock_free` says which a type takes. On `x86_64` Linux, whose default CPU
has no `cmpxchg16b`, it reads the CPU's features once at run time and calls the
128-bit instruction where the CPU has it; on `aarch64` its 128-bit load, store
and compare-exchange pick LSE or LSE2 the same way. Orderings are std's enum,
and `load(Release)` panics at run time. Its `AtomicPtr` sets a tag with
`fetch_or` or `bit_set` and keeps the pointer's provenance from rustc 1.91. Its
operations that return nothing, `or`, `and`, `xor`, `not` and `neg`, are one
`lock` instruction on `x86_64`; `fetch_max`, a used `fetch_or`, `fetch_neg`, a
float's `fetch_add` and a 128-bit read-modify-write are loops under those names.
Its documentation says each width's `bit_set` is `lock bts`; on `AtomicU8` the
source runs a compare-exchange loop.

atomix has no fallback and detects nothing at run time: an operation a target
lacks does not compile, and the build's CPU flags decide what it has. It holds
values of any type where portable-atomic holds integers and floats, and keeps a
pointer and a counter, or two pointers, in 16 bytes, with each pointer's
provenance exposed, where `AtomicU128` takes an integer. Where both have an
operation, the two are the same instructions and cost the same: a 16-byte
compare-exchange is 8.587 and 8.570 ns in atomix, 8.582 and 8.576 in
portable-atomic. On a CPU with LSE2, a build at the workspace's floor,
`+crc,+lse`, writes 16 bytes with portable-atomic's `stp`, found at run time, in
5.09 ns, and with atomix's `store_rmw`, a loop of compare-exchanges, in 12.0 ns;
a build that names the CPU makes atomix's `store` the same `stp`, as
[Performance](performance.md#against-core-and-its-peers) shows.

portable-atomic is the better choice on stable Rust, on embedded or any target
beyond atomix's, for a binary that must run on any `x86_64` or `aarch64` and use
`cmpxchg16b` or LSE2 where the CPU has it, or where a value with no instruction
of its width should still work, behind a lock. atomic-polyfill, its predecessor,
is deprecated in its favour.

## `crossbeam-utils`' `AtomicCell`

[`AtomicCell`] is a `Cell` threads share: it holds any `T`, `Copy` to load it
and `Copy + Eq` to compare-exchange it, with no trait to implement. A `T` of 1,
2, 4 or 8 bytes, aligned at least as its atomic is, goes through that atomic;
any other, `(u32, u32)`, `[u8; 8]`, every 16-byte value and `u128` among them,
takes a global striped seqlock, on every target, behind the same names. Its
orderings are fixed: `Acquire` loads, `Release` stores, `AcqRel` for the rest.
Its lock-free path copies `T` into the integer atomic with `transmute_copy`, so
a pointer read back from an `AtomicCell<*mut T>` has lost its provenance, and
Miri reports its use as undefined behaviour; so does a load of a lock-free `T`
with padding, which its source concedes. It is not built under loom.

atomix holds a `T` only where it fits one atomic word or two, and refuses any
other as it compiles, so no value of it takes a lock. Each `T` says through
`Atom` which reprs it takes, so an `Option` of a struct spends one of its spare
reprs on `None`, and a load decodes without a check; orderings are the caller's,
and pointers keep their provenance. A struct of 8 bytes aligned to 4 is one
`u64` in atomix, 0.75 ns to load, and 21 ns through `AtomicCell`'s seqlock; a
`u64`'s compare-exchange is a loop of core's weak one in `AtomicCell`, 0.6 to
0.8 ns slower than core's or atomix's.

`AtomicCell` is the better choice for a value of any type and any size, shared
between threads with no unsafe code and no derive, on stable Rust and any
target, where a lock behind the same names is acceptable for the sizes no
instruction fits.

## `atomic`

[atomic] has one `Atomic<T>` for any `T` that implements bytemuck's `NoUninit`,
which bytemuck derives. A `T` whose size is 1, 2, 4 or 8 bytes, aligned to its
size, goes through core's atomic of that width; any other takes one of 64 global
spinlocks, `load` included, unless its `fallback` feature is off, when the
operation panics at run time. Its `nightly` feature lets a 16-byte `T` go
through `AtomicU128`, and does not compile on `x86_64` Linux, where core has
none. Orderings are core's enum, and `load(Release)` panics. Pointers are not
`NoUninit`, so it holds neither a pointer nor an `Option<NonNull<T>>`. Its
documentation says a successful `compare_exchange` returns `new`; its source
returns the value it replaced, as core's does.

atomix's derive lays a struct out bit by bit, with no padding to align, so a
struct that atomic holds behind its spinlock, 8.4 ns to load, is one `u64` in
atomix, 0.75 ns; and atomix holds pointers, and the `Option` of any type with a
spare repr.

atomic is the better choice for a stable, portable `Atomic<T>` of any plain-data
type, sizes with no instruction included, where `const {
assert!(Atomic::<T>::is_lock_free()) }` is enough to keep a lock out of the
paths that matter.

## `atomig`

[atomig] has an `Atomic<T>` for any `T` that implements its `Atom`, converting
to and from a primitive of 64 bits at most, with no lock and no unsafe code. It
implements `Atom` for the floats, `char`, `NonZero`s, `NonNull`, their
`Option`s, and small integer arrays; its derive takes a struct of one field and
a C-like enum with an integer `repr`, and a derived enum's load compares its
repr against each variant, with a panic for none. `AtomLogic` and `AtomInteger`
add the bitwise and arithmetic operations of the repr. Orderings are core's
enum, and `load(Release)` panics. `new` is not `const`; `from_impl` is. Its
documentation says `target_has_atomic` is unstable and unused; its source and
changelog use it.

atomix is built on the same idea, a value converted to a primitive, and takes it
past atomig's: a derive that packs several fields into one word or two, and a
range of reprs for each type, `REPRS`, so a load decodes without a check and an
`Option` takes a spare repr of any type; orderings as types; fields and tags as
places of their own; 128 bits; loom. A derived enum's load is a bare `ldarb` in
atomix, and a compare and a branch to a panic in atomig; on one thread, where
the branch is always predicted, the two cost about the same, 0.76 and 0.82 ns.

atomig is the better choice for typed atomics of up to 64 bits on stable Rust
and any target, with no unsafe code and no dependency.

## `radium`

[radium] has the `Radium` trait, one API over an atomic and a `Cell`, so that
code generic over it builds both ways. `Atom<T>` is always an atomic, `Radon<T>`
always a `Cell`, and `Isotope<T>` an atomic where the target has one and a
`Cell` where it does not. It holds std's types, and with its `portable-atomic`
feature portable-atomic's, `AtomicU128` among them; a `Cell` ignores the
ordering it is passed.

atomix has no `Cell` mode: each type is an atomic, or is refused.

radium is the better choice for code that builds as atomic or not from one
source, as bitvec's does.

## `atomic_enum`

[atomic_enum] is an attribute that writes an atomic for a C-like enum: an
`AtomicUsize`, whatever the enum's `repr`, whose load matches the integer
against each variant and panics on none. It derives `Debug`, `Clone` and `Copy`
for the enum itself, so the enum cannot derive them again.

atomix's derive stores a C-like enum in the narrowest integer its variants fit,
a `u8` for a few, decodes it without a check where atomic_enum compares and
branches, and takes enums whose variants carry data. On one thread the two loads
cost the same, 0.76 ns against 0.77.

atomic_enum is the better choice for an atomic C-like enum on stable Rust, in
one attribute.

## `crossbeam-epoch`

[crossbeam-epoch] reclaims the memory of a lock-free structure once no thread
can still read it. Its `Atomic<T>` is a pointer to a `T` on the heap, read
through a pinned `Guard`, with a tag in the low bits `T`'s alignment leaves
clear: an untyped `usize`, which `with_tag` truncates to fit. Null is its own
value, so an `Option<Atomic<T>>` takes 16 bytes. It stores the pointer in an
`AtomicUsize`, casting it to an integer and back, which Miri's strict provenance
refuses. `Atomic::null` is `const`; `new` allocates. It has a loom mode, under
its own cfg.

atomix reclaims nothing: a node's memory is the caller's to free once no thread
can read it. Its tags are typed fields, checked against the pointee's alignment
as the crate compiles, and its pointers keep their provenance.

crossbeam-epoch is the better choice wherever a lock-free structure frees its
nodes, which atomix leaves to its caller; the two work side by side.

## `tagptr`

[tagptr] has `AtomicTagPtr<T, N>`, a pointer with `N` tag bits, in an
`AtomicUsize`. Nothing checks `N` against `T`'s alignment as it compiles:
`assert_alignment` panics at run time, where it is called. Its `fetch_add` on
the tag can carry into the pointer, which a `debug_assert` alone guards. It
casts integers to pointers, which Miri's strict provenance refuses.

tagptr is the better choice for a small tagged atomic pointer on stable Rust and
any target.

## Other Crates

- [atomic_float] has `AtomicF32` and `AtomicF64`, on stable and any target; its
  `fetch_add`, `fetch_sub`, `fetch_max` and `fetch_min` are compare-exchange
  loops, which its crate page says of the first two. atomix stores a float and
  has no `fetch_add` for one, since no target here adds one in an instruction:
  the loop is `update`.
- [atomic-traits] has traits over std's atomics, for code generic over their
  width. Its `fetch_update` names its orderings `(fetch_order, set_order)` and
  passes them to std's `(set_order, fetch_order)`, so a call that follows its
  names can panic.
- [arc-swap] holds an `Arc<T>` a reader loads without a lock and a writer
  replaces: a different problem, a snapshot shared by many readers, which atomix
  does not take on.

## Where atomix Fits

A program on stable Rust, on another target, or with values no instruction can
hold takes portable-atomic, `AtomicCell` or atomic, and a structure that frees
its nodes takes crossbeam-epoch beside atomix.
[What It Leaves Out](introduction.md#what-it-leaves-out) lists what atomix does
not do.

[`AtomicCell`]: https://docs.rs/crossbeam-utils/0.8.23/crossbeam_utils/atomic/struct.AtomicCell.html
[arc-swap]: https://docs.rs/arc-swap/1.9.2/arc_swap/
[atomic-traits]: https://docs.rs/atomic-traits/0.4.0/atomic_traits/
[atomic]: https://docs.rs/atomic/0.6.1/atomic/
[atomic_enum]: https://docs.rs/atomic_enum/0.3.0/atomic_enum/
[atomic_float]: https://docs.rs/atomic_float/1.1.0/atomic_float/
[atomig]: https://docs.rs/atomig/0.4.3/atomig/
[crossbeam-epoch]: https://docs.rs/crossbeam-epoch/0.9.21/crossbeam_epoch/
[portable-atomic]: https://docs.rs/portable-atomic/1.15.0/portable_atomic/
[radium]: https://docs.rs/radium/1.1.1/radium/
[std-atomic-bool]: https://doc.rust-lang.org/std/sync/atomic/type.AtomicBool.html
[std-atomic-ptr]: https://doc.rust-lang.org/std/sync/atomic/type.AtomicPtr.html
[tagptr]: https://docs.rs/tagptr/0.2.0/tagptr/
