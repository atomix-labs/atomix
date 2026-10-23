# Changelog

Every release, newest first, written by [git-cliff](https://git-cliff.org) from the commits.
[BREAKING-CHANGES.md](BREAKING-CHANGES.md) says how to move across a breaking change.

## [0.1.0](https://github.com/atomix-labs/atomix/releases/tag/v0.1.0) - 2026-10-23

### Features

- [eda1439](https://github.com/atomix-labs/atomix/commit/eda1439735a9ee4818c11a920df7f6f3e37cc5a9) Store every value that fits one atomic: references, function pointers, arrays, tuples, and core's and std's values ([#37](https://github.com/atomix-labs/atomix/pull/37))
- [4b25246](https://github.com/atomix-labs/atomix/commit/4b25246b9c926676e700bfac6ef1a5ed7a9c743b) *(atomix-core)* Give each type the impls and conversions std's counterpart has ([#35](https://github.com/atomix-labs/atomix/pull/35))
- [a45ee1a](https://github.com/atomix-labs/atomix/commit/a45ee1a3e63a58b846b1d3c2be735861f4f2ee71) *(atomix-core)* Give the ranged integers NonZero's const impls, and the exponent formats ([#34](https://github.com/atomix-labs/atomix/pull/34))
- [3f7c448](https://github.com/atomix-labs/atomix/commit/3f7c4488a28687db251d46368e9425227d943c9f) *(atomix-core)* Make the atomic operations const where core's are ([#32](https://github.com/atomix-labs/atomix/pull/32))
- [8ced0cd](https://github.com/atomix-labs/atomix/commit/8ced0cd6568e26fe375c1a27bb856d5815ca6c83) *(atomix-core)* Build for arm64ec ([#27](https://github.com/atomix-labs/atomix/pull/27))
- [0754ed2](https://github.com/atomix-labs/atomix/commit/0754ed2c4fe4dc97216c5dceaecd2bb233587652) *(atomix-derive)* Derive a packed struct whose parameters reach markers alone ([#25](https://github.com/atomix-labs/atomix/pull/25))
- [2ee06fe](https://github.com/atomix-labs/atomix/commit/2ee06fe1427f5b4f2804b4af9cd8d82f2fba9415) Refuse an atomic or a cell as a value, or a newtype over a non-atom, once, and name the fix for a full Option or a pointer's bits ([#23](https://github.com/atomix-labs/atomix/pull/23))
- [fbb890e](https://github.com/atomix-labs/atomix/commit/fbb890e43f980228910bc96c6568edf07d400b86) *(atomix-core)* Take an atomic from a mutable reference, and from a slice of them ([#22](https://github.com/atomix-labs/atomix/pull/22))
- [842d068](https://github.com/atomix-labs/atomix/commit/842d0686572f3d94edb94b0a30a6b054a5a7e175) *(atomix-core)* Set, clear and toggle a bit chosen at run time ([#21](https://github.com/atomix-labs/atomix/pull/21))
- [7c40829](https://github.com/atomix-labs/atomix/commit/7c40829319c20807b9d53ed70fa905dacd178e5c) *(atomix-core)* Refuse a target that is not aarch64 or x86_64, little-endian with 64-bit pointers ([#19](https://github.com/atomix-labs/atomix/pull/19))
- [6985507](https://github.com/atomix-labs/atomix/commit/6985507c64cc0dc0780cf5d58becbf5b9a0b31d7) *(atomiks)* Hold two words, each a pointer or an integer, in one atomic ([#13](https://github.com/atomix-labs/atomix/pull/13))
- [d2721ab](https://github.com/atomix-labs/atomix/commit/d2721ab51283455a6e4cbe558f26eb520f1749ba) *(atomiks)* Keep a pointer's tags in its low bits, in a struct or an enum ([#12](https://github.com/atomix-labs/atomix/pull/12))
- [9adeadf](https://github.com/atomix-labs/atomix/commit/9adeadfa653c14957264275383d33530d0b010db) *(atomiks)* Change one field of an atomic packed struct with one instruction ([#10](https://github.com/atomix-labs/atomix/pull/10))
- [945e172](https://github.com/atomix-labs/atomix/commit/945e1721b6e588d8a5e7bfe2afe6e854420f5d0b) *(atomiks-core)* Integrate serde, zerocopy, bytemuck, arbitrary, arbitrary-int and deranged ([#7](https://github.com/atomix-labs/atomix/pull/7))
- [0f0971d](https://github.com/atomix-labs/atomix/commit/0f0971dc3074ea172eecd35dc5986573c6d427fd) *(atomiks-core)* Add the ranged integers, held to a range, and store them in an atomic ([#5](https://github.com/atomix-labs/atomix/pull/5))
- [4e5e795](https://github.com/atomix-labs/atomix/commit/4e5e7954bbdeb0512941b9bd796f8fc3bf0f18bb) *(atomiks)* Derive Atom, and AtomAdd, AtomOrd and AtomBitwise on newtypes ([#4](https://github.com/atomix-labs/atomix/pull/4))
- [bfd5076](https://github.com/atomix-labs/atomix/commit/bfd5076a4d420a4f7249109d4b38be3941b3eb48) *(atomiks-core)* Describe each value's reprs by a range that may wrap through zero ([#3](https://github.com/atomix-labs/atomix/pull/3))
- [55abf89](https://github.com/atomix-labs/atomix/commit/55abf8966b90f2421fb648f10828b4768b682390) Add atomiks-core, typed atomics for any value that fits one atomic word, and the atomiks facade ([#2](https://github.com/atomix-labs/atomix/pull/2))

### Bug Fixes

- [292056a](https://github.com/atomix-labs/atomix/commit/292056ac11c8ef38a2942edbcb896a66d49ca41a) Report a misaligned tagged pointer where the caller encoded it ([#33](https://github.com/atomix-labs/atomix/pull/33))
- [a944270](https://github.com/atomix-labs/atomix/commit/a9442702ccad67b568d7f4a6fe4a92fa255e03bd) *(atomix-core)* Keep x86_64's lock bts wherever a bit test's result goes ([#26](https://github.com/atomix-labs/atomix/pull/26))
- [b3cdfd8](https://github.com/atomix-labs/atomix/commit/b3cdfd87585dcdec58dd57d472ce39061e2d4002) *(atomix-derive)* Silence a deprecated type's derive, and warn where a deprecated field's place is used ([#24](https://github.com/atomix-labs/atomix/pull/24))

### Performance

- [e15af2c](https://github.com/atomix-labs/atomix/commit/e15af2c4b803c068266e454253b81ab189d3d2e7) *(atomix-core)* Decode an Ordering by a copy, so its fetch_max masks nothing first ([#39](https://github.com/atomix-labs/atomix/pull/39))
- [e57382f](https://github.com/atomix-labs/atomix/commit/e57382f876d2f04181dfe1642ed9f34e21b25055) *(atomix-core)* Build an element's fetch_and operand with one shift ([#38](https://github.com/atomix-labs/atomix/pull/38))

### Refactor

- [1eb154f](https://github.com/atomix-labs/atomix/commit/1eb154f5e2e31a9083821ffe112f8117e61c6c49) Spell PtrAtom, NONZERO and atom(ptr) in whole words ([#31](https://github.com/atomix-labs/atomix/pull/31))
- [f281ad0](https://github.com/atomix-labs/atomix/commit/f281ad0a11f8c9adb4ce62963669388660435398) Rename atomiks to atomix, published as atomix-rs ([#16](https://github.com/atomix-labs/atomix/pull/16))
- [8a8409e](https://github.com/atomix-labs/atomix/commit/8a8409ec19fdd93c79f21b6ca6d1afc4b7ffa7ab) *(atomiks-core)* Name each read-modify-write as core does, with a short form only where x86 needs one ([#9](https://github.com/atomix-labs/atomix/pull/9))

### Documentation

- [3074b31](https://github.com/atomix-labs/atomix/commit/3074b3102635766410f4ec0abbd258bc0bccddb0) Document every target on docs.rs at the CPU floor, each badge in words ([#42](https://github.com/atomix-labs/atomix/pull/42))
- [6c94511](https://github.com/atomix-labs/atomix/commit/6c9451195fd22ece5ed7f232cd570459ef854da8) Write the book and its examples, and measure atomix against core and the crates beside it ([#41](https://github.com/atomix-labs/atomix/pull/41))
- [32fdbed](https://github.com/atomix-labs/atomix/commit/32fdbed9c596304582aa5ee75545d0c6d5170ada) *(atomix-core)* Say a tuple keeps each field in a slot as wide as its repr ([#40](https://github.com/atomix-labs/atomix/pull/40))
- [c003380](https://github.com/atomix-labs/atomix/commit/c00338049ad6efe0fa02a08559b610ebfc3f54a2) Present atomix with its logo, its README and each crate's ([#17](https://github.com/atomix-labs/atomix/pull/17))

### CI

- [041e254](https://github.com/atomix-labs/atomix/commit/041e254748de0eea7adb74d939219c9097e715e5) Release from a signed tag, and publish each crate to crates.io by trusted publishing ([#45](https://github.com/atomix-labs/atomix/pull/45))
- [2f0378d](https://github.com/atomix-labs/atomix/commit/2f0378d6e5cd296e2b2c14709d2b7637ff4392b2) Build each crate's docs as docs.rs does, each night, on its nightly ([#43](https://github.com/atomix-labs/atomix/pull/43))
- [6549ad9](https://github.com/atomix-labs/atomix/commit/6549ad9e6de0bc6b2b52a2fc5b9ed0a9a675873d) Build every aarch64 and x86_64 target each night, and run Miri on Windows ([#29](https://github.com/atomix-labs/atomix/pull/29))
- [13329f0](https://github.com/atomix-labs/atomix/commit/13329f0a2f68c77c215566d06972e73c767e35b5) Test on Windows, aarch64 and x86_64 ([#28](https://github.com/atomix-labs/atomix/pull/28))
- [6ffa917](https://github.com/atomix-labs/atomix/commit/6ffa917878f57d3a8063037b3361909a27085bfa) Test LSE2, aarch64 without LSE, and x86-64 v1 and v2 on their own legs ([#20](https://github.com/atomix-labs/atomix/pull/20))
- [5ebe976](https://github.com/atomix-labs/atomix/commit/5ebe9760543b2986ee71adc45a75f7433e8bdc53) Run Miri on each target as a nightly job of its own ([#11](https://github.com/atomix-labs/atomix/pull/11))

### Miscellaneous

- [845a575](https://github.com/atomix-labs/atomix/commit/845a57527d3ee8102e81f1706915dfbd40be574b) Ready each crate's package and its crates.io page for the first release ([#44](https://github.com/atomix-labs/atomix/pull/44))
- [d7da37e](https://github.com/atomix-labs/atomix/commit/d7da37e3193444272f362cb42ef13feac30ad8d3) *(atomix-core)* Pin the default CPUs of Android and Windows ([#30](https://github.com/atomix-labs/atomix/pull/30))
- [27e1dde](https://github.com/atomix-labs/atomix/commit/27e1dde1ebc380849fadbd49b7faad64f6443dcd) Take atxp v0.22.0, whose doc checks keep the CPU floor ([#18](https://github.com/atomix-labs/atomix/pull/18))
- [a74bf84](https://github.com/atomix-labs/atomix/commit/a74bf84176851bfdfac60daf8c3116be8cdab908) Take atxp v0.21.2, whose doc check knows a crate by its library's name ([#15](https://github.com/atomix-labs/atomix/pull/15))
- [c9e6ad8](https://github.com/atomix-labs/atomix/commit/c9e6ad883350483facfea8ee49694773fc8ddfc1) Take atxp v0.21.1, which runs the doctests with no target named ([#14](https://github.com/atomix-labs/atomix/pull/14))
- [326de45](https://github.com/atomix-labs/atomix/commit/326de45fe90fdd68ee671e9bff0b3a16eb0ae2b7) Take atxp v0.21.0, and check every feature's dependencies ([#6](https://github.com/atomix-labs/atomix/pull/6))
