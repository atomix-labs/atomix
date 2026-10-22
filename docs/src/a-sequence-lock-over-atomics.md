# A Sequence Lock Over Atomics

A sequence lock lets a writer change several words and readers read them whole,
without a reader writing anything: a reader reads the words, and retries if a
write overlapped. Readers share the cache line, so they do not slow each other
down, where a lock word or a 16-byte `load_rmw` would take the line from every
other reader. Here each word is an atomic, so a read that overlaps a write reads
each word whole, old or new, never a torn word or undefined behaviour; the
sequence, odd while a write is under way, tells the reader to retry.

A sequence lock over plain data, which a reader may read mid-write, needs a copy
that is atomic byte by byte: atomix-lock's `SeqLock`, yet to come, is that one.

## Writing

```rs
{{#include ../../crates/atomix/tests/cookbook/sequence_lock.rs:write}}
```

A writer claims the lock by making the sequence odd, with `Acquire` so its words
follow the last writer's, and a second writer waits. The
[`fence(StoreStore)`][`StoreStore`] orders the odd sequence before every word: a
reader that reads a word this write stored then reads the sequence moved on. On
`aarch64` it is `dmb ishst`, which orders stores alone, where a `Release` fence
is `dmb ish`, which orders loads too. It creates no happens-before in Rust's
model, so what it orders must be atomic, as each word is;
[Fences](orderings.md#fences) says what it is on each target.

## Reading

```rs
{{#include ../../crates/atomix/tests/cookbook/sequence_lock.rs:read}}
```

The orderings are Hans Boehm's, from
[Can Seqlocks Get Along with Programming Language Memory Models?][boehm-seqlocks]:
the first load of the sequence is `Acquire`, so the words a finished write
stored are read; the words are `Relaxed`; and the `Acquire` fence after them
makes a word stored by a later write pull that write's odd sequence into the
second load, which then differs from the first. `tests/model.rs` reads the lock
beside a writer that writes it twice: drop the fence, or make the first load
`Relaxed`, and loom finds a read that pairs one write's first word with another
write's second. The model keeps each of the two as a twin that must fail.

## Using It

```rs
{{#include ../../crates/atomix/tests/book/cookbook/a_sequence_lock_over_atomics.rs:use}}
```

[`StoreStore`]: api/atomix/ordering/struct.StoreStore.html
[boehm-seqlocks]: https://doi.org/10.1145/2247684.2247688
