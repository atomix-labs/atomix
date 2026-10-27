//! Threads take nodes from a free list and give them back, as an allocator's callers do, through
//! one packed head; then every node is checked back, once. Prints the operations a second.
//!
//! ```text
//! cargo run --release -p atomix --features derive --example free-list
//! ```

#![cfg_attr(not(loom), expect(clippy::print_stdout, reason = "an example prints what it measured"))]
#![cfg_attr(
    not(loom),
    expect(clippy::indexing_slicing, reason = "every node the list hands out is below `NODES`")
)]

#[cfg(not(loom))]
mod free_list;

/// Loom's atomics exist only inside a model: `tests/model.rs` models this list.
#[cfg(loom)]
fn main() {}

#[cfg(not(loom))]
fn main() {
    use std::thread;
    use std::time::Instant;

    use atomix::AtomicBool;
    use atomix::ordering::Relaxed;
    use free_list::FreeList;

    /// The nodes of the arena.
    const NODES: u16 = 64;
    /// The threads that take nodes and give them back.
    const THREADS: u32 = 4;
    /// How many times each thread takes a node and gives it back.
    const ROUNDS: u32 = 1_000_000;

    let list = FreeList::new(NODES);
    // Whether each node is held: two threads that held one at once would be a bug of the list.
    let held: Box<[AtomicBool]> = (0..NODES).map(|_| AtomicBool::new(false)).collect();
    let start = Instant::now();
    thread::scope(|scope| {
        for _ in 0..THREADS {
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
    let seconds = start.elapsed().as_secs_f64();

    let mut back = vec![false; usize::from(NODES)];
    while let Some(node) = list.pop() {
        let node_back = &mut back[usize::from(node.get())];
        assert!(!*node_back, "node {node:?} comes back once");
        *node_back = true;
    }
    assert!(back.iter().all(|node_back| *node_back), "every node came back");
    let operations = f64::from(THREADS) * f64::from(ROUNDS) * 2.0;
    println!(
        "{THREADS} threads took and gave back {NODES} nodes: {:.0} operations a second",
        operations / seconds
    );
}
