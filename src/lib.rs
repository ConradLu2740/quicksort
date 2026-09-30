//! 「快排进化论」—— 持续迭代中的 Rust 快速排序实现。
//!
//! 每一代都在 `EVOLUTION.md` 登记：改了什么、为什么、前后数据。
//! 判分器：`src/bin/bench.rs`；正确性门禁：`tests/differential.rs`。

pub mod inputs;

// GENERATION: 4 —— introsort 式保险：小侧递归 + 大侧循环（尾递归消除）+ 深度预算 fallback 堆排序
//
// 相对 Gen 3 的改进动机：
// - Gen 3 及以前：两侧都递归。若 pivot 持续极劣（对抗/病态输入），
//   递归深度 ≈ n，既有爆栈风险，时间也退化到 O(n²)
// - 「总是递归较小侧、循环较大侧」：每次入栈切片至多是上轮的一半，
//   栈深结构性 <= log2(n)（1M 输入也只有 ~20 层）
// - 深度预算 2*log2(n) 耗尽时该切片 fallback 堆排序：
//   最坏时间复杂度获得 O(n log n) 硬保证（introsort 的核心思想）
//
// 实现要点（不变式，改动时勿破坏）：
// 1. 三元素排序后 a[0] <= a[mid] <= a[hi]，pivot 值 = a[mid]，pivot 位置 p = mid 恒非末位
// 2. swap 碰到 pivot 位时 p 跟随移动，a[p] 恒为 pivot 值 —— Hoare 扫描不会越界
// 3. 分区边界 j 满足 arr[..=j] <= arr[j+1..] 且 j <= len-2，两侧严格变小

/// 小分区 cutoff：len <= 此值的子区间改用插入排序，不再分区递归。
const CUTOFF: usize = 16;

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

        let mut i = 1usize; // a[0] <= pivot，左扫描从 1 开始（pivot 位天然挡住越界）
        let mut j = hi - 1; // a[hi] >= pivot，右扫描从 hi-1 开始
        let mut p = mid;

        loop {
            while arr[i] < arr[p] {
                i += 1;
            }
            while arr[j] > arr[p] {
                j -= 1;
            }
            if i >= j {
                break;
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

        // 尾递归消除：小侧递归（入栈切片至多上轮一半，栈深 <= log2 n），大侧循环继续
        let left_len = j + 1;
        let right_len = len - j - 1;
        if left_len < right_len {
            quicksort_rec(&mut arr[..=j], depth_budget);
            arr = &mut arr[j + 1..];
        } else {
            quicksort_rec(&mut arr[j + 1..], depth_budget);
            arr = &mut arr[..=j];
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
}
