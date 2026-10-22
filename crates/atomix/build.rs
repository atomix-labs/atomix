//! Names the condition of the 16-byte atomics as atomix-core's build script does, so rustdoc
//! decides it from the flags rustc builds with.

use cfg_aliases::cfg_aliases;

/// Declares the alias, with the `check-cfg` that lets rustc know it.
fn main() {
    cfg_aliases! {
        // A 16-byte compare-exchange: aarch64's, arm64ec's, or x86_64's `cmpxchg16b`.
        wide: {
            any(
                target_arch = "aarch64",
                target_arch = "arm64ec",
                all(target_arch = "x86_64", target_feature = "cmpxchg16b")
            )
        },
    }
}
