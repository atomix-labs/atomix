//! The listings of the recipe A Free List, whose list is the free-list example's.

use std::thread;

use atomix::AtomicBool;
use atomix::ordering::Relaxed;

use super::ROUNDS;
use super::free_list::{FreeList, Index};

#[test]
// ANCHOR: use
fn a_node_taken_is_held_by_one_thread_until_given_back() {
    let list = FreeList::new(4);
    let held: Vec<AtomicBool> = (0..4).map(|_| AtomicBool::new(false)).collect();
    thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..ROUNDS {
                    let Some(node) = list.pop() else { continue };
                    let node_held = &held[usize::from(node.get())];
                    // ORDERING: Relaxed: the flag publishes only itself; the list's pop and push
                    // order each node between its holders.
                    assert!(!node_held.swap(true, Relaxed), "node {node:?} is held by one thread");
                    node_held.store(false, Relaxed);
                    list.push(node);
                }
            });
        }
    });
    let mut back: Vec<u16> = (0..4).filter_map(|_| list.pop()).map(Index::get).collect();
    back.sort_unstable();
    assert_eq!(back, [0, 1, 2, 3], "every node came back, once");
    assert_eq!(list.pop(), None, "and no other");
}
// ANCHOR_END: use
