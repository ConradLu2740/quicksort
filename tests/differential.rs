//! 正确性门禁：差分测试。
//!
//! 铁律：任何世代改动必须先让这里的全部测试通过，才允许跑基准。
//! 判据：我们的 quicksort 输出必须与 std `sort_unstable` 逐元素一致。

use sort::inputs::{Dist, Dist::*, DISTS, Rng};
use sort::quicksort;

fn check(data: &[u32]) {
    let mut ours = data.to_vec();
    let mut theirs = data.to_vec();
    quicksort(&mut ours);
    theirs.sort_unstable();
    assert_eq!(ours, theirs, "mismatch for input of len {}", data.len());
}

fn mk(dist: Dist, n: usize, rng: &mut Rng) -> Vec<u32> {
    match dist {
        Random => (0..n).map(|_| rng.next_u32()).collect(),
        AllEqual => vec![42; n],
        FewUnique => (0..n).map(|_| rng.next_u32() % 5).collect(),
        Sorted => (0..n as u32).collect(),
        Reverse => (0..n as u32).rev().collect(),
        NearlySorted => {
            let mut v: Vec<u32> = (0..n as u32).collect();
            if n > 1 {
                let swaps = (n / 100).max(1);
                for _ in 0..swaps {
                    let i = (rng.next_u64() as usize) % n;
                    let j = (rng.next_u64() as usize) % n;
                    v.swap(i, j);
                }
            }
            v
        }
        OrganPipe => (0..n)
            .map(|i| {
                let half = if i < n / 2 { i } else { n - 1 - i };
                (half + 1) as u32
            })
            .collect(),
    }
}

#[test]
fn edge_cases() {
    check(&[]);
    check(&[1]);
    check(&[2, 1]);
    check(&[1, 2]);
    check(&[7, 7, 7]);
    check(&[3, 1, 2, 1, 3]);
}

#[test]
fn distributions_across_sizes_and_seeds() {
    let mut rng = Rng::new(1);
    for dist in DISTS {
        for &n in &[0usize, 1, 2, 3, 5, 17, 64, 100, 333, 1_000] {
            for _ in 0..20 {
                let data = mk(dist, n, &mut rng);
                check(&data);
            }
        }
    }
}

#[test]
fn randomized_against_std() {
    let mut rng = Rng::new(2);
    for _ in 0..2_000 {
        let n = (rng.next_u64() as usize) % 800;
        let data: Vec<u32> = (0..n).map(|_| rng.next_u32()).collect();
        check(&data);
    }
}
