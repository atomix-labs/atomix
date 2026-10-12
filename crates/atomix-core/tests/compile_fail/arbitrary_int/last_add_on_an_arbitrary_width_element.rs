//! A `u4` fills half of its byte slot, so its carry would land in the slot's other half, which must
//! stay clear: the last element adds in place only where every repr of it decodes, `AtomAdd`.

use arbitrary_int::u4;
use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

fn main() {
    let nibbles = Atomic::new([u4::new(0); 2]);
    let _ = nibbles.last().fetch_add(1, Relaxed);
}
