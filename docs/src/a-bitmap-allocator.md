# A Bitmap Allocator

A bitmap allocator keeps an arena's slots free or taken by one bit each, 64 to a
word. A thread finds a clear bit, and takes it with one instruction that sets it
and says whether it was clear: [`bit_set`][`Atomic::bit_set`], `lock bts` on
`x86_64` and `ldseta` on `aarch64`. The thread that finds it clear holds the
slot; one that finds it set lost a race, and looks again.

## Allocating and Freeing

```rs
{{#include ../../crates/atomix/tests/cookbook/bitmap_allocator.rs:allocate}}
```

The search reads a word `Relaxed` and takes its lowest clear bit,
`trailing_ones`; only the `bit_set` claims, with `Acquire`, so the slot's last
holder's writes are seen. A free knows the bit is set, so it clears it with
[`and`][`Atomic::and`], `lock and` or `ldclrl`, which returns nothing:
[`bit_clear`][`Atomic::bit_clear`] would return the bit before, which no one
reads.

`bit_set` takes the bit at run time, one `lock bts` wherever the bit is, where a
test written by hand over core's `fetch_or` can be a compare-exchange loop, as
[What Each Operation Costs](what-each-operation-costs.md#a-bit-chosen-at-run-time)
shows.

An `Atomic<[bool; 64]>` holds the same bits, an element for each slot, whose
place claims the slot with `element(bit).test_and_set(Acquire)` and frees it
with `element(bit).clear(Release)`, each the same instruction after the index's
check, as [Arrays and Element Places](arrays-and-element-places.md) shows. The
bitmap keeps `u64` words for its search: a word's `trailing_ones` finds a clear
bit in two instructions on `x86_64`, `notq; tzcntq`, or three on `aarch64`,
`mvn; rbit; clz`, where a `[bool; 64]`'s load decodes 64 `bool`s, 18
instructions on `x86_64`.

## What a Claim Costs

The `contention` bench has 1 to 8 threads, each on a core of its own, claim one
bit and clear it again, through a field's `test_and_set` and through core's
`fetch_or` by hand, each `AcqRel` and so `ldsetal`, where the allocator's
`Acquire` claim is `ldseta`. A claim and its clear take longer with each thread
added, as every core takes the line in turn: the cost is the line's, not the
instruction's. [Under Contention](performance.md#under-contention) has the
figures.

## Using It

```rs
{{#include ../../crates/atomix/tests/book/cookbook/a_bitmap_allocator.rs:use}}
```

`tests/model.rs` runs two allocations under loom, and each takes a slot of its
own.

[`Atomic::and`]: api/atomix/struct.Atomic.html#method.and
[`Atomic::bit_clear`]: api/atomix/struct.Atomic.html#method.bit_clear
[`Atomic::bit_set`]: api/atomix/struct.Atomic.html#method.bit_set
