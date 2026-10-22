# Values and Their Reprs

An [`Atomic<T>`][`Atomic`] holds a value of `T` as its repr, [`Atom::Repr`]: one
primitive an atomic instruction reads and writes, a `bool`, an integer, a
pointer, or two words. [`Atom`] is the trait that says how: `to_repr` encodes a
value on the way in, `from_repr` decodes one on the way out, and
[`REPRS`][`Atom::REPRS`] is the range of reprs its values take.

## Why a Repr

An instruction knows integers and pointers, not a quote or a lock. So a value
goes in as its repr, every load decodes what it reads, and the decode runs with
no check, since an atomic holds only reprs a value encoded to: a store writes
`to_repr` of a value, and a read-modify-write runs only where its result is sure
to decode. A `char` is stored as its scalar value:

```rs
{{#include ../../crates/atomix/tests/book/values_and_their_reprs.rs:reprs}}
```

`from_repr` refuses each repr no value encodes to; a load calls
[`from_repr_unchecked`][`Atom::from_repr_unchecked`], which skips that check, as
`Atom`'s [safety contract][atom-safety] allows.

## Where `None` Goes

`REPRS` is a range of the repr's unsigned bits, and it may wrap through zero: an
`i8` from -1 to 1 runs from `0xFF` through `0x00` to `0x01`. Every repr outside
it is free, and `Option`'s `None` takes one. It takes zero where no value is
zero, as for a `NonZero`; otherwise a repr beside the range, past its end, or
before its start where that keeps the range narrower, which matters once the
value is a field of a packed struct. So an `Option` costs no width:

```rs
{{#include ../../crates/atomix/tests/book/values_and_their_reprs.rs:none}}
```

A value whose range holds every repr leaves `None` nothing, so its `Option` is
refused as the crate compiles, naming a type with a spare repr:

```rs
{{#include ../../crates/atomix/tests/compile_fail/option_with_no_spare_repr.rs}}
```

```text
{{#include ../../crates/atomix/tests/compile_fail/option_with_no_spare_repr.stderr:1:1}}
```

Each `Option` takes one more repr, so `Option<Option<Side>>` takes 2 and 3, and
[`RangedU64<2>`][`RangedU64`] leaves two, 0 and 1, for two of them.

## Validity

[`Atom::Validity`] says which reprs decode beyond those `to_repr` returns, and a
bound on it decides where an operation that can leave any repr is sound:

- `Total`, every repr a value, is what [`AtomAdd`] and [`AtomBitwise`] need,
  since an add or an or can leave any repr; what [`get_mut`][`Atomic::get_mut`]
  and [`from_mut`][`Atomic::from_mut`] need, which lend a value's place; and
  what zerocopy's `FromBytes` needs, which reads an atomic from any bytes.
- Zero decoding, `Total` or `ZeroValid`, is what bytemuck's `Zeroable` needs.
- Zero never decoding, `TotalZeroNiche` or `ZeroNiche`, puts `Option`'s `None`
  at zero.

The [`validity`] module's table gives each validity, and its `Option`'s. A hand
impl that states none is `Partial`, which promises nothing beyond its values;
the derive computes a type's from its fields, as
[Deriving Atom](deriving-atom.md#validity) shows.

## The Built-in Values

atomix implements `Atom` for `bool`, every integer, each `NonZero`, `char`,
`f16`, `f32` and `f64`, `()`, `PhantomData`, `Wrapping` and `Saturating` of a
value, `Option` of a value with a spare repr, the
[ranged integers](ranged-integers.md), and the pointers, `*mut`, `*const` and
`NonNull`, whose [tags](tagged-pointers.md) a pointer word keeps. A shared
reference and a function pointer are
[values too](references-and-function-pointers.md), and so are an
[array](arrays-and-element-places.md) and a [tuple](tuples-and-core-values.md)
of values, and core's small values, from an `Ipv4Addr` to a `Result`; with a
feature, std's `SystemTime`, `ExitStatus` and `BorrowedFd`, deranged's integers
and arbitrary-int's are values too, as
[Working with Other Crates](working-with-other-crates.md) shows. The 128-bit
integers and `f128`, and a tuple of two pointers, take
[two words](two-words.md), where the target has a 16-byte compare-exchange. The
[implementors of `Atom`][atom-implementors] list each.

## Implementing `Atom` by Hand

A type of your own derives `Atom`, as [Deriving Atom](deriving-atom.md) shows,
or implements it by hand. The impl is `const`, so a `static` of the type builds,
and `unsafe`, since every load decodes without a check and trusts it: its
[safety contract][atom-safety] lists what it promises.

```rs
{{#include ../../crates/atomix/tests/book/values_and_their_reprs.rs:hand_impl}}
```

The derive over a [`RangedU8<0, 100>`][`RangedU8`] writes the same impl and
keeps each promise itself, so it is the default; a hand impl is for a type whose
reprs no field's type can state.

[`Atom::REPRS`]: api/atomix/trait.Atom.html#associatedconstant.REPRS
[`Atom::Repr`]: api/atomix/trait.Atom.html#associatedtype.Repr
[`Atom::Validity`]: api/atomix/trait.Atom.html#associatedtype.Validity
[`Atom::from_repr_unchecked`]: api/atomix/trait.Atom.html#method.from_repr_unchecked
[`AtomAdd`]: api/atomix/trait.AtomAdd.html
[`AtomBitwise`]: api/atomix/trait.AtomBitwise.html
[`Atom`]: api/atomix/trait.Atom.html
[`Atomic::from_mut`]: api/atomix/struct.Atomic.html#method.from_mut
[`Atomic::get_mut`]: api/atomix/struct.Atomic.html#method.get_mut
[`Atomic`]: api/atomix/struct.Atomic.html
[`RangedU64`]: api/atomix/struct.RangedU64.html
[`RangedU8`]: api/atomix/struct.RangedU8.html
[`validity`]: api/atomix/validity/index.html
[atom-implementors]: api/atomix/trait.Atom.html#implementors
[atom-safety]: api/atomix/trait.Atom.html#safety
