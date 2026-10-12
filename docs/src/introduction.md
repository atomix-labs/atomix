# Introduction

atomix gives each value that fits one atomic an atomic of its own type:
integers, plain or held to a range, `NonZero`s, `char`s, floats, pointers,
`Option`s that spend a spare bit pattern on `None`, and the structs and enums
`#[derive(Atom)]` packs into one word or two. Its orderings are types, checked
as it compiles, and each operation is the instruction its name promises: where a
target has none, the operation does not exist, and a compare-exchange loop is
`update`, by name.

It is for code that shares state between threads, or between processes through
shared memory, and builds lock-free structures on it: a queue's head, a slot's
state and lap, a lock's owner, a tagged pointer. On such paths a nanosecond
counts, so a load decodes its value without a check, a field of a packed struct
changes alone in one instruction, and nothing hides a loop or a lock behind a
name.

## What It Holds

- [`Atomic<T>`][`Atomic`] holds a `T` as its repr, one primitive an atomic
  instruction reads and writes, and decodes it on the way out; the integer,
  `bool` and pointer atomics are its aliases, such as [`AtomicU64`]. An atomic
  packed struct lends each field as an [`AtomicField`], a place one instruction
  on the whole word changes alone.
- [`Atom`] is what a value implements to be held: the built-in impls, the ranged
  integers such as [`RangedU64`], and the structs and enums the derive lays out,
  a pointer word that keeps tags in its pointer's low bits among them, or two
  words in one 16-byte atomic.
- The [`ordering`] types, each accepted only where it means something, and the
  fences they order.
- The pieces a structure builds on: a loom-shaped [`cell`], the spin [`hint`],
  and, under loom, the `model` that checks it.

## The Crates

| Crate             | What it holds                                                                    |
| ----------------- | -------------------------------------------------------------------------------- |
| [`atomix-rs`]     | everything: `atomix-core` at its root, and `atomix-derive` with `derive`         |
| [`atomix-core`]   | the typed atomic, the values it holds, and the orderings and primitives under it |
| [`atomix-derive`] | `#[derive(Atom)]`, and the read-modify-writes a newtype takes from its field     |

A program depends on `atomix-rs`, the facade, since crates.io's `atomix` is an
unrelated placeholder, and imports its library as `atomix`. The locks built on
these atomics are yet to come, in `atomix-lock`.

## What It Leaves Out

- A lock behind an atomic's name: a value no instruction holds whole, wider than
  128 bits, is refused as the crate compiles, and a sequence lock for one is
  atomix-lock's, yet to come.
- A loop behind an operation's name: `fetch_max` does not exist where it would
  be a compare-exchange loop, and `update` is the loop, by name. core's
  `fetch_update`, deprecated, has no counterpart.
- A choice at run time: the build's CPU flags decide what a target has, and
  nothing reads the CPU as the program runs.
- An implicit `SeqCst`: every operation names its ordering.
- Freeing memory: a value is `Copy`, and atomix frees nothing a pointer in one
  points to, so a structure that frees its nodes takes crossbeam-epoch, or
  another reclaimer, beside it.
- Stable Rust, and other targets: atomix needs a nightly, and builds for
  `aarch64` and `x86_64`, 64-bit and little-endian, on any OS, and for
  `arm64ec`.

[How atomix Compares](how-atomix-compares.md) names the crates that do these.

## How to Read This Book

The guide teaches the pieces in the order a program meets them, from a first
program to testing with loom; the cookbook builds concurrent structures from
them; the reference holds what each operation costs, what builds where, and how
atomix compares with its neighbours. The API documentation, under API Reference
in the menu bar, holds each item's detail, and each page here links to it.

Every listing on these pages is code the tests build, under
`crates/atomix/tests/` or among the examples, so the code shown compiles against
the crates as they are; the cookbook's structures run under loom too, from the
same files.

[`atomix-core`]: api/atomix_core/index.html
[`atomix-derive`]: api/atomix_derive/index.html
[`atomix-rs`]: api/atomix/index.html
[`Atom`]: api/atomix/trait.Atom.html
[`AtomicField`]: api/atomix/struct.AtomicField.html
[`AtomicU64`]: api/atomix/type.AtomicU64.html
[`Atomic`]: api/atomix/struct.Atomic.html
[`RangedU64`]: api/atomix/struct.RangedU64.html
[`cell`]: api/atomix/cell/index.html
[`hint`]: api/atomix/hint/index.html
[`ordering`]: api/atomix/ordering/index.html
