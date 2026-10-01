use sort::quicksort;
use std::time::Instant;

fn timeit(name: &str, v: &mut [u32]) {
    let t = Instant::now();
    quicksort(v);
    let dt = t.elapsed();
    assert!(v.windows(2).all(|w| w[0] <= w[1]), "{name} unsorted");
    println!("{name:<22} n={:>7} took {dt:>12.3?} ({:.1} ns/elem)", v.len(), dt.as_nanos() as f64 / v.len() as f64);
}

#[test]
fn exotic_patterns_sanity() {
    let n = 100_000usize;
    // 1. 锯齿波 period-4
    let mut v: Vec<u32> = (0..n).map(|i| ((i % 4) * (n / 4) + i / 4) as u32).collect();
    timeit("sawtooth-4", &mut v);
    // 2. 双层级风琴形（organ-pipe 的 organ-pipe）
    let mut v: Vec<u32> = (0..n)
        .map(|i| {
            let q = i / (n / 4 + 1);
            let r = i % (n / 4 + 1);
            let inner = (r as u32) * 2;
            match q {
                0 | 3 => inner,
                1 | 2 => 2 * (n as u32 / 4 + 1) - inner,
                _ => 0,
            }
        })
        .collect();
    timeit("organ-pipe-x2", &mut v);
    // 3. 正弦扰动（有序 + 大幅波动）
    let mut v: Vec<u32> = (0..n)
        .map(|i| (i as f64 + (i as f64 / 97.0).sin() * (n as f64 / 8.0)) as u32)
        .collect();
    timeit("sine-perturbed", &mut v);
    // 4. 随机块交换（有序数组的块级 shuffle）
    let mut v: Vec<u32> = (0..n as u32).collect();
    let mut seed = 0xDEAD_BEEFu64;
    let mut next = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % n as u64) as usize };
    let block = n / 50;
    for _ in 0..50 {
        let (a, b) = (next(), next());
        let a = a.min(n - block);
        let b = b.min(n - block);
        for k in 0..block {
            v.swap(a + k, b + k);
        }
    }
    timeit("block-shuffled", &mut v);
}
