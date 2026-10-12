//! Three bytes leave a byte of their `u32` above them, which must stay clear: a `not` would set
//! it, so an array whose slots leave bits above has no bitwise operations, though a field of it
//! does.

use atomix_core::Atomic;
use atomix_core::ordering::Relaxed;

fn main() {
    Atomic::new([1_u8, 2, 3]).or([4, 5, 6], Relaxed);
}
