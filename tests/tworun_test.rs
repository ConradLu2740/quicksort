use sort::quicksort;
use std::time::Instant;

#[test]
fn two_sorted_runs_latent_quadratic() {
    let n = 100_000usize;
    let half = n / 2;
    let mut v: Vec<u32> = (0..half as u32).chain(0..half as u32).collect();
    let t = Instant::now();
    quicksort(&mut v);
    let dt = t.elapsed();
    assert!(v.windows(2).all(|w| w[0] <= w[1]));
    println!("two-runs n={n} took {dt:?} (Gen 22 基线 259ms，修复目标 < 10ms)");
    assert!(dt.as_millis() < 100, "two-run input went quadratic: {dt:?}");
}
