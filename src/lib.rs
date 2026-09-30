//! 「快排进化论」—— 持续迭代中的 Rust 快速排序实现。
//!
//! 每一代都在 `EVOLUTION.md` 登记：改了什么、为什么、前后数据。
//! 判分器：`src/bin/bench.rs`；正确性门禁：`tests/differential.rs`。

pub mod inputs;

// GENERATION: 0 —— 教科书朴素版：Lomuto 分区 + 末元素 pivot + 朴素递归
// 目的：故意保持最简形态，建立整条进化曲线的性能基线。
//
// 已知（留给后续世代解决的）缺陷：
// - 末元素 pivot 在「已排序 / 逆序 / 全等 / 风琴形」输入上分区极度不均 → O(n²) 且递归深度 ≈ n
// - Lomuto 分区在元素普遍小于 pivot 时交换次数约为 Hoare 分区两倍
// - 小分区仍然付出完整递归常数开销
//
// 铁律：任何世代改动必须先让 `cargo test` 全绿，才允许跑基准。

/// 原地快速排序（升序）。
pub fn quicksort<T: Ord>(arr: &mut [T]) {
    if arr.len() <= 1 {
        return;
    }
    quicksort_rec(arr);
}

fn quicksort_rec<T: Ord>(arr: &mut [T]) {
    let len = arr.len();
    if len <= 1 {
        return;
    }

    // Lomuto 分区：pivot 取区间最后一个元素，扫描期间 pivot 位置保持不动
    let hi = len - 1;
    let mut i = 0usize; // i 左侧（含 i-1）为已确认 <= pivot 的区域
    for j in 0..hi {
        if arr[j] <= arr[hi] {
            arr.swap(i, j);
            i += 1;
        }
    }
    arr.swap(i, hi); // pivot 落位
    let p = i;

    quicksort_rec(&mut arr[..p]);
    quicksort_rec(&mut arr[p + 1..]);
}
