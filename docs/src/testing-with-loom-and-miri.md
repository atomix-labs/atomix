# Testing with Loom and Miri

A test that runs threads tries the interleavings the machine happens to give it,
which may never be the one that breaks. Two tools try more: [loom], which runs a
model of the code under each interleaving, and each value a load may read,
within its bounds; and Miri, which interprets the code and reports undefined
behaviour, a pointer used past its provenance among it. atomix is built so that
the code it ships is the code both check.

## Loom

Under `--cfg loom`, with atomix's `loom` feature, every atomic, fence, cell and
spin hint in atomix is loom's, so the same source is the model; the facade's
[Model Checking][facade-model-checking] section lists what else changes there. A
crate models its own code with this dependency, and `RUSTFLAGS="--cfg loom"`:

```toml
[target.'cfg(loom)'.dependencies]
atomix = { version = "0.2", features = ["loom"] }
```

A model runs in `atomix::model::check`, and spawns its threads with
`atomix::model::{thread, Arc}`. This one checks that a bit set with `Release`
publishes the value written before it, to a thread that clears the bit with
`Acquire`, and that loom catches the same bit set `Relaxed`:

```rs
{{#include ../../crates/atomix/tests/model.rs:publish}}
```

`ZERO` is the model's packed struct with every field zero, and `ready` one of
its `bool` fields. With the `Relaxed` set, loom finds the interleaving where the
reader takes the bit yet reads the value from before its write, and the
`should_panic` test expects that assertion's message.

`check` bounds the preemptions loom tries at 3; `LOOM_MAX_PREEMPTIONS` sets
another bound, and no setting removes it. `LOOM_LOG`, a `tracing` filter such as
`trace`, prints each execution's trace with the test's output.

atomix runs its own models with `just check-loom`, which also lints every crate
under loom:

```sh
just check-loom
```

The cookbook's structures are modelled in `crates/atomix/tests/model.rs`, its
`structures` module, from the same source files their pages and examples show.

## Miri

Miri runs the tests with strict provenance, under which a pointer made from an
integer has none, and with isolation, its refusals warnings:

```sh
MIRIFLAGS='-Zmiri-strict-provenance -Zmiri-isolation-error=warn-nobacktrace' cargo miri test
```

One word keeps every pointer's provenance strictly, so a pointer word or a
pointer enum passes as it ships, as
[Provenance, Kept](tagged-pointers.md#provenance-kept) says. Two words expose
their pointers' provenance on hardware, and under Miri a [`DoubleWord`]'s cell
is a lock around plain copies of its words, which keeps each pointer's own
provenance: Miri then reports a pointer read back after its allocation is freed,
even where another allocation took its address. The lock orders every operation
after the one before, so Miri lets pass a race the orderings allow; loom, which
models the hardware's cell, catches those.

atomix runs every crate's tests under Miri each night, and with `just miri`, on
the targets [What CI Checks](platforms-and-features.md#what-ci-checks) lists;
`-Zmiri-env-forward=PROPTEST_CASES` and `PROPTEST_CASES=16` cut its property
tests to what Miri runs in time.

[`DoubleWord`]: api/atomix/struct.DoubleWord.html
[facade-model-checking]: api/atomix/index.html#model-checking
[loom]: https://docs.rs/loom/0.7.2/loom/
