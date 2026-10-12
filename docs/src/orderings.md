# Orderings

An ordering says what an atomic operation orders around it: nothing beyond
itself, the writes it publishes, or the writes it sees. In atomix each ordering
is a type of its own, [`Relaxed`], [`Acquire`], [`Release`], [`AcqRel`],
[`SeqCst`] and [`StoreStore`], and each operation names the orderings it accepts
through a trait, so one that means nothing for the operation does not compile.

## Which Operation Takes Which

| Operation                                                                   | Bound             | Orderings                                              |
| --------------------------------------------------------------------------- | ----------------- | ------------------------------------------------------ |
| a load, `load_rmw`; a compare-exchange's `failure`, an `update`'s reads     | [`LoadOrdering`]  | `Relaxed`, `Acquire`, `SeqCst`                         |
| a store, `store_rmw`                                                        | [`StoreOrdering`] | `Relaxed`, `Release`, `SeqCst`                         |
| a read-modify-write; a compare-exchange's `success`, an `update`'s exchange | [`RmwOrdering`]   | `Relaxed`, `Acquire`, `Release`, `AcqRel`, `SeqCst`    |
| [`fence`], [`compiler_fence`]                                               | [`FenceOrdering`] | `Acquire`, `Release`, `AcqRel`, `SeqCst`, `StoreStore` |

core takes its [`Ordering`][core-ordering] as a value. Its lint
`invalid_atomic_ordering` refuses a wrong one written as a literal, and one that
arrives as an argument panics at run time: core's `load` matches on it, and
panics with "there is no such thing as a release load". In atomix the wrong
ordering is a type error wherever it comes from:

```rs
{{#include ../../crates/atomix/tests/compile_fail/load_with_release.rs}}
```

```text
{{#include ../../crates/atomix/tests/compile_fail/load_with_release.stderr:1:10}}
```

An ordering is a unit type, so it costs nothing: an operation compiles to the
code core's does with the same ordering written as a literal, as
[The Same Code as Core](what-each-operation-costs.md#the-same-code-as-core)
says.

## Publishing a Value

A `Release` store publishes every write before it to the `Acquire` load that
reads what it stored. Here the price is written `Relaxed`, and the flag's
`Release` and `Acquire` order it:

```rs
{{#include ../../crates/atomix/tests/book/orderings.rs:publish}}
```

Each structure in the cookbook says why each of its operations takes the
ordering it does, in an `// ORDERING:` comment beside it:
[A Free List](a-free-list.md)'s pop reads with `Acquire` the node its push
published with `Release`.

## Fences

A [`fence`] orders the accesses around it, so the accesses themselves can be
`Relaxed`: a `Release` fence before a store publishes as a `Release` store
would, and an `Acquire` fence after a load sees as an `Acquire` load would.

```rs
{{#include ../../crates/atomix/tests/book/orderings.rs:fences}}
```

`StoreStore` orders this thread's earlier stores before its later ones, and
nothing else. On `aarch64` it is `dmb ishst`, a barrier for stores alone, which
Rust's memory model has no name for: it creates no happens-before, so what it
orders must be atomic, and only the hardware orders it. Everywhere else, and
under loom and Miri, it is a `Release` fence, which on `x86_64` emits no
instruction. [A Sequence Lock Over Atomics](a-sequence-lock-over-atomics.md)
uses it to order a writer's sequence before its words.

[`compiler_fence`] orders accesses within one thread: it emits no instruction,
and orders nothing between threads, so loom does not model it.

## Handing an Ordering On

Each ordering but `StoreStore` converts into core's of its name with `into`, for
code that hands an ordering on to core, std or a C interface. Nothing converts
back, so an ordering stays a type. Code generic over a bound passes on only the
orderings the bound admits:

```rs
{{#include ../../crates/atomix/tests/book/orderings.rs:into_core}}
```

[`AcqRel`]: api/atomix/ordering/struct.AcqRel.html
[`Acquire`]: api/atomix/ordering/struct.Acquire.html
[`FenceOrdering`]: api/atomix/ordering/trait.FenceOrdering.html
[`LoadOrdering`]: api/atomix/ordering/trait.LoadOrdering.html
[`Relaxed`]: api/atomix/ordering/struct.Relaxed.html
[`Release`]: api/atomix/ordering/struct.Release.html
[`RmwOrdering`]: api/atomix/ordering/trait.RmwOrdering.html
[`SeqCst`]: api/atomix/ordering/struct.SeqCst.html
[`StoreOrdering`]: api/atomix/ordering/trait.StoreOrdering.html
[`StoreStore`]: api/atomix/ordering/struct.StoreStore.html
[`compiler_fence`]: api/atomix/fn.compiler_fence.html
[`fence`]: api/atomix/fn.fence.html
[core-ordering]: https://doc.rust-lang.org/core/sync/atomic/enum.Ordering.html
