//! Three halves leave sixteen bits of their `u64` above them, which must stay clear: the last
//! element's carry would set them, so it adds in place only where the slots fill the repr.

use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

fn main() {
    let counts = Atomic::new([0_u16; 3]);
    let _ = counts.last().fetch_add(1, Relaxed);
}
