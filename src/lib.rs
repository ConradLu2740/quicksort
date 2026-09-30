//! 「快排进化论」—— 持续迭代中的 Rust 快速排序实现。
//!
//! 每一代都在 `EVOLUTION.md` 登记：改了什么、为什么、前后数据。
//! 判分器：`src/bin/bench.rs`；正确性门禁：`tests/differential.rs`。

pub mod inputs;

// GENERATION: 6 —— 混合分区：默认 Hoare 扫描 + 坏分区升级 DNF（pdqsort 正式版架构）
//
// 相对 Gen 5 的改进动机：
// - Gen 5 全程 DNF：random 大输入只有 0.12~0.15x。DNF 每层全量扫描一个元素一次
//   （1.58 n log n），Hoare 的指针相遇会提前收工（1.10 n log n）——差 44%
// - 但 Hoare 对重复密集数据没有 == 段冻结，all-equal / few-unique 要付
//   O(n log n) 的成对交换（Gen 4 实测 0.043x / 0.25x）
//
// 解法（pdqsort 的分区策略）：
// 1. 默认 Hoare 分区（位置跟踪，Gen 1 不变式）→ 边界 j：[..=j] <= pivot，[j+1..] > pivot
// 2. 分区失衡（某侧 < len/8）→ 升级通道：整段重跑 DNF 三路分区，
//    冻结 ==pivot 中段。重复密集时 Hoare 必失衡（pivot 是常见值 → 一侧近乎为空），
//    而全 distinct 数据只有 ~2-4% 节点触发升级，付 2n 过路费
// 3. partial insertion sort / 坏分区模式粉碎 / 深度预算 heapsort fallback 均保留
//
// 实现要点（不变式，改动时勿破坏）：
// 1. 三元素排序后 a[0] <= a[mid] <= a[hi]，pivot 值 = a[mid]，pivot 位置恒非末位
// 2. Hoare：swap 碰 pivot 位时 p 跟随移动，a[p] 恒为 pivot 值，扫描不越界；
//    边界 j 保证 arr[..=j] <= arr[j+1..] 且 j <= len-2
// 3. DNF：全程 a[p] 恒为 pivot 值；返回 (lt, gt)：
//    arr[..lt) < pivot，arr[lt..=gt] == pivot（就位不递归），arr[gt+1..) > pivot；
//    gt 不会下溢（i <= gt 才会 gt--，此时 pivot 必在 0 位，arr[0] > arr[0] 恒假）
// 4. 深度预算耗尽仍 fallback heapsort，最坏 O(n log n)

/// 小分区 cutoff：len <= 此值的子区间改用插入排序，不再分区递归。
/// 参数扫描记录（Gen 7，各跑 3 轮取中位，23 case 几何平均）：
/// 8 → 0.5026，12 → 0.5059，16 → 0.5173，24 → 0.5032，32 → 0.5098。
/// 差异均在 ±3% 噪声带内，16 即为最优，无需再调（避免无数据重搞此参数）。
const CUTOFF: usize = 16;

/// partial insertion sort 的容忍乱序数：扫描中「下降沿」超过此值即放弃。
///
/// 为什么必须两段式（先数下降沿、后插入）：
/// 单段边扫边插时，nearly-sorted 的第一个错位元素可能插入位移上千步，
/// 成本已经 O(n) 才数到第二个下降沿 —— 实测让 nearly-sorted 慢了 8 倍。
/// 两段式下，乱序输入最多 ~2·LIMIT 次比较即退场（逆向/风琴/随机都在 ~16 次比较内退场），
/// 确认近乎有序后才付出一次性的 O(n) 插入。
const PARTIAL_INSERTION_LIMIT: usize = 8;

/// 分区失衡阈值：任一侧 < len/8 视为坏分区。
const UNBALANCED_DIV: usize = 8;

/// 原地快速排序（升序）。
pub fn quicksort<T: Ord>(arr: &mut [T]) {
    if arr.len() > 1 {
        // 深度预算：3*log2(n)。正常输入 balanced 分割只用 log2(n)，永远碰不到；
        // 病态/对抗输入耗尽后触发 heapsort fallback，保证最坏 O(n log n)。
        // 常数取 3 而非经典 Musser 的 2：2 在「持续轻度不均衡」的输入
        // （实测 organ-pipe，分区树深 ~25~26 层）上会过早触发 fallback，
        // 3 留出余量且不破坏 O(n log n) 保证（总量 <= O(c·n·log n)）。
        let depth_budget = 3 * (usize::BITS - arr.len().leading_zeros()) as usize;
        quicksort_rec(arr, depth_budget);
    }
}

/// 小分区收尾：交换式插入排序（泛型 T: Ord 免 Clone 的标准写法）。
fn insertion_sort<T: Ord>(arr: &mut [T]) {
    for i in 1..arr.len() {
        let mut j = i;
        while j > 0 && arr[j] < arr[j - 1] {
            arr.swap(j, j - 1);
            j -= 1;
        }
    }
}

/// partial insertion sort（两段式）：先纯比较扫描数「下降沿」，
/// 超过 PARTIAL_INSERTION_LIMIT 立即返回 false（乱序输入 ~2·LIMIT 次比较退场，
/// 不碰任何交换）；确认近乎有序后，才用一趟插入排序直接排完切片（一次性 O(n)）。
///
/// 收益面：sorted / all-equal / 「有序+少量错位」在各节点直接完成，
/// 从根上消灭退火链，也让有序类输入跑赢 pdqsort。
fn partial_insertion_sort<T: Ord>(arr: &mut [T]) -> bool {
    let mut disorder = 0usize;
    for i in 1..arr.len() {
        if arr[i] < arr[i - 1] {
            disorder += 1;
            if disorder > PARTIAL_INSERTION_LIMIT {
                return false;
            }
        }
    }
    insertion_sort(arr);
    true
}

/// 堆排序（深度预算耗尽时的 fallback）：sift-down 建大顶堆，逐轮把堆顶换到尾部。
fn heapsort<T: Ord>(arr: &mut [T]) {
    let n = arr.len();
    if n < 2 {
        return;
    }
    for start in (0..n / 2).rev() {
        sift_down(arr, start, n);
    }
    for end in (1..n).rev() {
        arr.swap(0, end);
        sift_down(arr, 0, end);
    }
}

fn sift_down<T: Ord>(arr: &mut [T], mut root: usize, end: usize) {
    loop {
        let mut child = 2 * root + 1;
        if child >= end {
            return;
        }
        if child + 1 < end && arr[child] < arr[child + 1] {
            child += 1;
        }
        if arr[root] < arr[child] {
            arr.swap(root, child);
            root = child;
        } else {
            return;
        }
    }
}

/// median-of-three：三比较把首/中/尾排成 a[0] <= a[mid] <= a[hi]。
fn median_of_3_sort<T: Ord>(arr: &mut [T]) {
    let len = arr.len();
    let hi = len - 1;
    let mid = hi / 2;
    if arr[mid] < arr[0] {
        arr.swap(0, mid);
    }
    if arr[hi] < arr[0] {
        arr.swap(0, hi);
    }
    if arr[hi] < arr[mid] {
        arr.swap(mid, hi);
    }
}

/// Hoare 分区（pivot 位置跟踪，免 Clone）。调用方须先做 median_of_3_sort。
/// 返回边界 j：arr[..=j] <= pivot，arr[j+1..] > pivot，且 j <= len-2。
///
/// 已实验并否决（Gen 8）：改成「pivot 值 ptr::read 进 ManuallyDrop 本地副本 +
/// swap(0, mid) 钉位 + 裸指针无界检查扫描」。random 1M 仅 -3.4%（56.4→54.5ms），
/// 但 reverse/sorted 劣化 25~54%（总分 -9%）。诊断结论：random 大输入的 6 倍差距
/// 的真瓶颈是**分支预测失败**（数据依赖扫描 ~50% mispredict ≈ 8 周期/次比较），
/// 不是边界检查或 pivot 跟踪 —— 下一步应对准「无分支分区」，别再碰指针化。
fn hoare_partition<T: Ord>(arr: &mut [T]) -> usize {
    let hi = arr.len() - 1;
    let mut i = 1usize; // a[0] <= pivot，左扫描从 1 开始（pivot 位天然挡住越界）
    let mut j = hi - 1; // a[hi] >= pivot，右扫描从 hi-1 开始
    let mut p = hi / 2;

    loop {
        while arr[i] < arr[p] {
            i += 1;
        }
        while arr[j] > arr[p] {
            j -= 1;
        }
        if i >= j {
            return j;
        }
        arr.swap(i, j);
        // pivot 跟随交换移动，维持 a[p] 为 pivot 值
        if p == i {
            p = j;
        } else if p == j {
            p = i;
        }
        i += 1;
        j -= 1;
    }
}

/// DNF 三路分区（自带 3-sort + pivot 位置跟踪，免 Clone）。
/// 返回 (lt, gt)：arr[..lt) < pivot，arr[lt..=gt] == pivot（已就位），arr[gt+1..) > pivot。
fn dnf_partition<T: Ord>(arr: &mut [T]) -> (usize, usize) {
    let len = arr.len();
    let hi = len - 1;
    median_of_3_sort(arr);
    let mut p = hi / 2;
    let mut lt = 0usize;
    let mut gt = hi;
    let mut i = 0usize;
    while i <= gt {
        if arr[i] < arr[p] {
            arr.swap(lt, i);
            if p == lt {
                p = i;
            } else if p == i {
                p = lt;
            }
            lt += 1;
            i += 1;
        } else if arr[p] < arr[i] {
            arr.swap(i, gt);
            if p == gt {
                p = i;
            } else if p == i {
                p = gt;
            }
            gt -= 1;
        } else {
            i += 1;
        }
    }
    (lt, gt)
}

/// 坏分区后用少量确定性「伪随机」交换打碎自相似的输入结构。
/// 索引生成只依赖切片长度，不依赖数据 —— 静态输入无法自适应它。
fn break_patterns<T>(arr: &mut [T]) {
    let len = arr.len();
    if len < 8 {
        return;
    }
    let mut seed = (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let idx = |seed: &mut u64| {
        *seed ^= *seed >> 12;
        *seed ^= *seed << 25;
        *seed ^= *seed >> 27;
        (seed.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) as usize % len
    };
    let (a, b, c, d) = (idx(&mut seed), idx(&mut seed), idx(&mut seed), idx(&mut seed));
    if a != b {
        arr.swap(a, b);
    }
    if c != d {
        arr.swap(c, d);
    }
}

/// 坏分区（任一侧 < len/8）时对两侧分别调用模式粉碎。
/// 注意：必须分别作用于左/右两个子切片 —— 对整段调用会打乱分区边界！
fn break_patterns_sides<T>(arr: &mut [T], lt: usize, gt: usize) {
    if lt >= 8 {
        break_patterns(&mut arr[..lt]);
    }
    if arr.len() - gt - 1 >= 8 {
        break_patterns(&mut arr[gt + 1..]);
    }
}

fn quicksort_rec<T: Ord>(mut arr: &mut [T], mut depth_budget: usize) {
    loop {
        let len = arr.len();
        if len <= CUTOFF {
            insertion_sort(arr);
            return;
        }
        if depth_budget == 0 {
            heapsort(arr);
            return;
        }
        // 近乎有序的切片直接排完返回，不进入分区路径
        if partial_insertion_sort(arr) {
            return;
        }
        depth_budget -= 1;

        median_of_3_sort(&mut arr);
        let j = hoare_partition(&mut arr);
        let left_len = j + 1;
        let right_len = len - j - 1;

        if left_len + right_len == 0 {
            return; // 全等切片（Hoare 对全等也对半劈，不会到这里；防御性保留）
        }

        if left_len < len / UNBALANCED_DIV || right_len < len / UNBALANCED_DIV {
            // 坏分区升级通道（Gen 6 核心）：
            // 重复密集数据在此现身（pivot 是常见值 → 一侧近乎为空）。
            // 整段重跑 DNF，把 ==pivot 中段冻结，递归只处理 < / > 两侧。
            //
            // 已实验并否决：分区后做「等值采样探测」（两侧各采 4 位，命中 >=2 即升级）。
            // 它能多抓住 few-unique 这类按值域均衡的重复数据（0.514 → 0.608），
            // 但 random 全线付出 ~10%（每节点 8 次额外比较），总分净负 ~4%，已回退。
            let (lt, gt) = dnf_partition(&mut arr);
            let d_left = lt;
            let d_right = len - gt - 1;
            if d_left + d_right == 0 {
                return; // 整段 == pivot，一趟扫描即完成
            }
            if d_left < len / UNBALANCED_DIV || d_right < len / UNBALANCED_DIV {
                break_patterns_sides(&mut arr, lt, gt);
            }
            if d_left < d_right {
                quicksort_rec(&mut arr[..lt], depth_budget);
                arr = &mut arr[gt + 1..];
            } else {
                quicksort_rec(&mut arr[gt + 1..], depth_budget);
                arr = &mut arr[..lt];
            }
        } else {
            // 均衡分区：小侧递归，大侧循环
            if left_len < right_len {
                quicksort_rec(&mut arr[..=j], depth_budget);
                arr = &mut arr[j + 1..];
            } else {
                quicksort_rec(&mut arr[j + 1..], depth_budget);
                arr = &mut arr[..=j];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_sorted(v: &[u32]) -> bool {
        v.windows(2).all(|w| w[0] <= w[1])
    }

    #[test]
    fn heapsort_fallback_direct() {
        // depth_budget = 0：顶层分区后立即进入 heapsort fallback 路径
        let mut v: Vec<u32> = (0..5_000).rev().collect();
        quicksort_rec(&mut v, 0);
        assert!(is_sorted(&v));
    }

    #[test]
    fn depth_limit_holds_on_pathological_input() {
        // 风琴形（median-of-3 静态杀手）+ 极小深度预算：
        // 模拟最坏情况退化路径，验证 introsort 兜底依然产出全序
        let n = 20_000;
        let mut v: Vec<u32> = (0..n)
            .map(|i| {
                let half = if i < n / 2 { i } else { n - 1 - i };
                (half + 1) as u32
            })
            .collect();
        quicksort_rec(&mut v, 1);
        assert!(is_sorted(&v));
        assert_eq!(v.len(), n);
    }

    #[test]
    fn duplicate_heavy_escalation_path() {
        // 直接压测升级通道：pivot 恒为常见值 → Hoare 必失衡 → DNF 冻结等值段
        let mut v: Vec<u32> = (0..10_000).map(|i| (i % 3) as u32).collect();
        quicksort(&mut v);
        assert!(is_sorted(&v));
        assert_eq!(v.len(), 10_000);
    }
}
