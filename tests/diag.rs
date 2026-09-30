//! 比较次数回归测试（permanent，勿删）。
//!
//! 背景：Gen 5（DNF 三路分区）曾经在 sorted 输入上比较次数暴涨到 6.4 n log n
//! （「有序+错位尾元素」自相似退火链），墙钟时间 21 倍回退，靠这个诊断定位。
//! 现固化为断言：任何世代不得让比较次数明显劣于 n log n 量级，
//! 防止同类结构性退化在无墙钟信号时悄悄混入。

use std::cell::Cell;
use std::cmp::Ordering;

use sort::quicksort;

thread_local! {
    static COUNT: Cell<u64> = Cell::new(0);
}

#[derive(PartialEq, Eq, Clone, Copy)]
struct C(u32);

impl PartialOrd for C {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for C {
    fn cmp(&self, other: &Self) -> Ordering {
        COUNT.with(|c| c.set(c.get() + 1));
        self.0.cmp(&other.0)
    }
}

/// 单次排序的比较次数，以及是否排对。
fn count_comparisons(data: &[C]) -> (u64, bool) {
    let mut v = data.to_vec();
    COUNT.with(|c| c.set(0));
    quicksort(&mut v);
    let count = COUNT.with(|c| c.get());
    (count, v.windows(2).all(|w| w[0] <= w[1]))
}

fn assert_ratio(name: &str, data: &[C], bound: f64) {
    let n = data.len() as f64;
    let (count, ok) = count_comparisons(data);
    let ratio = count as f64 / (n * n.log2());
    assert!(ok, "{name}: output not sorted");
    println!("{name:<8} n={len:<7} comparisons={count:>9} ({ratio:.2} n log n)", len = data.len());
    assert!(
        ratio <= bound,
        "{name}: {ratio:.2} n log n exceeds bound {bound} n log n —— 疑似结构性退化（参考 Gen 5 事故：6.38 n log n）"
    );
}

#[test]
fn comparison_counts_stay_near_n_log_n() {
    const N: usize = 10_000;
    let bound = 3.0;

    assert_ratio(
        "sorted",
        &(0..N as u32).map(C).collect::<Vec<_>>(),
        bound,
    );
    assert_ratio(
        "reverse",
        &(0..N as u32).rev().map(C).collect::<Vec<_>>(),
        bound,
    );
    // 确定性 xorshift，不用外部 rng
    let mut seed = 0x243F_6A88_85A3_08D3u64;
    let mut rng = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let random: Vec<C> = (0..N).map(|_| C((rng() % N as u64) as u32)).collect();
    assert_ratio("random", &random, bound);
}
