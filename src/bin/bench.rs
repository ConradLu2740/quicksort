//! 基准测试台（判分器）。
//!
//! 对每个 (分布, 规模) 组合，比较我们的 quicksort 与标准库 `sort_unstable`（pdqsort）。
//! 所有输入由 `sort::inputs` 固定 seed 生成，结果可复现。
//!
//! 用法：`cargo run --release --bin bench`
//!
//! 输出「EVOLUTION SPEED SCORE」= 各组合 speedup（pdqsort 耗时 / 我们耗时）的
//! 几何平均。这个分数就是进化曲线，越高越好。
//!
//! 计时方法：批量计时。单次排序在小 n 下低于计时器分辨率（Windows QPC ~100ns），
//! 因此连续排 k 个、总量一次计时再除以 k；拷贝基线单独测一版扣除。
//! 每组取 3 个批次的中位数。

use std::hint::black_box;
use std::time::Instant;

use sort::inputs::DISTS;
use sort::quicksort;

const BATCHES: usize = 3;
const WARMUP_BATCHES: usize = 1;

/// 一组批量计时的净单次耗时（ms）与该排序是否产出有序数组。
fn batch_time(data: &[u32], k: usize, ours: bool) -> (f64, bool) {
    let mut buf = data.to_vec();

    // 拷贝基线：k 次 copy_from_slice，之后从「拷贝+排序」里扣除
    let mut copy_times = Vec::with_capacity(BATCHES);
    for _ in 0..(WARMUP_BATCHES + BATCHES) {
        let start = Instant::now();
        for _ in 0..k {
            buf.copy_from_slice(data);
        }
        black_box(&buf);
        copy_times.push(start.elapsed().as_secs_f64() * 1e3 / k as f64);
    }

    let mut gross_times = Vec::with_capacity(BATCHES);
    for _ in 0..(WARMUP_BATCHES + BATCHES) {
        let start = Instant::now();
        for _ in 0..k {
            buf.copy_from_slice(data);
            if ours {
                quicksort(&mut buf);
            } else {
                buf.sort_unstable();
            }
        }
        black_box(&buf);
        gross_times.push(start.elapsed().as_secs_f64() * 1e3 / k as f64);
    }

    let copy_ms = median(&mut copy_times[WARMUP_BATCHES..]);
    let gross_ms = median(&mut gross_times[WARMUP_BATCHES..]);
    let ok = is_sorted(&buf);
    ((gross_ms - copy_ms).max(0.0), ok)
}

fn is_sorted(v: &[u32]) -> bool {
    v.windows(2).all(|w| w[0] <= w[1])
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn run() {
    println!(
        "{:<14} {:>9} {:>12} {:>12} {:>9}",
        "distribution", "n", "ours(ms)", "pdqsort(ms)", "speedup"
    );
    println!("{}", "-".repeat(60));

    let mut log_sum = 0.0f64;
    let mut cases = 0usize;
    let mut all_ok = true;
    let mut worst = ("", f64::INFINITY);

    for dist in DISTS {
        for &n in dist.sizes() {
            let data = dist.make(n);
            let k = dist.iters(n);
            let (ours_ms, ok1) = batch_time(&data, k, true);
            let (std_ms, _ok2) = batch_time(&data, k, false);
            all_ok &= ok1;
            // 计时器分辨率兜底：把不可分辨的耗时按 1ns 计，避免 ln(0)
            let ours_eff = ours_ms.max(1e-6);
            let std_eff = std_ms.max(1e-6);
            let speedup = std_eff / ours_eff;
            log_sum += speedup.ln();
            cases += 1;
            if speedup < worst.1 {
                worst = (dist.name(), speedup);
            }
            println!(
                "{:<14} {:>9} {:>12.4} {:>12.4} {:>8.3}x",
                dist.name(),
                n,
                ours_ms,
                std_ms,
                speedup
            );
        }
    }

    let score = (log_sum / cases as f64).exp();
    println!("{}", "-".repeat(60));
    println!(
        "EVOLUTION SPEED SCORE (geomean speedup vs pdqsort, {cases} cases): {score:.6}x"
    );
    println!("worst case: {} ({:.3}x)", worst.0, worst.1);

    if !all_ok {
        eprintln!("CORRECTNESS GATE FAILED: produced an unsorted array!");
        std::process::exit(1);
    }
}

fn main() {
    // 大栈线程：病态分布的朴素实现递归深度可达 ~n，1MB 默认栈不够
    let handle = std::thread::Builder::new()
        .stack_size(512 * 1024 * 1024)
        .spawn(run)
        .expect("spawn bench thread");
    handle.join().expect("bench thread panicked");
}
