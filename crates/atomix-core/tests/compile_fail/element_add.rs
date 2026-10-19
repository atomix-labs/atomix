//! A carry out of an element would reach the next: an element adds in place only where it is the
//! last, at the repr's top, which `last` lends.

use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

fn main() {
    let counts = Atomic::new([0_u16; 2]);
    let _ = counts.element(0).fetch_add(1, Relaxed);
}
