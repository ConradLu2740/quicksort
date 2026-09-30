//! 「快排进化论」—— 持续迭代中的 Rust 快速排序实现。
//!
//! 每一代都在 `EVOLUTION.md` 登记：改了什么、为什么、前后数据。
//! 判分器：`src/bin/bench.rs`；正确性门禁：`tests/differential.rs`。

pub mod inputs;

// GENERATION: 3 —— Hoare + median-of-three + 小分区插入排序 cutoff
//
// 相对 Gen 2 的改进动机：
// - Gen 2 的递归一直钻到 len <= 3 才停；每个不足 20 个元素的小叶子
//   仍要付「3 次中位比较 + Hoare 分区 + 两次递归调用」的固定成本，
//   而小分区占全部叶子的绝大多数 —— 这是当前最大的常数项浪费
// - len <= 16 直接插入排序：近乎有序的小切片上接近线性、无递归开销
//
// 已知的、本代不解决的病灶：
// - organ-pipe 首轮后已退化为近似随机（Gen 2 实测），不再灾难
// - 最坏情况分区深度仍无保护（Gen 4 深限）
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
        quicksort_rec(arr);
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

fn quicksort_rec<T: Ord>(arr: &mut [T]) {
    let len = arr.len();
    if len <= CUTOFF {
        insertion_sort(arr);
        return;
    }
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

    quicksort_rec(&mut arr[..=j]);
    quicksort_rec(&mut arr[j + 1..]);
}
