//! 「快排进化论」—— 持续迭代中的 Rust 快速排序实现。
//!
//! 每一代都在 `EVOLUTION.md` 登记：改了什么、为什么、前后数据。
//! 判分器：`src/bin/bench.rs`；正确性门禁：`tests/differential.rs`。

pub mod inputs;

// GENERATION: 11 —— 退化分区强粉碎（scramble_patterns：分块轮换击败 organ-pipe 剥层链）
//
// Gen 9-10 的双分区 + 三档信号之上新增：
// 退化分区（DNF 升级后某侧 <= len/64 = pivot 恰为区间极值的铁证）时，
// 对大侧做分块轮换粉碎（首四分之一 ↔ 末四分之一，len/4 对 swap）。
//
// 为什么需要它：organ-pipe 的三采样 (1, max, 1) 使 pivot 恒为区间最小值，
// 每层只剥 1~2 个元素，2 对弱 break_patterns 打不碎「99% 两段有序」
// （Gen 5 教训），最终落 heapsort（0.26~0.38ms）。分块轮换把两段式结构
// 拦腰打碎，下一层 3-sort 的中位采样落到中位数值附近 —— organ-pipe 10k
// 实测 0.26~0.38ms → 0.133ms（heapsort 成本线），speedup 0.19~0.41x → 0.49~0.52x。
//
// 触发极窄：random 数据 pivot 恰为极值的概率 ~3/(2n)/节点，期望成本
// ~0.4 次交换/节点，可忽略；小侧（<128）自动 no-op 自限。
//
// 演进脉络：
// - Gen 6：默认 Hoare + 坏分区升级 DNF（pdqsort 分区策略）
// - Gen 8 诊断：random 大输入与 pdqsort 差 6.3 倍，瓶颈是 Hoare 扫描的数据依赖
//   分支 ~50% mispredict（~8 周期/次比较），不是边界检查
// - Gen 9：分支免费版 Lomuto（无条件 swap + cmov 条件自增）拿下 random
//   （1M 54.5→21.9ms，2.4 倍）；但 Lomuto 左区按扫描顺序搬运小于元素，
//   逆序输入下左区为逆序、partial insertion 接不住（1.47 vs Hoare 0.30 n log n）
// - Gen 10：信号从两档扩到三档 —— bail_pos ≤ 10（逆序密度）或 ≥ 48（下降沿极
//   稀疏 = 近乎有序）都走 Hoare；中间密度（random ~17、few-unique ~22、
//   organ-pipe ~n/2+9）走 Lomuto。nearly-sorted 10k 恢复 -51%（0.0765→0.0376ms）
//
// 结果：Gen 9 总分中位 0.604x（random 2.4~3.3 倍、few-unique 历史最佳）；
// Gen 10 总分中位 0.586x（噪声带内持平），但 nearly-sorted 绝对耗时 -51%。
//
// 实现要点（不变式，改动时勿破坏）：
// 1. 两分区统一输出「切分点 k」：arr[..k] 与 arr[k..] 两侧严格变小
// 2. branchless Lomuto：pivot 值 ManuallyDrop 本地副本；每步无条件 swap(i,j)，
//    `i += less as usize` cmov 化；!less 时 swap 交换两个都 >= pivot 的元素、无副作用
// 3. 坏分区升级 DNF / partial insertion / 模式粉碎 / 深度预算 heapsort fallback
//    全部保留；Hoare 是有结构数据的专用路径（逆序→左区有序；近有序→扫描提前收工）

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

/// 逆序密度信号阈值：partial insertion 的 bail 位置 <= 此值 → 判定逆序/近逆序。
/// 逆序数据第 9 个位置即攒够 9 个下降沿（密度 100%）；random 要约 17+，
/// 误触率约 1%（P(前 10 位内出现 9+ 下降沿, p=0.5) ≈ 1.1%）。
const DESCENDING_DENSE_BAIL: usize = 10;

/// 稀疏下降沿信号阈值：bail 位置 >= 此值 → 判定近乎有序（nearly-sorted 的
/// 1% 扰动密度下第 9 个下降沿约在 ~900 位；random ~17±5，P(>=48) ≈ 1.5% 误触）。
/// 近乎有序数据 Hoare 扫描大幅提前收工（Gen 6 实测 nearly-sorted 10k 1.598x，
/// Lomuto 只有 0.736x）—— 即「稀疏」也走 Hoare。
const SPARSE_DESCENT_BAIL: usize = 48;

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
/// 超过 PARTIAL_INSERTION_LIMIT 立即返回 Some(第九个下降沿的位置)（乱序输入
/// ~2·LIMIT 次比较退场，不碰任何交换）；确认近乎有序后，才用一趟插入排序直接
/// 排完切片并返回 None。
///
/// 返回的 bail 位置同时是 quicksort_rec 的分区策略信号：
/// 前 10 个位置就攒够 9 个下降沿 = 逆序/近逆序密度 → 选 Hoare。
fn partial_insertion_sort<T: Ord>(arr: &mut [T]) -> Option<usize> {
    let mut disorder = 0usize;
    for i in 1..arr.len() {
        if arr[i] < arr[i - 1] {
            disorder += 1;
            if disorder > PARTIAL_INSERTION_LIMIT {
                return Some(i);
            }
        }
    }
    insertion_sort(arr);
    None
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

/// median-of-three：三比较把三个位置排成 a[i] <= a[j] <= a[k]（排序网络，免 Clone）。
fn sort3<T: Ord>(arr: &mut [T], i: usize, j: usize, k: usize) {
    if arr[j] < arr[i] {
        arr.swap(i, j);
    }
    if arr[k] < arr[i] {
        arr.swap(i, k);
    }
    if arr[k] < arr[j] {
        arr.swap(j, k);
    }
}

/// median_of_3_sort：三比较把首/中/尾排成 a[0] <= a[mid] <= a[hi]。
/// Hoare 分区专用（其哨兵不变量要求 pivot 在 mid且两端有序）。
fn median_of_3_sort<T: Ord>(arr: &mut [T]) {
    let len = arr.len();
    sort3(arr, 0, (len - 1) / 2, len - 1);
}

/// Ninther 阈值：len >= 此值用九采样中位的的中位，否则中位三。
/// 盈亏点实测模型：ninther 省 ~13% 比较/交换（m·log2(m)·0.16），成本 ~20 比较，
/// m=32 附近回本，取 64 留余量。
const NINTHER_MIN: usize = 64;

/// 为 branchless Lomuto 选 pivot：len < 64 中位三；否则 Tukey ninther
/// （3 组三采样各取中位，再对 3 个中位取中位）。pivot 值放到 mid 位。
/// ninther 把 pivot 秩次方差再压一档：random 1.26 → ~1.10 n log n
///（比较与交换次数同降 ~13%）。
fn lomuto_pivot<T: Ord>(arr: &mut [T]) {
    let len = arr.len();
    let mid = (len - 1) / 2;
    if len < NINTHER_MIN {
        median_of_3_sort(arr);
        return;
    }
    let step = len / 8;
    let i0 = 0;
    let i1 = step;
    let i2 = 2 * step;
    let i3 = 3 * step;
    let i4 = 4 * step; // ≈ len/2
    let i5 = 5 * step;
    let i6 = 6 * step;
    let i7 = 7 * step;
    let i8 = len - 1;
    // 三组三采样，各组中位落到中间位（i1 / i4 / i7）
    sort3(arr, i0, i1, i2);
    sort3(arr, i3, i4, i5);
    sort3(arr, i6, i7, i8);
    // 三个中位取中位 → 落到 i4（九采样中位，Tukey ninther）
    sort3(arr, i1, i4, i7);
    // 钉到 branchless_partition 读取的 mid 位
    arr.swap(mid, i4);
}

/// Hoare 分区（pivot 位置跟踪，免 Clone）。调用方须先做 median_of_3_sort。
/// 返回切分点 k = j+1：arr[..k] <= pivot，arr[k..] > pivot。
///
/// Gen 9 起定位为「逆序数据专家」：逆序输入下 Hoare 分区产出的左区恰好有序，
/// 可被 partial insertion sort 免费接住（实测 0.30 n log n，近线性）；
/// 而 branchless Lomuto 的左区是逆序、需全额递归（1.47 n log n）。
/// 由 quicksort_rec 按 partial insertion 的下降沿密度信号选用。
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
            return j + 1;
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

/// 分支免费版 Lomuto 分区（Gen 9：branchless 化；Gen 12：裸指针去边界检查）。
/// 调用方须先做 median_of_3_sort，pivot 取中位值 = a[mid]。
///
/// 做法：每步「无条件 swap(i, j) + 条件自增 i」——比较结果只走 cmov 不进分支：
/// - arr[j] < pivot：正常 Lomuto，小于者换到左段，i 前进
/// - arr[j] >= pivot：swap(i, j) 交换的是两个都 >= pivot 的元素（左段头部与 j），
///   无副作用，i 不动 —— 不变量 [0, i) < pivot、[i, j+1) >= pivot 保持
///
/// 免 Clone：pivot 值 ptr::read 进 ManuallyDrop 本地副本（绝不 drop，泛型 T
/// 可能有 Drop 实现，避免双重 drop）；副本在扫描期间不变，是全部比较的基准。
///
/// 返回切分点 i：arr[..i) < pivot，arr[i..) >= pivot（等值归右段——
/// 全等/重复密集时 i 趋 0，会被调用方的失衡检测捕获并升级 DNF）。
/// 严格收缩：pivot 元素自身 == pivot 必在右段，故 i <= len-1 且 len-i >= 1。
///
/// 定位为「random/重复数据专家」：消除扫描分支的 ~50% mispredict，
/// random 大输入实测快 2.5~3.3 倍（1M：54.5→21.9ms）。
fn branchless_partition<T: Ord>(arr: &mut [T]) -> usize {
    let len = arr.len();
    debug_assert!(len >= 4);
    let mid = (len - 1) / 2;

    // SAFETY: mid < len；副本包 ManuallyDrop 绝不 drop
    let pivot: std::mem::ManuallyDrop<T> =
        unsafe { std::mem::ManuallyDrop::new(std::ptr::read(arr.as_ptr().add(mid))) };
    let base = arr.as_mut_ptr();

    let mut i = 0usize;
    for j in 0..len {
        // SAFETY: 不变式 i <= j < len（i 每轮至多追平 j、从不反超，见函数级注释）
        let less = unsafe { *base.add(j) < *pivot };
        // SAFETY: 同上。无条件交换 + cmov 化的条件自增（LLVM 编译为 cmov/setcc）。
        unsafe {
            std::ptr::swap(base.add(i), base.add(j));
        }
        i += less as usize;
    }
    i
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

/// 弱模式粉碎：坏分区后用少量确定性「伪随机」交换打碎自相似的输入结构。
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

/// 强模式粉碎（Gen 11）：退化分区专用 —— pivot 恰为区间最小/最大值时
/// （一侧 <= len/64），分区每层只剥 1~2 个元素，弱交换（2 对）打不碎
/// 「99% 两段有序」的 organ-pipe 结构（Gen 5 教训：2 对交换只动 4/n 个元素）。
/// 改用分块轮换：首四分之一与末四分之一整块互换（len/4 对 swap，O(len)），
/// 两段式结构被拦腰打碎，下一层 3-sort 的中位采样落到中位数值附近。
/// 索引只依赖长度不依赖数据；小侧调用时 len < 128 自动 no-op（自限）。
fn scramble_patterns<T>(arr: &mut [T]) {
    let len = arr.len();
    if len < 128 {
        return;
    }
    let q = len / 4;
    for i in 0..q {
        arr.swap(i, 3 * q + i);
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
        let Some(bail_pos) = partial_insertion_sort(arr) else {
            return;
        };
        depth_budget -= 1;

        // 分区策略选择（Gen 9-10，三档下降沿密度信号）：
        // - bail_pos <= 10：逆序密度（前 10 位 9+ 下降沿）→ Hoare：
        //   双向交换的分区副产品是「左区有序」，partial insertion 免费接住
        // - bail_pos >= 48：近乎有序（下降沿极稀疏）→ Hoare：
        //   扫描提前收工，nearly-sorted 10k 实测 1.598x vs Lomuto 0.736x
        // - 中间密度（random ~17、few-unique ~22、organ-pipe ~n/2+9）→ branchless Lomuto：
        //   消除扫描分支 mispredict，random 快 2.4 倍+
        // 原理：Hoare 吃有结构的数据，Lomuto 吃无结构数据。
        // 两条路径统一到切分点 k：arr[..k] 与 arr[k..] 两侧。
        let k = if bail_pos <= DESCENDING_DENSE_BAIL || bail_pos >= SPARSE_DESCENT_BAIL {
            median_of_3_sort(&mut arr); // Hoare 的哨兵不变量要求三位置有序
            hoare_partition(&mut arr)
        } else {
            lomuto_pivot(&mut arr); // Gen 13：ninther（len>=64）/ 中位三
            branchless_partition(&mut arr)
        };
        let left_len = k;
        let right_len = len - k;

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
                if d_left <= len / 64 || d_right <= len / 64 {
                    // 退化分区（pivot 恰为区间极值，每层只剥 1~2 个元素）：
                    // 强粉碎两侧（小侧不足 128 自动 no-op，自限）。
                    scramble_patterns(&mut arr[..lt]);
                    scramble_patterns(&mut arr[gt + 1..]);
                } else {
                    break_patterns_sides(&mut arr, lt, gt);
                }
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
                quicksort_rec(&mut arr[..k], depth_budget);
                arr = &mut arr[k..];
            } else {
                quicksort_rec(&mut arr[k..], depth_budget);
                arr = &mut arr[..k];
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
