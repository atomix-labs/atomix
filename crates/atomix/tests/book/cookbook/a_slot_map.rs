//! The listings of the recipe A Slot Map, whose map is `tests/cookbook/slot_map.rs`, over the
//! free list of the free-list example.

use std::thread;

use super::ROUNDS;
use super::slot_map::SlotMap;

#[test]
// ANCHOR: use
fn a_key_outlives_its_value_harmlessly() {
    let map = SlotMap::new(1);
    let first = map.insert(7).expect("a slot is free");
    assert_eq!(map.insert(8), None, "and no other");
    assert_eq!(map.remove(first), Some(7), "the value removed");
    assert_eq!(map.remove(first), None, "once");

    // The slot is free again, and its next value takes a new generation.
    let second = map.insert(9).expect("the slot is free again");
    assert_eq!(map.get(first), None, "so the first key finds nothing");
    assert_eq!(map.get(second), Some(9), "and the second its value");
}
// ANCHOR_END: use

#[test]
fn a_stale_key_never_finds_a_later_value() {
    let map = SlotMap::new(2);
    thread::scope(|scope| {
        for value in 0..4 {
            let map = &map;
            scope.spawn(move || {
                for _ in 0..ROUNDS {
                    let Some(key) = map.insert(value) else { continue };
                    assert_eq!(map.get(key), Some(value), "a key finds its own value");
                    assert_eq!(map.remove(key), Some(value), "and removes it");
                    assert_eq!(map.get(key), None, "and nothing after");
                }
            });
        }
    });
}
