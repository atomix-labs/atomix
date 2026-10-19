//! A wide reference is two words, whose cell exposes its pointer's provenance, and no constant
//! exposes one: a static holds a wide reference only as `None`, and takes one at run time.

use atomix_core::Atomic;

/// A venue's name, refused at the static.
static VENUE: Atomic<&'static str> = Atomic::new("XNAS");

fn main() {
    let _ = &VENUE;
}
