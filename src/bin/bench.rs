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

    // 相位 gauge（Gen 31，放在矩阵之后测量——实测提前跑会改变分配器/缓存
    // 状态、污染被测 case 达 +10%，只能后置）：固定参照负载作为「机器
    // 温度计」随分数输出。冷/热相位可让 pdqsort 侧读数摆动 ~20%，
    // 单看总分会误读跨代比较——今后每个 EVOLUTION.md 条目都配 gauge 阅读。
    let gauge = {
        let n = 1_000_000usize;
        let mut rng = sort::inputs::Rng::new(0x6A5E_6A5E);
        let data: Vec<u32> = (0..n).map(|_| rng.next_u32()).collect();
        let (ms, _ok) = batch_time(&data, 3, false);
        ms
    };
    println!("phase gauge (std 1M random): {gauge:.3} ms —— 跨代比较请对 gauge（~8.8 = 冷相, ~10.3 = 热相）");

    // 附加信息测量（Gen 42，不计入 23 case 分数——保持分数序列跨代可比）：
    // two-run 分布（[升序 run | 升序 run]，合并有序流的真实类）——
    // Gen 22 发现的二次方陷阱、Gen 39 修复（259ms→1.82ms@100k）后的性能水位。
    {
        let half = 50_000u32;
        let data: Vec<u32> = (0..half).chain(0..half).collect();
        let (ours_ms, ok1) = batch_time(&data, 5, true);
        let (std_ms, _ok2) = batch_time(&data, 5, false);
        all_ok &= ok1;
        println!(
            "info two-run  n={:>7} ours(ms)={ours_ms:>10.4} pdqsort(ms)={std_ms:>10.4} speedup={:.3}x",
            data.len(),
            std_ms / ours_ms
        );
    }

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
