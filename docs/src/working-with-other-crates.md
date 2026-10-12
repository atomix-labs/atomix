# Working with Other Crates

Each crate atomix works with is behind a feature of its own, which brings that
crate's traits for atomix's types, or atomix's for that crate's; none is on by
default. The facade's [Crate Features][facade-crate-features] section lists
them, a row each. The feature of a crate at 0.x names the version, as
`zerocopy-08` does, so the next 0.y can sit beside it. Each listing below runs
under its feature.

## `serde`

An atomic serializes as its value, read `Relaxed` as serde reads core's atomics,
and deserializes into a new atomic; a ranged integer serializes as its integer,
and refuses one outside its range on the way in, in serde's words:

```rs
{{#include ../../crates/atomix/tests/book/working_with_other_crates.rs:serde}}
```

Where the target has no load for the atomic's width, a 16-byte one without LSE2
or AVX, serializing reads with `load_rmw`, which writes the cache line.

## `zerocopy-08`

zerocopy's traits let a page of shared memory be read as a struct of atomics. An
atomic has each trait its cell has, `FromBytes` where every repr decodes, as
[`Atomic`]'s page says, but never `Immutable`, since its shared reference
writes. So a page this process lays out, before another maps it, becomes a
header through `mut_from_bytes`; a page another process may already be writing
is reached through [`Atomic::from_ptr`], since a `&mut [u8]` over it would be
aliased:

```rs
{{#include ../../crates/atomix/tests/book/working_with_other_crates.rs:zerocopy}}
```

Under loom, whose cells are not plain memory, an atomic has none of them.

## `bytemuck`

An atomic is `Zeroable` where its zero repr decodes, but for two words, whose
cell holds addresses alone; a struct of such atomics derives it. It has none of
the traits that read its bytes, `Pod`, `NoUninit` or `CheckedBitPattern`, since
each needs `Copy`, which no cell is; a ranged integer has `NoUninit`,
`CheckedBitPattern` and `Contiguous`.

```rs
{{#include ../../crates/atomix/tests/book/working_with_other_crates.rs:bytemuck}}
```

## `arbitrary`

A fuzz target builds an atomic from its input as it builds the atomic's value,
from the same bytes, and a ranged integer inside its range:

```rs
{{#include ../../crates/atomix/tests/book/working_with_other_crates.rs:arbitrary}}
```

## `arbitrary-int`

arbitrary-int's integers of any width, `UInt` and `Int`, are values, and a
packed struct stores each in its width: a `u4` in four bits. Not every pattern
of a `u4`'s `u8` is a `u4`, so an atomic of one has no add or bitwise operation
of its own; as a field, which stores its four bits alone, it has both, the add
at the word's top:

```rs
{{#include ../../crates/atomix/tests/book/working_with_other_crates.rs:arbitrary_int}}
```

## `deranged-05`

deranged 0.5's ranged integers, and their `OptionRanged` forms, are values, each
stored as its integer, a field taking the bits its range needs as atomix's own
do. `From` converts between each and atomix's own of the same bounds. It keeps
the integer, and compiles to a move at most:

```rs
{{#include ../../crates/atomix/tests/book/working_with_other_crates.rs:deranged}}
```

atomix keeps its own ranged integers beside deranged's for what a plain value
gains from them, as [Ranged Integers](ranged-integers.md) says.

## `loom`

`loom` brings loom's types under `--cfg loom`, and nothing without it:
[Testing with Loom and Miri](testing-with-loom-and-miri.md).

## Crates That Need No Feature

crossbeam-utils' `CachePadded` puts an atomic on a cache line of its own, as
[Values and Places](values-and-places.md) shows. A structure whose nodes are
freed while other threads may read them takes a reclaimer, crossbeam-epoch or
another, since atomix reclaims nothing.

[`Atomic::from_ptr`]: api/atomix/struct.Atomic.html#method.from_ptr
[`Atomic`]: api/atomix/struct.Atomic.html
[facade-crate-features]: api/atomix/index.html#crate-features
