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

#[test]
fn large_scale_differential() {
    // 大规模差分（Gen 30 新增）：此前覆盖率止步 800 元素，
    // 大 n 下的病态（深层递归、深度预算路径、叶子行为）只有规模能触发。
    const N: usize = 20_000;
    let mut rng = Rng::new(0xA11CE);
    let names = ["random", "few-unique", "sorted", "reverse", "nearly-sorted", "organ-pipe"];
    for name in names {
        let data: Vec<u32> = match name {
            "random" => (0..N).map(|_| rng.next_u32()).collect(),
            "few-unique" => (0..N).map(|_| rng.next_u32() % 3).collect(),
            "sorted" => (0..N as u32).collect(),
            "reverse" => (0..N as u32).rev().collect(),
            "nearly-sorted" => {
                let mut v: Vec<u32> = (0..N as u32).collect();
                for _ in 0..N / 100 {
                    let i = (rng.next_u64() as usize) % N;
                    let j = (rng.next_u64() as usize) % N;
                    v.swap(i, j);
                }
                v
            }
            "organ-pipe" => (0..N)
                .map(|i| {
                    let half = if i < N / 2 { i } else { N - 1 - i };
                    (half + 1) as u32
                })
                .collect(),
            _ => unreachable!(),
        };
        check(&data);
    }
}

#[test]
fn generic_type_paths() {
    // 泛型路径覆盖（Gen 30 新增）：i64（负值域）与 String（非 Copy、含 Drop、
    // 比较非平凡）—— 此前全部测试只走 u32 单态化。
    let mut v: Vec<i64> = vec![-5, 3, 0, -7, 3, i64::MIN, i64::MAX, -1, 0];
    v.sort_unstable();
    let mut ours = v.clone();
    // 打乱（固定动作）后排序
    ours.reverse();
    quicksort(&mut ours);
    assert_eq!(ours, v);

    let mut words = vec![
        "banana".to_string(),
        "apple".to_string(),
        "cherry".to_string(),
        "apple".to_string(),
        "date".to_string(),
    ];
    let mut expect = words.clone();
    expect.sort_unstable();
    quicksort(&mut words);
    assert_eq!(words, expect);
}

#[test]
fn two_sorted_runs_small() {
    // 两段有序输入（Gen 22 记录的已知二次方类，n=2k 保持 debug 快速）：
    // 断言正确性（该类的性能修复因布局税在 Gen 22/28 两度入档未落地，
    //  correctness 不受影响——已知慢但正确）。
    let n = 2_000usize;
    let half = n / 2;
    let data: Vec<u32> = (0..half as u32).chain(0..half as u32).collect();
    check(&data);
}

/// 仅 release 模式运行的百万级 stress（Gen 32）：unsafe 指针路径在优化后
/// 代码生成不同，debug 门禁不够；release 下 20k 只要 0.02s，故把大规模
/// 差分放到这里跑，默认 debug 门禁零成本（cfg 反选跳过）。
#[test]
#[cfg(not(debug_assertions))]
fn release_only_mega_stress() {
    let mut rng = Rng::new(0x4E5A_6A5E);
    let patterns: [&dyn Fn(usize, &mut Rng) -> Vec<u32>; 5] = [
        &|n, rng| (0..n).map(|_| rng.next_u32()).collect(),
        &|n, rng| (0..n).map(|_| rng.next_u32() % 7).collect(),
        &|n, _| (0..n as u32).collect(),
        &|n, _| (0..n as u32).rev().collect(),
        &|n, rng| {
            let mut v: Vec<u32> = (0..n as u32).collect();
            for _ in 0..n / 50 {
                let i = (rng.next_u64() as usize) % n;
                let j = (rng.next_u64() as usize) % n;
                v.swap(i, j);
            }
            v
        },
    ];
    for (idx, pat) in patterns.iter().enumerate() {
        let n = 200_000usize;
        let data = pat(n, &mut rng);
        let mut ours = data.clone();
        let mut theirs = data.clone();
        quicksort(&mut ours);
        theirs.sort_unstable();
        assert_eq!(ours, theirs, "mega stress pattern #{idx} mismatch");
    }
}
