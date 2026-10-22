# What Each Operation Costs

An operation of atomix is the instruction its name promises: a `load` is one
load, a `fetch_add` one add, a field's `set` one bitwise or. Where a target has
no such instruction, the operation does not exist there, and the
compare-exchange loop that would stand in for it is spelled out, `update` or
`try_update`. So code that compiles has no compare-exchange loop it did not ask
for, and no lock: atomix has no fallback. At `aarch64`'s default CPU, without
LSE, a read-modify-write is an LL/SC loop, or on Linux an outline call that runs
one where the CPU lacks LSE, as core's is; the `+lse` row of
[The Flags for Full Speed](platforms-and-features.md#the-flags-for-full-speed)
makes it one instruction. The facade's [What Each Operation Costs][facade-costs]
and [Platforms][facade-platforms] sections hold the detail for each target.

## One Instruction, or a Loop by Name

Whether a repr runs an operation in one instruction is a trait of the primitive,
which the target implements or not:

| Capability          | Gives                                                                    |
| ------------------- | ------------------------------------------------------------------------ |
| [`Load`], [`Store`] | `load`, `store`                                                          |
| [`Swap`]            | `swap`                                                                   |
| [`FetchAdd`]        | `fetch_add`, `fetch_sub`; a field's at the top of its word               |
| [`MaskBitwise`]     | `and`, `or`, `xor`, `not`; a field's, and a `bool` field's `set` and kin |
| [`FetchBitwise`]    | `fetch_and`, `fetch_or`, `fetch_xor`, `fetch_not`                        |
| [`MinMax`]          | `fetch_max`, `fetch_min`                                                 |
| [`BitTest`]         | `bit_set`, `bit_clear`, `bit_toggle`; a `bool` field's `test_and_set`    |
| [`ReadByExchange`]  | `load_rmw`                                                               |

`x86_64` has neither [`FetchBitwise`] nor [`MinMax`]: its `lock or` cannot
return the value it found, and it has no maximum. atomix has no 16-byte swap,
add or bitwise operation, and a 16-byte load or store needs LSE2 or AVX. Each
refusal names what to call instead, which is often the loop:

```rs
{{#include ../../crates/atomix/tests/book/what_each_operation_costs.rs:by_name}}
```

`update` and `try_update` replace the value with what their closure makes of it,
retrying until no other write intervenes, so the closure may run more than once.
They compare reprs, so they converge: every value has one repr, as [`Atom`]
asks, which also makes `-0.0` and `0.0` two values, and a NaN equal only to its
own bits.

## The Same Code as Core

Where core has an operation too, atomix's compiles to core's own code.
`tests/codegen.rs` writes a twin of each probe over core's atomics: core's
operation of the same name, its ordering written as a literal, or a field's, a
tag's or an element's bits masked by hand, an element's after the check of its
index a slice's index makes. It holds each twin to its probe, instruction for
instruction and operand for operand, on every target it builds for: `aarch64`
Linux, macOS and Android, and `x86_64` Linux, macOS and Windows. The two part
only where its tables pin how:

- a bit's test on `x86_64`, [below](#a-bit-chosen-at-run-time), where LLVM's own
  is a compare-exchange loop at some bits, and masks a run-time element's bit
  again after its check, an `andl` atomix's place spares;
- a 128-bit update at Android's default CPU, without LSE, whose first read is a
  compare-exchange of zero for zero in atomix and one compare shorter in core.

Two more differ in little: a checked element's test on `aarch64` ends in as many
other instructions, and an unchecked element's load schedules its shift
otherwise, atomix's a `movl` longer at Windows' default CPU.

[Performance](performance.md) has what each costs, measured.

## Discarding the Value Before

`and`, `or`, `xor` and `not` return nothing, and so need no loop on any target:
`or` is `lock or` on `x86_64` and `ldset` on `aarch64`. Their `fetch_` forms,
which return the value before, exist where the target has [`FetchBitwise`].
These four alone have a form that discards, under [portable-atomic]'s names: a
`fetch_add` whose value goes unused is `lock add` already, so there is no `add`.
For one bit, `bit_set` and its kin return the bit before, in one instruction on
both targets, below. All of this holds in an optimized build: unoptimized,
`x86_64` compiles `and` and its kin as compare-exchange loops, as the docs of
[`Atomic::and`] say.

## No Loop on 16 Bytes

A 128-bit value, or two words, has no `swap` on any target, since none exchanges
16 bytes in one instruction:

```rs
{{#include ../../crates/atomix/tests/compile_fail/swap_on_a_wide_value.rs}}
```

```text
{{#include ../../crates/atomix/tests/compile_fail/swap_on_a_wide_value.stderr:1:8}}
```

Where a target has no 16-byte load or store, [`Atomic::load_rmw`] reads with one
compare-exchange of zero for zero, and [`Atomic::store_rmw`] writes with a loop
of them, each named for what it is. The exchange takes the cache line exclusive
and writes it, so many readers of one value contend where a load would not;
[Two Words](two-words.md) says when that matters, and what it costs.

## What a Value Means

A read-modify-write must also mean something for the value. Adding to a `bool`
means nothing:

```rs
{{#include ../../crates/atomix/tests/compile_fail/add_without_the_capability.rs}}
```

```text
{{#include ../../crates/atomix/tests/compile_fail/add_without_the_capability.stderr:1:9}}
```

[`AtomAdd`] says that an add on the repr is an add on the value, [`AtomOrd`]
that the repr's order is the value's, and [`AtomBitwise`] that the bitwise
operations combine values. Each is safe to implement: `AtomAdd` and
`AtomBitwise` need every repr to decode, and the larger of two reprs is one of
them, so a wrong impl gives wrong values, never undefined behaviour. A newtype
derives each from its field:

```rs
{{#include ../../crates/atomix/tests/book/what_each_operation_costs.rs:capability}}
```

A raw pointer's `and`, `or` and `xor` take a `usize` mask on its address, as
core's `AtomicPtr`'s do, and keep its provenance:

```rs
{{#include ../../crates/atomix/tests/book/what_each_operation_costs.rs:pointer_mask}}
```

A mask's bits have no names: [Tagged Pointers](tagged-pointers.md) names each as
a field of a word, with the pointer's alignment checked as the crate builds.

## A Bit Chosen at Run Time

[`Atomic::bit_set`], [`Atomic::bit_clear`] and [`Atomic::bit_toggle`] set, clear
or invert one bit, chosen at run time, and return it before: one `ldset`,
`ldclr` or `ldeor` on `aarch64`, and one `lock bts`, `btr` or `btc` on `x86_64`,
from 16 bits up:

```rs
{{#include ../../crates/atomix/tests/book/what_each_operation_costs.rs:bit}}
```

On `x86_64` that `lock bts` is an `asm!` block, since LLVM's own test of a bit,
written over core's `fetch_or`, is a compare-exchange loop at the lowest bit, at
the top one, and wherever the result lands in an `Option`. `tests/codegen.rs`
pins both: each probe's twin over core's atomics is its code on `aarch64`, and
on `x86_64` its table `X86_64_TWINS` holds what LLVM writes instead: the loops,
and elsewhere a `lock bts` and a shift, a `lock xadd` to toggle the top bit, or
a `lock or` where the result is discarded. The `asm!` costs a `movl` more than
LLVM's own for a constant bit, which a field's test takes as an immediate, so
[Fields](fields.md) are the way to name a constant bit; and where `bit_set`'s
result is discarded, it keeps the `setb` that LLVM's `lock or` does without.
[A Bitmap Allocator](a-bitmap-allocator.md) takes its slots with `bit_set`.

## In a Constant

Each operation is `const` where core's is, so an atomic built in a constant is
read and written there too, by a crate that enables `const_trait_impl`:

```rs
{{#include ../../crates/atomix/tests/book/what_each_operation_costs.rs:constant}}
```

A pointer's compare-exchange, offsets and masks are not `const`, as core's are
not, and under loom no operation is.

[`Atom`]: api/atomix/trait.Atom.html
[`AtomAdd`]: api/atomix/trait.AtomAdd.html
[`AtomBitwise`]: api/atomix/trait.AtomBitwise.html
[`AtomOrd`]: api/atomix/trait.AtomOrd.html
[`Atomic::and`]: api/atomix/struct.Atomic.html#method.and
[`Atomic::bit_clear`]: api/atomix/struct.Atomic.html#method.bit_clear
[`Atomic::bit_set`]: api/atomix/struct.Atomic.html#method.bit_set
[`Atomic::bit_toggle`]: api/atomix/struct.Atomic.html#method.bit_toggle
[`Atomic::load_rmw`]: api/atomix/struct.Atomic.html#method.load_rmw
[`Atomic::store_rmw`]: api/atomix/struct.Atomic.html#method.store_rmw
[`BitTest`]: api/atomix/trait.BitTest.html
[`FetchAdd`]: api/atomix/trait.FetchAdd.html
[`FetchBitwise`]: api/atomix/trait.FetchBitwise.html
[`Load`]: api/atomix/trait.Load.html
[`MaskBitwise`]: api/atomix/trait.MaskBitwise.html
[`MinMax`]: api/atomix/trait.MinMax.html
[`ReadByExchange`]: api/atomix/trait.ReadByExchange.html
[`Store`]: api/atomix/trait.Store.html
[`Swap`]: api/atomix/trait.Swap.html
[facade-costs]: api/atomix/index.html#what-each-operation-costs
[facade-platforms]: api/atomix/index.html#platforms
[portable-atomic]: https://docs.rs/portable-atomic/1.15.0/portable_atomic/
