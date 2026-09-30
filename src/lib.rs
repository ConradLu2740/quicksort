//! 「快排进化论」—— 持续迭代中的 Rust 快速排序实现。
//!
//! 每一代都在 `EVOLUTION.md` 登记：改了什么、为什么、前后数据。
//! 判分器：`src/bin/bench.rs`；正确性门禁：`tests/differential.rs`。

pub mod inputs;

// GENERATION: 1 —— Hoare 双指针分区 + 中间元素 pivot（位置跟踪法，无需 Clone）
//
// 相对 Gen 0 的改进动机：
// - Lomuto 每进一个 <= pivot 的元素就交换一次；Hoare 从两端向中间成对交换，
//   交换次数约为 Lomuto 的一半（random 输入预期 0.2x → 0.4~0.6x）
// - 全等输入下 Lomuto 每次只剥掉 1 个元素（O(n²)）；
//   Hoare 在全等时 i/j 在中间相遇、直接对半劈（O(n log n)）
//
// 实现要点（不变式，改动时勿破坏）：
// 1. pivot 取索引 p = (len-1)/2，永远不是末位 —— 这是递归必然收缩的前提
// 2. 每次 swap 若碰到 pivot 所在位置，p 跟随移动到新位置，
//    保证不变式 a[p] 始终等于初始 pivot 值 —— Hoare 扫描由此不会越界：
//    左扫描必停于首个 >= pivot 处（最迟停在 pivot 位），右扫描必停于首个 <= pivot 处
// 3. 分区边界返回 j：arr[..=j] 全部 <= arr[j+1..] 全部，且 j <= len-2，
//    两侧都必然严格变小，递归终止

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

    let mut i = 0usize;
    let mut j = len - 1;
    let mut p = (len - 1) / 2; // pivot 位置；不变量：a[p] 恒等于初始 pivot 值

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
        // pivot 跟随交换移动，维持 a[p] 为初始 pivot 值
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
