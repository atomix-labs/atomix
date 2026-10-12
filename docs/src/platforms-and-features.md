# Platforms and Features

atomix builds for every `aarch64` and `x86_64` target with 64-bit pointers,
little-endian, on any OS or none, and for `arm64ec`, the `aarch64` code Windows
runs beside `x86_64` code. Every crate is `#![no_std]` and allocates nothing. It
needs a nightly Rust.

## What Builds Where

On any other target, the build script stops the build before rustc compiles a
line, with one error:

```text
error: atomix-core@0.1.0: atomix builds for `aarch64`, `arm64ec` and `x86_64`, little-endian with 64-bit pointers, not for `i686-unknown-linux-gnu`: on another target, an operation it promises as one instruction could be a compare-exchange loop, as a 64-bit add is on 32-bit x86
```

`crates/atomix-core/tests/unsupported_target.rs` holds the build to that message
for `i686`, for `x86_64` with 32-bit pointers, for big-endian `aarch64` and for
`riscv64`, and to none for `x86_64` FreeBSD and `arm64ec`. Within the targets it
builds for, what an operation lowers to depends on the features the target turns
on, not on its OS: the facade's [Platforms][facade-platforms] section gives, in
a table, what each target's default CPU has, and so what exists on it.

## The Flags for Full Speed

A target's default CPU is the oldest it runs on, and lacks the instructions some
operations need. A crate names the CPU it runs on, or a floor, in its
`.cargo/config.toml`, as atomix's own names the workspace's floor for its tests
and benches:

```toml
[target.'cfg(target_arch = "x86_64")']
rustflags = ["-C", "target-cpu=x86-64-v3"]

[target.'cfg(all(any(target_arch = "aarch64", target_arch = "arm64ec"), not(target_os = "macos")))']
rustflags = ["-C", "target-feature=+lse"]
```

| Flag                           | Adds                                                                                    |
| ------------------------------ | --------------------------------------------------------------------------------------- |
| `+lse`, on `aarch64` but macOS | each read-modify-write in one instruction, in place of an outline call or an LL/SC loop |
| a CPU with LSE2, on `aarch64`  | a 16-byte `load` and `store`, `ldp` and `stp`, in place of `load_rmw` and `store_rmw`   |
| `x86-64-v2`, on `x86_64`       | `cmpxchg16b`, so 128-bit atomics and two words, where the target's default lacks it     |
| `x86-64-v3`, on `x86_64`       | AVX too, so a 16-byte `load` and `store`, `vmovdqa`                                     |

macOS on `aarch64` needs none: its default, the M1, has LSE and LSE2. LSE2 comes
with a CPU rather than a flag, so a build names the CPU it runs on, such as
`neoverse-v2` for Graviton4; the facade's [Platforms][facade-platforms] section
names those that have it, and the CPUs an OS requires already, whose flags cost
nothing. Without a flag, everything still builds that the default CPU can run: a
16-byte read is a compare-exchange, by name, and two words are refused on
`x86_64` Linux, naming the CPU they need.

## Rust Version

atomix needs `nightly-2026-09-28` or newer, for `const_trait_impl`, pattern
types and the other unstable features its crates enable; that nightly's version,
1.101, is every crate's `rust-version`. A derived type without parameters needs
no feature gate in the crate that derives it; [Deriving Atom](deriving-atom.md)
says when one with parameters does.

## Crate Features

None is on by default. Under `--cfg loom`, whose cells are not plain memory, an
atomic has neither zerocopy's traits nor `Zeroable`.

| Feature         | Adds                                                                       |
| --------------- | -------------------------------------------------------------------------- |
| `derive`        | `#[derive(Atom)]`: projections, tagged pointers; a newtype's capabilities  |
| `serde`         | serde's traits for an atomic, and a ranged integer, held to its range      |
| `zerocopy-08`   | zerocopy's traits for an atomic, as its validity allows, never `Immutable` |
| `bytemuck`      | `Zeroable` for an atomic, and the bit-pattern traits for a ranged integer  |
| `arbitrary`     | `Arbitrary` for an atomic, and a ranged integer, held to its range         |
| `arbitrary-int` | `Atom`, `AtomOrd`, `FieldBitwise` and `FieldAdd` for `UInt` and `Int`      |
| `deranged-05`   | `Atom`, `AtomOrd` and `From` with atomix's own for deranged 0.5's integers |
| `loom`          | loom's types under `--cfg loom`; nothing without the cfg                   |

That is the facade's table; atomix-core has every feature but `derive`. A
feature is named with `cargo add`, as `cargo add --git
https://github.com/atomix-labs/atomix atomix-rs --features derive,serde`.
[Working with Other Crates](working-with-other-crates.md) shows each integration
at work, and [Testing with Loom and Miri](testing-with-loom-and-miri.md) the
`loom` feature.

## What CI Checks

- `check.yml`, on `x86_64` Linux, runs every `just check` recipe: the tests; the
  codegen test, which pins what each operation lowers to on `aarch64` Linux,
  macOS and Android and `x86_64` Linux, macOS and Windows; the lints for each
  target and CPU; the loom models; and this book.
- `platforms.yml` runs the tests and the models on arm64 Linux, on macOS on
  arm64 and `x86_64`, and on Windows on arm64 and `x86_64`; and the tests at the
  CPUs a dependent may build for: `aarch64` Linux with LSE2 and at its default
  CPU, and `x86_64` at `x86-64` and `x86-64-v2`.
- `nightly.yml` runs Miri on `aarch64` Linux and macOS, on `x86_64` Linux at the
  floor and at `x86-64-v2`, and on `x86_64` Windows, each with strict
  provenance; and builds the facade, with every feature it can, for each of the
  44 targets rustup ships that the build script admits: 41 with an OS and 3 with
  none.

[facade-platforms]: api/atomix/index.html#platforms
