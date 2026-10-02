//! Panic-safety 回归测试（第三方审计 Gen 49 的 S1 发现）。
//!
//! 背景：洞式插入把待插值 ptr::read 进 ManuallyDrop 本地变量，数组槽位里
//! 留下 ghost 副本。若用户的 `Ord::cmp` 在移位循环中 panic，unwind 会让
//! ghost 副本与该值的 real 副本被各 drop 一次 —— 对 `T: Drop` 是 double-free。
//!
//! 这两个测试用「panic 的 Ord + Drop 计数器」把每条路径都逼出 panic：
//! - case A：n=4 直达顶层 insertion_sort（CUTOFF 叶路径）
//! - case B：n=34 走 partial insertion → insertion_with_budget（预算内 panic）
//!
//! 断言：panic 之后每个 id 恰好被 drop 一次（修复前 PK 路径上必有 id 被
//! drop 两次、同时 saved 值泄漏零次）。
//!
//! 本文件仅在 debug profile 有意义：release profile 设了 panic = "abort"，
//! catch_unwind 不适用，测试整体跳过。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

use sort::quicksort;

/// Drop 计数器：`key` 参与排序，`id` 只用于 drop 计数；包含 id=7 的
/// 「诈弹」元素在「自身 key 大于对方 key」时 panic —— 即 `arr[j-1] > saved`
/// 的左值场景，正好是分区哨兵比较。
#[derive(Clone)]
struct DropCounter {
    id: u32,
    key: u32,
    log: Arc<Mutex<Vec<u32>>>,
}

impl PartialEq for DropCounter {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}
impl Eq for DropCounter {}
impl PartialOrd for DropCounter {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for DropCounter {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let ord = self.key.cmp(&other.key);
        if self.id == 7 && ord == std::cmp::Ordering::Greater {
            panic!(
                "adversarial Ord::cmp panic (id 7, key {} > {})",
                self.key, other.key
            );
        }
        ord
    }
}
impl Drop for DropCounter {
    fn drop(&mut self) {
        self.log.lock().unwrap().push(self.id);
    }
}

fn assert_each_id_dropped_exactly_once(log: &[u32], note: &str) {
    let mut counts = std::collections::HashMap::new();
    for &id in log {
        *counts.entry(id).or_insert(0usize) += 1;
    }
    let bad: Vec<String> = counts
        .iter()
        .filter(|&(_, c)| *c != 1)
        .map(|(id, c)| format!("id {id}: {c} drops"))
        .collect();
    assert!(
        bad.is_empty(),
        "{note}: panic 之后 drop 次数异常 = {bad:?} \
         （>1 = double drop，0 = 泄漏）"
    );
}

fn check_permutation(arr: &[DropCounter], orig: &[u32], note: &str) {
    let mut seen: Vec<u32> = arr.iter().map(|d| d.id).collect();
    seen.sort_unstable();
    let mut expect = orig.to_vec();
    expect.sort_unstable();
    assert_eq!(seen, expect, "{note}: panic 后数组不是合法排列");
}

#[test]
#[cfg(debug_assertions)]
fn panic_in_insertion_sort_happy_path() {
    // n=4 直达 insertion_sort（<= CUTOFF 叶路径）。
    // PK(id=7, key=2) 在「saved=key0 的 shift 循环」里作为左值出现 →
    // panic。之前已完成 2 次 shift：id=9(key3) 有 real + ghost 两份 →
    // 修复前 double drop。
    let log = Arc::new(Mutex::new(Vec::new()));
    let mk = |id: u32, key: u32| DropCounter {
        id,
        key,
        log: log.clone(),
    };
    let mut arr = vec![mk(9, 3), mk(2, 4), mk(7, 2), mk(1, 0)];
    let orig: Vec<u32> = arr.iter().map(|d| d.id).collect();

    let result = catch_unwind(AssertUnwindSafe(|| {
        quicksort(&mut arr);
    }));
    assert!(result.is_err(), "测试设计错误：这次排序没有 panic？");

    assert_each_id_dropped_exactly_once(&log.lock().unwrap(), "case A");
    check_permutation(&arr, &orig, "case A");
}

#[test]
#[cfg(debug_assertions)]
fn panic_in_insertion_with_budget_path() {
    // n=34 → 完整路径：partial insertion 的 3 个下降沿 <= 8 →
    // 调 insertion_with_budget（budget=36）：30 个 key=1 的升序元素 +
    // [key=50, key=3, PK(key=2,id=7), key=0]。PK 在「saved=key0 的 shift
    // 循环」中作为左值出现，此时已 shift 3 次（key=3/key=50 各有 ghost）
    // → panic。修复前 id=50 被 double-drop。
    let log = Arc::new(Mutex::new(Vec::new()));
    let mk = |id: u32, key: u32| DropCounter {
        id,
        key,
        log: log.clone(),
    };
    let mut arr: Vec<DropCounter> = (0..30)
        .map(|i| mk(100 + i, 1))
        .chain([mk(2, 50), mk(3, 50 + 1), mk(7, 2), mk(1, 0)])
        .collect();
    assert_eq!(arr.len(), 34);
    let orig: Vec<u32> = arr.iter().map(|d| d.id).collect();

    let result = catch_unwind(AssertUnwindSafe(|| {
        quicksort(&mut arr);
    }));
    assert!(result.is_err(), "测试设计错误：这次排序没有 panic？");

    assert_each_id_dropped_exactly_once(&log.lock().unwrap(), "case B");
    check_permutation(&arr, &orig, "case B");
}
