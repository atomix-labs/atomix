# Ranged Integers

[`RangedU64<MIN, MAX>`][`RangedU64`] holds a `u64` from `MIN` to `MAX`, and its
siblings, [`RangedU8`] to [`RangedIsize`], hold each other integer so. `MAX`
defaults to the integer's own, so `RangedU64<3>` is any `u64` from 3 up. A
ranged integer is a value in plain Rust and in an atomic alike, and it is what
leaves an `Option`, or a lock's states, reprs of their own.

## Why a Ranged Integer

It is built as core's `NonZero` is: its private field is a pattern type, so the
range is a validity invariant. That buys three things a plain integer with a
check cannot:

- rustc keeps an `Option`'s `None` outside the range, in the integer's own
  width, in a `Vec` or a struct as in an atomic;
- Miri checks the range wherever a value is made, as it does for `NonZero`;
- LLVM knows the range, so a comparison outside it folds to a constant:
  `tests/codegen.rs` pins `value.get() < 3` on a `RangedU64<3>` as one
  instruction that returns `false`.

deranged's integers hold the same ranges in a plain integer, so its `Option`
takes a second word:

```rs
{{#include ../../crates/atomix/tests/book/ranged_integers.rs:niche}}
```

With the `deranged-05` feature, deranged 0.5's integers are values too, stored
as atomix's are, and each converts to and from atomix's of the same range with
`From`, as [Working with Other Crates](working-with-other-crates.md#deranged-05)
shows.

## The Integers Below the Range

The integers below `MIN` are no value's, so a value that holds a ranged integer,
and its `Option`, take them. A lock whose owner's id is a `RangedU64<3>` keeps
`None`, `Free` and `Poisoned` at 0, 1 and 2, and every other state in the id
itself, one `u64` in all, as
[Deriving Atom](deriving-atom.md#an-enum-with-fields) lays out and
[A Lock Word](a-lock-word.md) takes and frees. One spare integer below `MIN`
leaves room for one `Option`, two for two.

## Holding the Range

An operation is infallible only where the range is closed under it, so
arithmetic is `checked_` or `saturating_`, a conversion from an integer, a
`NonZero`, another ranged integer or a text is checked, and there is no
`Default`, since zero may lie outside the range. A refusal is a [`RangeError`],
which names the integer and the range, or a [`ParseRangeError`] for a text.
[`expand`][`RangedU64::expand`] moves a value to a wider range of its integer,
with no check, [`narrow`][`RangedU64::narrow`] to a narrower one, with one, and
[`to_range`][`RangedU64::to_range`] to any other; a range of values iterates:

```rs
{{#include ../../crates/atomix/tests/book/ranged_integers.rs:api}}
```

A `MIN` above `MAX` names no type, and rustc refuses it where the type is used,
in a message that quotes the bounds but names no file or line:

```rs
{{#include ../../crates/atomix/tests/compile_fail/ranged_integer_with_its_bounds_swapped.rs}}
```

```text
{{#include ../../crates/atomix/tests/compile_fail/ranged_integer_with_its_bounds_swapped.stderr}}
```

[`RangedU64`]'s page lists the rest: each operation, each conversion, and the
traits it has, each `const` where `NonZero`'s is.

## In an Atomic

An atomic stores a ranged integer as its integer, and its `Option`'s `None` as
an integer outside the range, so an atomic of a full range's `Option` is
refused. It has [`AtomOrd`], since the larger or smaller of two values in the
range is in it, but neither [`AtomAdd`] nor [`AtomBitwise`], since a sum or an
or of two can leave it; a change that can is an [`update`][`Atomic::update`], by
name.

As a field of a packed struct, a ranged integer takes the bits its range needs
and no more: 31 for a `RangedU32<0, 0x7FFF_FFFF>`, which leaves the struct's top
32 bits to a count that [adds in place](fields.md):

```rs
{{#include ../../crates/atomix/tests/book/ranged_integers.rs:field_width}}
```

[`AtomAdd`]: api/atomix/trait.AtomAdd.html
[`AtomBitwise`]: api/atomix/trait.AtomBitwise.html
[`AtomOrd`]: api/atomix/trait.AtomOrd.html
[`Atomic::update`]: api/atomix/struct.Atomic.html#method.update
[`ParseRangeError`]: api/atomix/enum.ParseRangeError.html
[`RangeError`]: api/atomix/struct.RangeError.html
[`RangedIsize`]: api/atomix/struct.RangedIsize.html
[`RangedU64::expand`]: api/atomix/struct.RangedU64.html#method.expand
[`RangedU64::narrow`]: api/atomix/struct.RangedU64.html#method.narrow
[`RangedU64::to_range`]: api/atomix/struct.RangedU64.html#method.to_range
[`RangedU64`]: api/atomix/struct.RangedU64.html
[`RangedU8`]: api/atomix/struct.RangedU8.html
