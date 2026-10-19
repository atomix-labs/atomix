//! A constant index past the array's end is refused where the place is built, at `cargo build`.

use atomix::Atomic;
use atomix::ordering::Acquire;

fn main() {
    let levels = Atomic::new([0_u8; 4]);
    let _ = levels.place::<4>().load(Acquire);
}
