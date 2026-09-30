//! 「快排进化论」—— 持续迭代中的 Rust 快速排序实现。
//!
//! 每一代都在 `EVOLUTION.md` 登记：改了什么、为什么、前后数据。
//! 判分器：`src/bin/bench.rs`；正确性门禁：`tests/differential.rs`。

pub mod inputs;

// GENERATION: 2 —— Hoare 分区 + median-of-three pivot
//
// 相对 Gen 1 的改进动机：
// - 单取中间元素，pivot 秩次方差大；首/中/尾取中位数后，
//   random 输入比较次数从 ~1.39·n·log n 降至 ~1.19·n·log n（期望 ~15%）
// - len <= 3 时「三元素排序」本身就是完全排序，直接返回
//   （小分区提前收手的雏形，为 Gen 3 插入排序 cutoff 铺路）
//
// 已知的、本代不解决的病灶（诚实记录）：
// - organ-pipe（风琴形）：首/中/尾 = (1, max, 1)，中位数 = 1 = 最小值，
//   pivot 恒取极小值 → 仍 O(n²)。median-of-three 对静态构型杀手无效，
//   真正的防线是 Gen 4 深限 fallback / Gen 6 模式识别
//
// 实现要点（不变式，改动时勿破坏）：
// 1. 三元素排序后 a[0] <= a[mid] <= a[hi]，pivot 值 = a[mid]，pivot 位置 p = mid 恒非末位
// 2. swap 碰到 pivot 位时 p 跟随移动，a[p] 恒为 pivot 值 —— Hoare 扫描不会越界
// 3. 分区边界 j 满足 arr[..=j] <= arr[j+1..] 且 j <= len-2，两侧严格变小

/// 原地快速排序（升序）。
pub fn quicksort<T: Ord>(arr: &mut [T]) {
    if arr.len() > 1 {
        quicksort_rec(arr);
    }
}

fn quicksort_rec<T: Ord>(arr: &mut [T]) {
    let len = arr.len();
    if len <= 1 {
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
    if len <= 3 {
        return; // 三个位置就是全部元素，已全序
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
