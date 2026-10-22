# Getting Started

Add the crate, with its derive:

```sh
cargo add --git https://github.com/atomix-labs/atomix atomix-rs --features derive
```

The package is `atomix-rs`, and the library it adds is `atomix`: `use
atomix::Atomic;`. atomix needs a nightly Rust, `nightly-2026-09-28` or newer,
for the unstable features [Rust Version](platforms-and-features.md#rust-version)
names, and a `rust-toolchain.toml` beside the crate's `Cargo.toml` pins one for
every build:

```toml
[toolchain]
channel = "nightly-2026-09-28"
```

atomix builds at any CPU its targets run on, but some operations need a feature
a target's default CPU lacks, such as `cmpxchg16b` for two words on `x86_64`
Linux, or LSE's one-instruction read-modify-writes on `aarch64` Linux, which are
outline calls without it. A crate's own `.cargo/config.toml` names the CPU it
runs on; [Platforms and Features](platforms-and-features.md) gives the flags for
each target, and every feature.

## A First Program

A quote and its side pack into one `u64`, and `Option`'s `None` takes a repr no
quote has, so the atomic stays one word:

```rs
{{#include ../../crates/atomix/tests/book/getting_started.rs:first}}
```

The derive laid the quote out as it compiled, and the load decodes it with no
check: an atomic holds only the reprs its values encode to.
[Values and Their Reprs](values-and-their-reprs.md) says how, and
[Deriving Atom](deriving-atom.md) how the derive lays out a struct or an enum.

## Four Examples

In a checkout of the repository, four examples run a structure on threads and
print what they measured:

```sh
cargo run --release -p atomix-rs --features derive --example free-list
cargo run --release -p atomix-rs --features derive --example lock-word
cargo run --release -p atomix-rs --features derive --example quote-book
cargo run --release -p atomix-rs --example counters
```

`free-list` takes nodes from [a free list](a-free-list.md) and gives them back;
`lock-word` takes turns at [a lock word](a-lock-word.md); `quote-book` posts
quotes to a book's levels while takers race to take each, one [field](fields.md)
at a time; and `counters` times counts that share a cache line against counts
that each have one, as [Values and Places](values-and-places.md) shows.

Every listing in this book runs as a test too:

```sh
cargo test -p atomix-rs --all-features --test book
```
