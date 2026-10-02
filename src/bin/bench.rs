//! 基准测试台（判分器）。
//!
//! 对每个 (分布, 规模) 组合，比较我们的 quicksort 与标准库 `sort_unstable`。
//! 注意：Rust 1.81 起 `sort_unstable` 的内核已由 pdqsort 换成 **ipnsort**
//! （1.96 工具链实测标签，早期日志里的「对标 pdqsort」标签随之过时——
//! 新内核更快，所以同代码读数在 1.81 前后不可直接比）。
//! 所有输入由 `sort::inputs` 固定 seed 生成，结果可复现。
//!
//! 用法：`cargo run --release --bin bench`
//!
//! 输出「EVOLUTION SPEED SCORE」= 各组合 speedup（std 耗时 / 我们耗时）的
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
        "distribution", "n", "ours(ms)", "std(ms)", "speedup"
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
        "EVOLUTION SPEED SCORE (geomean speedup vs std sort_unstable (ipnsort), {cases} cases): {score:.6}x"
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
            "info two-run  n={:>7} ours(ms)={ours_ms:>10.4} std(ms)={std_ms:>10.4} speedup={:.3}x",
            data.len(),
            std_ms / ours_ms
        );
    }

    // few-unique 种子方差体检（Gen 50，复验外部评估最尖锐的发现）：
    // 外部三家在 k=2/4/8 上实测 speedup 跨种子跨度 7.5x（cv 91%）——重复密集
    // 输入的分区树只有十来个节点，单个路由决策翻转就能反转胜负，单种子读数
    // 不是稳定统计量。自家 bench 的 few-unique 恰是单一固定种子（k=5, n=10k）。
    // 这里同分布扫 9 个种子，量化自家参数下的方差。
    {
        let n = 10_000usize;
        for k in [2u32, 5, 8] {
            let mut ratios = Vec::new();
            for s in 1..=9u64 {
                let mut rng = sort::inputs::Rng::new(s.wrapping_mul(0x9E37_79B9_7F4A_7C15));
                let data: Vec<u32> = (0..n).map(|_| rng.next_u32() % k).collect();
                let (ours_ms, ok1) = batch_time(&data, 5, true);
                let (std_ms, _) = batch_time(&data, 5, false);
                all_ok &= ok1;
                ratios.push(std_ms / ours_ms);
            }
            let min = ratios.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = ratios.iter().cloned().fold(0.0, f64::max);
            let geo = (ratios.iter().map(|r| r.ln()).sum::<f64>() / ratios.len() as f64).exp();
            let mean = ratios.iter().sum::<f64>() / ratios.len() as f64;
            let var = ratios.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / ratios.len() as f64;
            println!(
                "info few-unique seed-sweep k={k} n={n}: geo={geo:.3}x min={min:.3}x max={max:.3}x spread={:.1}x cv={:.0}%",
                max / min,
                (var.sqrt() / mean * 100.0)
            );
        }
    }

    // 冷/热对照体检（Gen 51，复验外部评估「同数据重复热身」指控）：
    // 外部评估实测：对同一份数据重复排序，ours 提速至 2.4 倍而 std 恒定
    // （few-distinct 上 0.022→0.009ms），机制推测是分支预测器跨轮保留。
    // 本判分器的批设计每配置把同一数据排 20 次，系统性采到热端——若 warmup
    // 只利己方，23 case 分数就有结构性乐观偏差。这里同分布对比：
    //   warm = 现有 batch_time 语义（同数据 copy 复位后排 k 次）
    //   cold = 每轮新数据池（8 个互异数组，计时区内逐个排，不复位）
    {
        let pool = 8usize;
        let reps = 3usize;
        let gen_pool =
            |n: usize, seed: u64, k: Option<u32>| -> Vec<Vec<u32>> {
                let mut out = Vec::with_capacity(pool);
                for i in 0..pool {
                    let mut rng = sort::inputs::Rng::new(seed.wrapping_add(i as u64));
                    out.push((0..n).map(|_| match k {
                        Some(k) => rng.next_u32() % k,
                        None => rng.next_u32(),
                    }).collect());
                }
                out
            };
        let cold_time = |pools: &[Vec<Vec<u32>>], ours: bool| -> f64 {
            let mut best = Vec::new();
            for rep_pool in pools.iter().take(reps) {
                let t = Instant::now();
                for buf in rep_pool {
                    let mut buf = buf.clone();
                    if ours {
                        quicksort(&mut buf);
                    } else {
                        buf.sort_unstable();
                    }
                    black_box(&buf);
                }
                best.push(t.elapsed().as_secs_f64() * 1e3 / pool as f64);
            }
            median(&mut best)
        };
        for n in [1_000usize, 10_000] {
            for (name, k) in [("random", None), ("few-unique-k5", Some(5u32))] {
                let seed = 0xC01D_5EED;
                let pools: Vec<Vec<Vec<u32>>> = (0..reps)
                    .map(|r| gen_pool(n, seed + r as u64 * 1000, k))
                    .collect();
                let ours_cold = cold_time(&pools, true);
                let std_cold = cold_time(&pools, false);
                let data = pools[0][0].clone();
                let (ours_warm, _) = batch_time(&data, 5, true);
                let (std_warm, _) = batch_time(&data, 5, false);
                println!(
                    "info cold/warm n={n:>6} {name:<14}: cold ratio={:.3}x (ours {:.4?}ms) | warm ratio={:.3}x (ours {:.4?}ms) | std cold {:.4?}ms warm {:.4?}ms",
                    std_cold / ours_cold, ours_cold, std_warm / ours_warm, ours_warm, std_cold, std_warm
                );
            }
        }
    }

    // 规模扩展体检（Gen 51，复验外部评估的规模预言）：
    // 外部第三轮用 11 分布 × 4 规模分离测量断言：两轮分数分歧的根源是**规模
    // 区间**不是分布集——n=1k/10k 几何平均 ~1.18（≈打平），n=1e5/1e6 → 1.28~1.32
    // （慢三成）；我们的 23 表全是 n<=10k，所以读到 ~0.95；"缺的是规模不是分布"。
    // 这里把 7 个矩阵分布的规模补到 1e5/1e6，验证该预言（不计入 23 case 分数，
    // 保持跨代序列可比；iters 复用矩阵定义）。
    {
        let mut log_sum = 0.0f64;
        let mut cases = 0usize;
        for dist in DISTS {
            for &n in &[100_000usize, 1_000_000] {
                let data = dist.make(n);
                let k = dist.iters(n);
                let (ours_ms, ok1) = batch_time(&data, k, true);
                let (std_ms, _) = batch_time(&data, k, false);
                all_ok &= ok1;
                let ratio = std_ms / ours_ms;
                log_sum += ratio.ln();
                cases += 1;
                println!(
                    "info scale-ext {:<14} n={:>8} ours={ours_ms:>10.4} std={std_ms:>10.4} speedup={ratio:.3}x",
                    dist.name(),
                    n
                );
            }
        }
        let geo = (log_sum / cases as f64).exp();
        println!(
            "info scale-ext geomean (7 dists x 1e5/1e6, {cases} cases): {geo:.3}x —— 外部预言 ~0.76x（1.32x 慢）；23-case 分数（n<=10k）不受此行影响"
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
