# Performance

An operation of atomix costs the instruction its name promises, and nothing
around it. Two things hold it to that: `crates/atomix-core/tests/codegen.rs`
pins what each operation lowers to on every target, and holds it to core's own
code, instruction for instruction; and the benches time it against core and its
peers, portable-atomic, crossbeam-utils' `AtomicCell`, atomic, atomig and
atomic_enum.

Every figure on this page is from a run committed under
`crates/atomix/benches/results/`: a directory of two runs' transcripts at each
of two builds, and a `manifest.toml` that says how they were taken. Each run was
taken on an AWS Graviton4 (Arm Neoverse V2, a c8g.8xlarge) under Linux 6.12.53,
built by rustc 1.101.0-nightly of 2026-09-27 in the bench profile (release's
settings, fat LTO, one codegen unit), pinned to cores the kernel keeps other
work off; once at the workspace's floor, `-C target-feature=+crc,+lse`, and once
with `-C target-cpu=neoverse-v2`, which has LSE2, as
[The Flags for Full Speed](platforms-and-features.md#the-flags-for-full-speed)
sets out. Each build ran twice. An `operations` figure is the median of 100
samples, each pair the two runs' medians; a `contention` figure is one thread's
median of 5,000 rounds, as [Under Contention](#under-contention) says.

| Run                                    | What it compares                                                                |
| -------------------------------------- | ------------------------------------------------------------------------------- |
| `2026-10-12T17-47Z-4b25246-operations` | one operation on one thread, against core, its peers, and the same bits by hand |
| `2026-10-12T17-47Z-4b25246-contention` | one operation on 1, 2, 4 and 8 threads, each pinned to a core of its own        |
| `2026-10-19T08-41Z-423b1e5-values`     | an array's elements, four bytes and a shared reference, on one thread           |

The name is the time the run was taken, in UTC, the commit it measured, and what
it measured. No `x86_64` run is committed yet: one waits for an `x86_64` host,
and the figures below are `aarch64`'s alone.

## What Each Operation Lowers To

`tests/codegen.rs` builds a fixture of one function per operation for each
target
[The Same Code as Core](what-each-operation-costs.md#the-same-code-as-core)
names, and reads its assembly: each function must be the instructions its row
names, with no loop but `update`'s, and no call but to a cold refusal off its
fast path: a tagged pointer's of a misaligned pointer, or an element's of an
index past the end. Where core has the operation too, a twin written over core's
atomics must be the same code, but in the places that section names. For the
operations a hot path takes:

| Operation, ordering                     | `aarch64`, `+lse` | `aarch64`, LSE2    | `x86_64`, `x86-64-v3`     |
| --------------------------------------- | ----------------- | ------------------ | ------------------------- |
| `load`, `Acquire`                       | `ldar`            | `ldapr`            | `movq`                    |
| `store`, `Release`                      | `stlr`            | `stlr`             | `movq`                    |
| `swap`, `AcqRel`                        | `swpal`           | `swpal`            | `xchgq`                   |
| `fetch_add`, `AcqRel`                   | `ldaddal`         | `ldaddal`          | `lock xaddq`              |
| `compare_exchange`, `AcqRel`            | `casal`           | `casal`            | `lock cmpxchgq`           |
| `or`, `Release`                         | `ldsetl`          | `ldsetl`           | `lock orq`                |
| `fetch_or`, `AcqRel`                    | `ldsetal`         | `ldsetal`          | does not compile          |
| `fetch_max`, `AcqRel`                   | `ldumaxal`        | `ldumaxal`         | does not compile          |
| `bit_set` at a run-time bit, `AcqRel`   | `ldsetal`, `tst`  | `ldsetal`, `tst`   | `lock btsq`, `setb`       |
| a field's `set`, `Release`              | `ldsetl`          | `ldsetl`           | `lock orq`                |
| a field's `test_and_set`, `AcqRel`      | `ldsetal`, `ubfx` | `ldsetal`, `ubfx`  | `lock btsq`, `setb`       |
| a tagged pointer's `store`, `Release`   | `tbnz`, `stlr`    | `tbnz`, `stlr`     | `testb`, `jne`, `movq`    |
| two words' `load`, `Acquire`            | does not compile  | `ldp`, `dmb ishld` | `vmovdqa`                 |
| two words' `load_rmw`, `Acquire`        | `caspa`           | `caspa`            | `lock cmpxchg16b`         |
| two words' `compare_exchange`, `AcqRel` | `caspal`          | `caspal`           | `lock cmpxchg16b`         |
| a field's `update`, `AcqRel`            | a loop of `casal` | a loop of `casal`  | a loop of `lock cmpxchgq` |

The table is read from `tests/codegen.rs`, which a change to atomix that changes
a row fails.

## Against Core and Its Peers

On one thread, in nanoseconds, at the floor. The LSE2 build's figures, in the
manifest, differ in the same ways but one: a tagged pointer's load, alike at the
floor, is 0.3 ns slower in atomix with LSE2, for the order its result is
written, which the manifest's notes show. A `u64`, each in the same orderings;
crossbeam-utils' `AtomicCell` fixes its own, `Acquire` loads, `Release` stores
and `AcqRel` for the rest:

| Operation          | core         | atomix       | portable-atomic | `AtomicCell` | atomic       | atomig       |
| ------------------ | ------------ | ------------ | --------------- | ------------ | ------------ | ------------ |
| `load`             | 0.759, 0.759 | 0.759, 0.759 | 0.759, 0.759    | 0.759, 0.759 | 0.758, 0.759 | 0.759, 0.758 |
| `store`            | 2.008, 2.007 | 2.007, 2.008 | 2.011, 2.007    | 2.008, 2.007 | 3.206, 2.007 | 2.024, 2.009 |
| `swap`             | 7.757, 7.714 | 7.760, 7.746 | 7.749, 7.775    | 7.758, 7.477 | 7.750, 7.749 | 7.756, 7.442 |
| `fetch_add`        | 7.750, 7.753 | 7.748, 7.758 | 7.527, 7.755    | 7.787, 7.757 | 7.755, 7.763 | 7.766, 7.766 |
| `compare_exchange` | 7.168, 7.202 | 7.175, 7.201 | 7.170, 7.205    | 7.992, 7.980 | 7.168, 7.170 | 7.204, 7.203 |

Each crate's `u64` is the same instruction, and a store's 2.0 or 3.2 ns, or a
read-modify-write's spread of 0.8 ns, moves from run to run in every crate
alike. `AtomicCell`'s compare-exchange is a loop of core's weak one, 0.6 to 0.8
ns slower.

A fieldless enum's load, and a struct of a price, a quantity and a side, whole:

| Operation                     | core, by hand | atomix       | `AtomicCell` | atomic       | atomig       | atomic_enum  |
| ----------------------------- | ------------- | ------------ | ------------ | ------------ | ------------ | ------------ |
| an enum's `load`              | 0.780, 0.780  | 0.759, 0.758 |              |              | 0.818, 0.818 | 0.774, 0.778 |
| a struct's `load`             | 0.751, 0.757  | 0.750, 0.751 | 21.07, 20.85 | 8.361, 8.361 |              |              |
| a struct's `store`            | 2.032, 2.118  | 2.046, 3.207 | 19.63, 19.64 | 8.676, 8.676 |              |              |
| a struct's `compare_exchange` | 8.111, 8.113  | 8.105, 8.107 | 13.63, 13.62 | 11.05, 11.05 |              |              |

atomix's enum load is a bare `ldarb`; core's by hand compares and selects, and
atomig and atomic_enum each compare and branch to a panic. The struct is 8 bytes
aligned to 4, so every operation of `AtomicCell` goes through its striped
seqlock, a load reading optimistically and locking only where a write overlaps
it, and every operation of atomic, a load among them, takes its spinlock, where
atomix holds the struct in one `u64`.

One field of a packed struct, against the same bits written by hand over core's
`AtomicU64` and against a compare-exchange loop on the whole word, the way a
crate that holds the struct whole changes a field:

| Operation            | by hand      | atomix       | a loop       |
| -------------------- | ------------ | ------------ | ------------ |
| a `bool`'s `set`     | 6.369, 6.369 | 6.369, 6.371 | 8.678, 8.656 |
| its `test_and_clear` | 6.531, 6.531 | 6.533, 6.534 | 7.874, 8.137 |
| a byte's `or`        | 8.031, 8.193 | 8.008, 8.197 | 10.41, 10.41 |
| the top field's add  | 6.932, 6.866 | 6.896, 6.869 | 8.637, 8.653 |

LLVM merged each by-hand function with atomix's, so the two rows call one
function: the same code, as the codegen twins say on every target. The loop
costs 1.3 to 2.4 ns more on one thread, and retries under contention, where one
instruction does not.

A Treiber stack's head, a pointer beside tags in its low bits, against
`AtomicPtr` and masks by hand:

| Operation              | by hand      | atomix       |
| ---------------------- | ------------ | ------------ |
| `store`                | 2.049, 2.057 | 2.167, 2.153 |
| `compare_exchange`     | 8.111, 8.117 | 8.379, 8.386 |
| a tag's `test_and_set` | 6.535, 6.531 | 6.529, 6.535 |

The store pays the one test and branch [Tagged Pointers](tagged-pointers.md)
describes, 0.10 to 0.13 ns, and the exchange tests both its pointers in one,
0.27 to 0.30 ns; a by-hand store that skipped the test would write a misaligned
pointer's bits into its tags.

Two words, a node's pointer beside a `u64` version, against portable-atomic's
`AtomicU128` and `AtomicCell`'s lock, in each build:

| Operation          | floor: atomix | portable-atomic | `AtomicCell` | LSE2: atomix | portable-atomic |
| ------------------ | ------------- | --------------- | ------------ | ------------ | --------------- |
| `compare_exchange` | 8.587, 8.570  | 8.582, 8.576    | 13.26, 13.20 | 8.939, 8.912 | 8.940, 8.859    |
| a read             | 5.029, 5.030  | 5.810, 5.205    | 26.59, 26.14 | 6.174, 6.143 | 6.170, 6.325    |
| a write            | 12.00, 11.99  | 5.090, 5.090    | 19.78, 19.37 | 5.738, 5.705 | 5.262, 5.227    |

At the floor, atomix's read and write are `load_rmw` and `store_rmw`, a
compare-exchange and a loop of them; portable-atomic reads the CPU's features at
run time and calls `ldp` and `stp` on this one. A build that names a CPU with
LSE2, as
[Platforms and Features](platforms-and-features.md#the-flags-for-full-speed)
says, gives atomix `load` and `store`, `ldp` and `stp` too: its reads then cost
what portable-atomic's do, and the bench's timed loop of its write is
portable-atomic's, byte for byte, `dmb ish` then `stp`, so the 0.5 ns between
the two columns is the harness's, not the code's. On one thread, the `ldp` read
is no faster than `load_rmw`'s `caspa`; under contention it is.

An array's element at an index known at run time, against core's `AtomicU64`
masked by hand after the same check, and atomix's `bit_set`, which masks its
index rather than checking it; four levels' depths, `[u8; 4]`, against
`AtomicU32`; and a shared reference against `AtomicPtr`, from the values' run,
at the floor:

| Operation                                      | by hand, or core | atomix       | `bit_set`    |
| ---------------------------------------------- | ---------------- | ------------ | ------------ |
| a `[bool; 64]` element's `test_and_set`        | 7.524, 7.334     | 7.477, 7.295 | 7.314, 7.001 |
| its `clear`                                    | 6.792, 6.794     | 6.796, 6.795 | 6.729, 6.744 |
| a `[Slot; 4]` element's field's `test_and_set` | 7.647, 7.265     | 7.463, 7.351 |              |
| its other field's `load`                       | 0.985, 0.985     | 0.983, 0.988 |              |
| `[u8; 4]`'s `load`                             | 0.779, 0.780     | 0.780, 0.780 |              |
| its `store`                                    | 2.053, 2.036     | 1.978, 1.997 |              |
| a reference's `load`                           | 0.778, 0.779     | 0.778, 0.780 |              |
| its `store`                                    | 3.205, 1.979     | 2.081, 1.995 |              |
| its `swap`                                     | 7.737, 7.754     | 7.933, 8.339 |              |

An element's fast path is the mask by hand's, instruction for instruction: `cmp;
b.hs`, then `mov; lsl; ldsetal; tst; cset`, the two parting only in the cold
call. `bit_set` checks nothing, a compare and a branch shorter, which the
medians do not tell apart. LLVM merged `[u8; 4]`'s functions with `AtomicU32`'s,
and a reference's store with `AtomicPtr`'s, so those rows call one function and
differ by the harness alone; a store is 2.0 or 3.2 ns, and a swap moves by up to
0.4 ns from run to run in either, and 0.6 between them.

## Under Contention

In nanoseconds an operation, the fastest and the slowest thread's medians, each
thread on a core of its own, from the first run at the floor, and the 16-byte
`load` from the first with LSE2, which it needs. The second runs, in the
manifest, move by up to a fifth from two threads on:

| Case                                     | 1 thread | 2 threads      | 4 threads      | 8 threads       |
| ---------------------------------------- | -------- | -------------- | -------------- | --------------- |
| one shared count's `fetch_add`, atomix   | 6.82     | 22.55          | 38.09 to 48.46 | 79.92 to 103.94 |
| the same, core                           | 6.82     | 22.52          | 37.95 to 49.41 | 79.89 to 104.32 |
| a count each, side by side               | 5.20     | 15.25          | 30.64 to 32.71 | 59.54 to 70.71  |
| a count each, in `CachePadded`           | 5.21     | 5.21           | 5.21           | 5.21            |
| a 16-byte read, `load_rmw` (`caspa`)     | 5.30     | 16.38          | 34.55 to 34.67 | 61.42 to 77.44  |
| a 16-byte read, `load` (`ldp`), LSE2     | 4.10     | 4.10           | 4.10           | 4.10            |
| a claim of one bit and its clear, atomix | 11.91    | 30.04 to 30.53 | 53.32 to 61.39 | 91.92 to 109.54 |
| the same, core by hand                   | 11.91    | 30.00 to 30.15 | 52.99 to 61.58 | 93.83 to 107.89 |

Each pair compared, atomix's and core's, shares one run and one cache line, and
the two move together from run to run. Counts side by side, each added to by one
thread, `Relaxed`, still share a line that every add takes in turn: 15.25 ns at
two threads, where each in a `CachePadded` line of its own stays at 5.21;
[Values and Places](values-and-places.md) pads them so. A `load` lets readers
share the line, where `load_rmw` takes it from every other reader:
[Two Words](two-words.md) says when to want LSE2 for it.

## Measuring on Your Own Machine

The benches are divan's `operations`, on one thread, and `contention`, a harness
of its own that pins each thread to a core:

```sh
cargo bench -p atomix-rs --features derive --bench operations
cargo bench -p atomix-rs --features derive --bench contention
```

A name after `--` runs the `operations` benches it matches, as `-- fields`. For
figures that hold still, build first, so the run times no compilation, then pin
the run to cores the kernel keeps other work off, ones that `isolcpus` and
`nohz_full` name on the kernel's command line, and run it twice. `contention`
pins its threads to the cores of its mask, eight or more:

```sh
cargo bench -p atomix-rs --features derive --bench contention --no-run
taskset -c 20-27 cargo bench -p atomix-rs --features derive --bench contention
```

The CPU flags change what a build has, so a run names its own: `RUSTFLAGS='-C
target-cpu=native'` replaces the workspace's floor with the machine's. A run
worth keeping goes under `crates/atomix/benches/results/`, in a directory named
for its time in UTC, its commit and what it measures, with its transcripts and a
`manifest.toml` in the shape of those there: the hardware, the environment, the
toolchain, the parameters, notes on what the disassembly showed, and the
results.

The lowering tables need no machine of a kind: on any host with the targets
installed, `cargo test -p atomix-core --test codegen` builds the fixture for
each target and checks every row.
