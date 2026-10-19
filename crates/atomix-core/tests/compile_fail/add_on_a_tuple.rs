//! A tuple has no add: a carry out of one field would land in the next, so `fetch_add` needs
//! `AtomAdd`, which a tuple lacks. Its last field adds alone, where the slots fill the repr.

use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

fn main() {
    let counts = Atomic::new((1_u16, 2_u16));
    let _ = counts.fields().1.fetch_add(1, Relaxed);
    let _ = counts.fetch_add(1 << 16, Relaxed);
}
