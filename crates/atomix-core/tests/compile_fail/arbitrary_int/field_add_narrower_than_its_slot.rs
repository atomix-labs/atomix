//! A `u7` takes seven bits of its byte slot, so its carry would land in the slot's top bit, which
//! no `u7` sets: a tuple's last field adds in place only where every repr of its slot decodes, as a
//! `u8`'s does.

use arbitrary_int::u7;
use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

fn main() {
    let filled = Atomic::new((0_u8, 0_u8));
    let _ = filled.fields().1.fetch_add(1, Relaxed);
    let narrow = Atomic::new((0_u8, u7::new(127)));
    let _ = narrow.fields().1.fetch_add(1, Relaxed);
}
