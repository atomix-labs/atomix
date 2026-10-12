//! A load publishes nothing, so a `Release` one would promise an order it cannot give, and
//! `core` panics on one.

use atomix::AtomicU64;
use atomix::ordering::Release;

fn main() {
    let _ = AtomicU64::new(0).load(Release);
}
