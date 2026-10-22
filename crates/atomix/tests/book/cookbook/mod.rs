//! The listings of the Cookbook: each recipe's structure, taken from its example or from
//! `tests/cookbook/`, as the loom models in `tests/model.rs` take it, and in the module named for
//! its page, the tests that run it on threads.

mod a_bitmap_allocator;
mod a_free_list;
mod a_lock_word;
mod a_sequence_lock_over_atomics;
mod a_slot_map;
mod marking_links_for_removal;

#[path = "../../cookbook/bitmap_allocator.rs"]
mod bitmap_allocator;
#[path = "../../../examples/free-list/free_list.rs"]
mod free_list;
#[path = "../../../examples/lock-word/lock_word.rs"]
mod lock_word;
#[path = "../../cookbook/sequence_lock.rs"]
mod sequence_lock;
#[path = "../../cookbook/slot_map.rs"]
mod slot_map;
#[path = "../../cookbook/sorted_list.rs"]
mod sorted_list;

/// How many rounds each thread runs: Miri interprets each, so it runs few.
const ROUNDS: u64 = if cfg!(miri) { 20 } else { 10_000 };
