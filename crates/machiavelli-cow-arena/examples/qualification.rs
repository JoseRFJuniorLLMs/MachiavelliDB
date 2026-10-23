//! Run explicitly on the target deployment hardware; timings are not portable.
use machiavelli_cow_arena::Arena;
use std::{sync::Arc, time::Instant};
fn main() -> std::io::Result<()> {
    let nodes = 1_000_000usize;
    let arena = Arc::new(Arena::new(&vec![0; nodes * 128])?);
    let mut latencies = Vec::new();
    for _ in 0..100 {
        let start = Instant::now();
        let branch = arena.fork()?;
        latencies.push(start.elapsed().as_nanos());
        assert_eq!(branch.bytes().len(), nodes * 128);
        drop(branch);
    }
    latencies.sort_unstable();
    println!(
        "M3 mapping benchmark: nodes={nodes}, bytes={}, median_ns={}, p99_ns={}, median_below_100us={}",
        nodes * 128,
        latencies[50],
        latencies[99],
        latencies[50] < 100_000
    );
    Ok(())
}
