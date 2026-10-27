# Breaking Changes

A migration note for every breaking change, newest release first: what changed,
why, and what a user does about it.

[CHANGELOG.md](CHANGELOG.md) lists every change; this lists only those a user
must act on.

## 0.2.0

### The Facade Is Published as `atomix`

The facade, published as `atomix-rs` up to 0.1.0, is the package `atomix` from
0.2.0, since crates.io freed the name an unrelated placeholder held. `atomix-rs`
has no further release.

Its library was `atomix` already, so code is unchanged. A dependency names the
new package, with the same features:

```toml
[dependencies]
atomix = { version = "0.2", features = ["derive"] }
```

in place of `atomix-rs = { version = "0.1", … }`, and `--features
atomix-rs/loom` is `--features atomix/loom`. A crate that depends on
`atomix-core` or `atomix-derive` alone moves to 0.2 with no change to its code.
