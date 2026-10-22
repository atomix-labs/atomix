//! The listings of the recipe Marking Links for Removal, whose list is
//! `tests/cookbook/sorted_list.rs`.

use std::thread;

use super::sorted_list::{List, Node};

#[test]
// ANCHOR: use
fn a_node_linked_after_one_on_its_way_out_is_not_lost() {
    // The arena outlives the list, which frees none of its nodes.
    let arena = [10, 20, 30, 25].map(Node::new);
    let [ten, twenty, thirty, twenty_five] = &arena;
    let list = List::new();
    for node in [thirty, ten, twenty] {
        assert!(list.insert(node), "each key goes in once");
    }
    // One thread takes 20 out as another links 25 after it: 25 must not go out with 20.
    thread::scope(|scope| {
        scope.spawn(|| assert!(list.remove(20), "20 was in"));
        scope.spawn(|| assert!(list.insert(twenty_five), "25 was not"));
    });
    let keys: Vec<u64> = [10, 20, 25, 30].into_iter().filter(|key| list.contains(*key)).collect();
    assert_eq!(keys, [10, 25, 30], "20 out, 25 in");
    assert!(!list.insert(ten), "and 10 goes in once");
}
// ANCHOR_END: use
