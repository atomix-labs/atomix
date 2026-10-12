# A Lock Word

A lock word is a lock that fits one word and names its holder: free, poisoned by
a holder that panicked, or held by an owner's id. Taking it is one
compare-exchange, and the word tells whoever reads it who holds the lock, which
a spin lock's `bool` does not. atomix-lock, yet to come, builds its owned locks
on words like this one.

## The Word

```rs
{{#include ../../crates/atomix/examples/lock-word/lock_word.rs:word}}
```

An owner's id starts at 3, so its [`RangedU64`] leaves 0, 1 and 2 free, and the
derive gives them to the states: `Free` is 1, `Poisoned` 2, and `Option`'s
`None` 0, while `Held(OwnerId(7))` is 7, the id as it is. The lock costs no bit
beyond the id's `u64`. [Deriving Atom](deriving-atom.md) draws this
niche-filling layout, and [Ranged Integers](ranged-integers.md) says why the
range leaves the reprs below it free.

## Taking and Freeing

```rs
{{#include ../../crates/atomix/examples/lock-word/lock_word.rs:try_lock}}
```

`try_lock` exchanges `Free` for the owner's `Held` with `Acquire`, so the last
holder's writes are seen, and hands back the state that kept the lock where it
fails.

```rs
{{#include ../../crates/atomix/examples/lock-word/lock_word.rs:lock}}
```

`lock` waits on a `Relaxed` load while the lock is held, so a waiter reads the
cache line rather than writing it, and tries the exchange again once the lock is
free: a test-and-test-and-set lock. [`spin_loop`][`hint::spin_loop`] is loom's
yield under loom, so a model of a waiter ends.

```rs
{{#include ../../crates/atomix/examples/lock-word/lock_word.rs:unlock}}
```

The guard frees the lock with `Release`, publishing what its holder wrote, or
poisons it where a panic unwinds through it, so the next to try the lock learns
that what it guards may be half written.

## Using It

```rs
{{#include ../../crates/atomix/tests/book/cookbook/a_lock_word.rs:poison}}
```

The `lock-word` example has four threads take turns at the lock, each counting
with a `Relaxed` load and store that only the lock keeps from losing a count,
and `tests/model.rs` runs two of them under loom. Its builds abort on a panic,
as the workspace's profiles do, so the example cannot show the poisoning, which
the test above does.

[`RangedU64`]: api/atomix/struct.RangedU64.html
[`hint::spin_loop`]: api/atomix/hint/fn.spin_loop.html
