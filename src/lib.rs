//! 「快排进化论」—— 持续迭代中的 Rust 快速排序实现。
//!
//! 每一代都在 `EVOLUTION.md` 登记：改了什么、为什么、前后数据。
//! 判分器：`src/bin/bench.rs`；正确性门禁：`tests/differential.rs`。

pub mod inputs;

// GENERATION: 5 —— pdqsort 三件套：DNF 三路分区 + 两段式 partial insertion sort + 坏分区模式粉碎
//
// 相对 Gen 4 的改进动机：
// - all-equal 是当时最差 case（0.043x）：Hoare 在全等输入上仍成对交换、逐层递归
// - few-unique（0.25x）：pivot 与大量元素相等时 Hoare 做无用功
//
// 三个组件以及每个的实际作用：
// 1. 荷兰旗三路分区（DNF）：分区成 <p | ==p | >p，==p 段直接就位不再递归。
//    全等输入一趟扫描即完成；海量重复按「不同值个数」收缩。
//    泛型 T: Ord 无法拷贝 pivot 值 —— 解法：Gen 1 的 pivot 位置跟踪扩展到 DNF
//    （swap 碰到 pivot 位就更新 p，维持不变式 a[p] 恒为 pivot 值）。
// 2. 两段式 partial insertion sort：分区前先纯比较数「下降沿」，超过 8 立即
//    退场（乱序输入 ~16 次比较即走，不碰交换）；确认近乎有序才一趟插入排完。
//    sorted / all-equal / 「有序+少量错位」在各节点直接完成。
// 3. 坏分区模式粉碎：pivot 贴边（某侧 < len/8）时，DNF 的 > 清扫会在右区留下
//    「有序+队尾一个错位元素」，下层三元素排序把次小值送进 pivot 位，形成
//    每层只剥 1 个元素的退火链。用确定性伪随机位置的少量交换打碎结构。
//
// 实现要点（不变式，改动时勿破坏）：
// 1. 三元素排序后 a[0] <= a[mid] <= a[hi]，pivot 值 = a[mid]，pivot 位置 p = mid 恒非末位
// 2. DNF 全程 a[p] 恒为 pivot 值（swap 碰 p 即跟随），这是所有比较的基准
// 3. 分区结果：arr[..lt) < pivot，arr[lt..=gt] == pivot（已就位、不再递归），
//    arr[gt+1..) > pivot；lt/gt 边界保证两侧严格变小
// 4. gt 不会下溢：仅当 i <= gt 才会 gt--，此时若 i == gt == 0 则 pivot 必在 0 位，
//    arr[0] > arr[0] 恒假，不会进入 > 分支
// 5. 深度预算耗尽仍 fallback heapsort（Gen 4 机制保留），最坏 O(n log n)

/// 小分区 cutoff：len <= 此值的子区间改用插入排序，不再分区递归。
const CUTOFF: usize = 16;

/// partial insertion sort 的容忍乱序数：扫描中「下降沿」超过此值即放弃。
///
/// 为什么必须两段式（先数下降沿、后插入）：
/// 单段边扫边插时，nearly-sorted 的第一个错位元素可能插入位移上千步，
/// 成本已经 O(n) 才数到第二个下降沿 —— 实测让 nearly-sorted 慢了 8 倍。
/// 两段式下，乱序输入最多 ~2·LIMIT 次比较即退场（逆向/风琴/随机都秒退），
/// 确认近乎有序后才付出一次性的 O(n) 插入。
const PARTIAL_INSERTION_LIMIT: usize = 8;

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

/// pdqsort 的 partial insertion sort（两段式）：先纯比较扫描数「下降沿」，
/// 超过 PARTIAL_INSERTION_LIMIT 立即返回 false（乱序输入 ~2·LIMIT 次比较退场，
/// 不碰任何交换）；确认近乎有序后，才用一趟插入排序直接排完切片（一次性 O(n)）。
///
/// 收益面：sorted / all-equal / 「有序+少量错位」（DNF > 清扫的自相似产物）
/// 在各自节点一趟完成，从根上消灭退火链，也让有序类输入跑赢 pdqsort。
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
        // 近乎有序的切片（有序+少量错位）直接排完返回，不进入分区路径
        if partial_insertion_sort(arr) {
            return;
        }
        depth_budget -= 1;

        let hi = len - 1;
        let mid = hi / 2;

        // median-of-three：三个比较把首/中/尾排成 a[0] <= a[mid] <= a[hi]
        if arr[mid] < arr[0] {
            arr.swap(0, mid);
        }
        if arr[hi] < arr[0] {
            arr.swap(0, hi);
        }
        if arr[hi] < arr[mid] {
            arr.swap(mid, hi);
        }

        // 荷兰旗三路分区。不变量：a[p] 恒为 pivot 值；
        // [..lt) < pivot，[lt..i) == pivot，(gt..len) > pivot。
        let mut p = mid;
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

        // arr[lt..=gt] 全部 == pivot，已就位，不再递归。
        let left_len = lt;
        let right_len = len - gt - 1;
        if left_len + right_len == 0 {
            return; // 全等切片：一趟扫描即完成
        }

        // 坏分区模式粉碎（pdqsort 核心技巧）：
        // pivot 贴边（某侧 < len/8）时，分区结构会自相似复制 —— 实测：
        // DNF 的 > 清扫在右区留下「有序+队尾一个错位元素」，下层三元素排序
        // 把次小值送进 pivot 位，形成每层只剥 1 个元素的退火链
        // （sorted 输入比较次数从 0.84 n log n 暴涨到 6.4 n log n）。
        // 用确定性伪随机位置的少量交换打碎结构，链即断。
        let balanced = left_len >= len / 8 && right_len >= len / 8;
        if !balanced {
            break_patterns(&mut arr[..lt]);
            break_patterns(&mut arr[gt + 1..]);
        }

        // 尾递归消除：小侧递归（入栈切片至多上轮一半，栈深 <= log2 n），大侧循环继续
        if left_len < right_len {
            quicksort_rec(&mut arr[..lt], depth_budget);
            arr = &mut arr[gt + 1..];
        } else {
            quicksort_rec(&mut arr[gt + 1..], depth_budget);
            arr = &mut arr[..lt];
        }
    }
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
}
