//! The listings of the recipe A Bitmap Allocator, whose bitmap is
//! `tests/cookbook/bitmap_allocator.rs`.

use std::collections::HashSet;
use std::sync::Mutex;
use std::thread;

use super::ROUNDS;
use super::bitmap_allocator::Bitmap;

#[test]
// ANCHOR: use
fn each_slot_is_taken_by_one_thread_at_a_time() {
    let bitmap = Bitmap::<2>::new();
    // The slots threads hold: a slot two held at once would be a bug of the bitmap.
    let held = Mutex::new(HashSet::new());
    thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..ROUNDS {
                    let slot = bitmap.allocate().expect("4 threads, 128 slots");
                    assert!(
                        held.lock().expect("no thread panics").insert(slot),
                        "{slot:?} is free"
                    );
                    held.lock().expect("no thread panics").remove(&slot);
                    bitmap.free(slot);
                }
            });
        }
    });
    let all: HashSet<_> =
        (0..128).map(|_| bitmap.allocate().expect("every slot is free")).collect();
    assert_eq!(all.len(), 128, "each slot taken once");
    assert_eq!(bitmap.allocate(), None, "and no other");
}
// ANCHOR_END: use
