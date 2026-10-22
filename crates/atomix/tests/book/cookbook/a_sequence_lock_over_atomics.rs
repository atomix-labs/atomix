//! The listings of the recipe A Sequence Lock Over Atomics, whose lock is
//! `tests/cookbook/sequence_lock.rs`.

use std::thread;

use super::ROUNDS;
use super::sequence_lock::SequenceLock;

#[test]
// ANCHOR: use
fn a_reader_reads_the_words_a_write_left_whole() {
    // Two words whose second is always twice the first: a read of one write's first word beside
    // another's second would break that.
    let lock = SequenceLock::new([0, 0]);
    thread::scope(|scope| {
        for writer in 1..3 {
            let lock = &lock;
            scope.spawn(move || {
                for round in 0..ROUNDS {
                    let first = writer * 1_000_000 + round;
                    lock.write([first, first * 2]);
                }
            });
        }
        scope.spawn(|| {
            for _ in 0..ROUNDS {
                let [first, second] = lock.read();
                assert_eq!(second, first * 2, "both words from one write");
            }
        });
    });
}
// ANCHOR_END: use
