//! 非自反 `Ord` 的契约回归测试（Gen 51）。
//!
//! 背景：外部三家评估的 C5.7 构造出具体见证——`hoare_partition` 左扫描
//! `while arr[i] < arr[p] { i += 1 }` 原本无上界，靠 `sort3` 建立的哨兵
//! （a[0] <= pivot <= a[hi]）挡住。**非自反**比较器（`cmp(x, x) == Less`）
//! 使 `arr[p] < arr[p]` 成立，pivot 位不再是障碍，左扫描跑飞到 `arr[len]`。
//!
//! 历史行为（本仓库修复前实测）：公开 API `quicksort` 可触发越界 **panic**
//! （本构造 n=34 起；外部构造最小 n=33 = CUTOFF+1，即第一次进入分区路径的
//! 尺寸）。是安全索引的边界检查 panic，不是内存不安全。
//!
//! Gen 51 修复：左右扫描加显式边界（`i < hi` / `j > 0`）。合法 Ord 下哨兵
//! 先触发、边界永不生效（零行为变化）；非自反 Ord 下越界 panic 变成不可能
//! —— 排序输出无意义（契约允许），但**不 panic、不挂起、多重集守恒**。
//!
//! 触发条件是「非自反」（`cmp(x,x) != Equal`），比「非传递」更窄——
//! 石头剪刀布型（自反但非传递）不触发跑飞（对照组验证）。

use std::cmp::Ordering;

use sort::quicksort;

/// 见证比较器：除 (0,1) 对外全部返回 Less —— 非自反（x < x 恒真）。
/// (0,1) -> Greater 这一对让 `try_reverse_sorted` 的整段递减扫描在 i=1 处
/// 失败（否则 reverse 数据会被短路反转、根本进不了分区路径）。
/// Gen 51 修复前，该输入使 hoare 左扫描跑飞到 arr[len] 触发越界 panic。
struct SentinelBreaker(u32);

impl PartialEq for SentinelBreaker {
    fn eq(&self, _: &Self) -> bool {
        false // 非自反：自己也不等于自己
    }
}
impl Eq for SentinelBreaker {}
impl PartialOrd for SentinelBreaker {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o)) // 必须手写委托给 cmp：derive(PartialOrd) 会比较
                          // 内部字段、绕过自定义 Ord，测了个寂寞（外部评估踩过）
    }
}
impl Ord for SentinelBreaker {
    fn cmp(&self, o: &Self) -> Ordering {
        if self.0 == 0 && o.0 == 1 {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }
}

/// v[0]=1, v[1]=0（Greater 对相邻），其余递增互异。
fn witness(n: usize) -> Vec<SentinelBreaker> {
    assert!(n >= 3);
    let mut v: Vec<SentinelBreaker> = (1..n as u32).map(SentinelBreaker).collect();
    v[0] = SentinelBreaker(1);
    v[1] = SentinelBreaker(0);
    v
}

fn multiset(v: &[SentinelBreaker]) -> Vec<u32> {
    let mut ids: Vec<u32> = v.iter().map(|x| x.0).collect();
    ids.sort_unstable();
    ids
}

#[test]
fn non_reflexive_ord_completes_without_panic_and_conserves_multiset() {
    // Gen 51 修复后：跑飞被边界截停，排序完成（输出无意义，契约允许），
    // 不 panic、不挂起、每个元素恰好还在（无丢失/重复/未初始化）。
    for &n in &[34usize, 64, 1000, 50_000] {
        let mut v = witness(n);
        let before = multiset(&v);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            quicksort(&mut v);
        }));
        assert!(result.is_ok(), "n={n}: 非自反 Ord 修复后不应 panic（Gen 51 前此处越界 panic）");
        assert_eq!(multiset(&v), before, "n={n}: 多重集不守恒");
    }
}

#[test]
fn reflexive_nontransitive_ord_completes_too() {
    // 对照组：自反但非传递（石头剪刀布型）—— 从未触发跑飞，
    // 修复前后行为一致：完成、输出无意义、多重集守恒。
    struct Rps(u8);
    impl PartialEq for Rps {
        fn eq(&self, o: &Self) -> bool {
            self.0 == o.0 // 自反：x == x 成立
        }
    }
    impl Eq for Rps {}
    impl PartialOrd for Rps {
        fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for Rps {
        fn cmp(&self, o: &Self) -> Ordering {
            match (self.0, o.0) {
                (a, b) if a == b => Ordering::Equal,
                (0, 1) | (1, 2) | (2, 0) => Ordering::Greater,
                _ => Ordering::Less,
            }
        }
    }
    let mut v: Vec<Rps> = (0..2000u32).map(|i| Rps((i % 3) as u8)).collect();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        quicksort(&mut v);
    }));
    assert!(result.is_ok(), "石头剪刀布型不应 panic");
    let mut counts = [0usize; 3];
    for x in &v {
        counts[x.0 as usize] += 1;
    }
    assert!(counts.iter().all(|&c| c > 0), "多重集应守恒");
}
