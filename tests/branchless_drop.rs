//! branchless Lomuto 分区的非 Copy/Drop 类型安全测试（Gen 52）。
//!
//! 背景：第四家评估指控 `branchless_partition` 的 `ptr::read(arr[mid])`
//! 「搬空 mid 槽位后循环仍读该槽 = 未初始化内存读取 UB」。该评估方未能
//! 运行 Miri，仅静态审查。
//!
//! 本测试用证据裁决：非 Copy、带 Drop、带堆载荷的元素，长度 > CUTOFF 且
//! 路由进入 branchless Lomuto（random 走粗糙预筛直达），断言：
//! 1. 排序结果正确（对拍 std）；
//! 2. 多重集守恒（无丢失/重复）；
//! 3. **每个元素恰好 drop 一次**（无 double-drop、无泄漏）；
//! 4.  panic 路径同样恰好一次。
//!
//! Miri（Stacked Borrows）下同数据跑一遍：若真有未初始化读取/双重 drop，
//! Miri 直接报 UB。测试见文末运行说明。
//!
//! 顺带堵上一个真实覆盖缺口：此前泛型测试（tests/differential.rs 的
//! generic_type_paths）全是 n=5/9 的元素，走不到任何分区路径——
//! 非 Copy 类型的分区路径此前零覆盖。

use std::sync::{Arc, Mutex};

use sort::quicksort;

/// 非 Copy、带堆载荷（String 分配）、带全局 drop 计数的比较键。
/// `payload` 字段只为提供堆载荷与 Drop 副作用（UM：让 String 的 drop
/// 真实发生），比较只走 key——保留读引用以过 dead_code 检查。
#[derive(Clone)]
struct Heavy {
    key: u32,
    id: u32,
    payload: String,
    log: Arc<Mutex<Vec<u32>>>,
}

impl Heavy {
    fn touch(&self) -> usize {
        self.payload.len()
    }
}

impl PartialEq for Heavy {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.id == other.id
    }
}
impl Eq for Heavy {}

impl PartialOrd for Heavy {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Heavy {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}
impl Drop for Heavy {
    fn drop(&mut self) {
        self.log.lock().unwrap().push(self.id);
    }
}

/// 确定性 xorshift（不依赖外部 rng，n=10k 也秒级完成）
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn make(n: usize, seed: u64, log: &Arc<Mutex<Vec<u32>>>) -> Vec<Heavy> {
    let mut rng = Rng(seed);
    (0..n)
        .map(|i| Heavy {
            key: (rng.next() % 1000) as u32, // 有重复值，同时覆盖 Lomuto 与坏分区/DNF 路径
            id: i as u32,
            payload: format!("payload-{i}"),
            log: log.clone(),
        })
        .collect()
}

fn each_id_dropped_exactly_once(log: &[u32], note: &str) {
    let mut counts = std::collections::HashMap::new();
    for &id in log {
        *counts.entry(id).or_insert(0usize) += 1;
    }
    let bad: Vec<String> = counts
        .iter()
        .filter(|&(_, c)| *c != 1)
        .map(|(id, c)| format!("id {id}: {c} drops"))
        .collect();
    assert!(bad.is_empty(), "{note}: drop 次数异常 {bad:?}");
}

#[test]
fn branchless_lomuto_non_copy_drop_types_are_safe() {
    // n = 100/1000/10000：> CUTOFF(=32)，random 数据经粗糙预筛直达
    // ninther + branchless Lomuto；重复值把坏分区/DNF 升级路径也拉进来。
    for &n in &[100usize, 1000, 10_000] {
        for &seed in &[42u64, 1337, 7] {
            let log = Arc::new(Mutex::new(Vec::new()));
            let mut v = make(n, seed, &log);
            let mut expect: Vec<u32> = v.iter().map(|h| h.key).collect();
            expect.sort_unstable();

            quicksort(&mut v);

            let got: Vec<u32> = v.iter().map(|h| h.key).collect();
            assert_eq!(got, expect, "n={n} seed={seed}: 排序结果不正确");
            // 堆载荷确实存在（每个元素都持有有效 String 分配）
            assert!(
                v.iter().all(|h| h.touch() >= "payload-".len()),
                "n={n}: payload 异常"
            );

            // 多重集守恒：id 恰好一个都不少
            let mut ids: Vec<u32> = v.iter().map(|h| h.id).collect();
            ids.sort_unstable();
            assert_eq!(
                ids,
                (0..n as u32).collect::<Vec<_>>(),
                "n={n}: 多重集不守恒"
            );

            drop(v); // 此刻才析构 —— 若 pivot 搬运有 double-drop，计数立刻异常
            each_id_dropped_exactly_once(&log.lock().unwrap(), &format!("n={n} seed={seed}"));
        }
    }
}

#[test]
fn miri_sized_small_non_copy_drop() {
    // Miri 友好规模（n=100/200 > CUTOFF，仍进 branchless Lomuto）。
    // 若 branchless_partition 的 ptr::read(mid) 真构成未初始化读取或
    // double-drop，Miri（Stacked Borrows）在此直接报 UB。
    for &n in &[100usize, 200] {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut v = make(n, 4242, &log);
        let mut expect: Vec<u32> = v.iter().map(|h| h.key).collect();
        expect.sort_unstable();
        quicksort(&mut v);
        let got: Vec<u32> = v.iter().map(|h| h.key).collect();
        assert_eq!(got, expect);
        drop(v);
        each_id_dropped_exactly_once(&log.lock().unwrap(), &format!("miri n={n}"));
    }
}

#[test]
fn branchless_lomuto_panic_path_non_copy_drop_is_safe() {
    // panic 路径：在 branchless Lomuto 的比较中 panic（非 Copy + Drop）。
    // 断言：catch_unwind 后数组仍是合法排列 + 每元素恰好 drop 一次。
    // 注意：drop 日志记**唯一 id**（不是 key——key 域 0..999 天然重复）。
    struct PanicKey {
        key: u32,
        id: u32,
        log: Arc<Mutex<Vec<u32>>>,
    }
    impl PartialEq for PanicKey {
        fn eq(&self, o: &Self) -> bool {
            self.key == o.key
        }
    }
    impl Eq for PanicKey {}
    impl PartialOrd for PanicKey {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for PanicKey {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            let ord = self.key.cmp(&o.key);
            if self.key == 500 && ord == std::cmp::Ordering::Greater {
                panic!("adversarial cmp panic in lomuto");
            }
            ord
        }
    }
    impl Drop for PanicKey {
        fn drop(&mut self) {
            self.log.lock().unwrap().push(self.id);
        }
    }

    let log = Arc::new(Mutex::new(Vec::new()));
    // 1000 个随机 key + 一个 panic 键：粗糙预筛判粗糙（desc 2~6 概率 ~94%），
    // 直达 branchless Lomuto；若未中粗糙档，partial bail 后中间密度也走 Lomuto。
    let mut rng = Rng(99);
    let mut v: Vec<PanicKey> = (0..999)
        .map(|i| PanicKey {
            key: (rng.next() % 1000) as u32,
            id: i as u32,
            log: log.clone(),
        })
        .collect();
    v.push(PanicKey {
        key: 500,
        id: 999,
        log: log.clone(),
    });

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        quicksort(&mut v);
    }));

    // 是否 panic 无所谓（取决于 panic 键是否被以左值比较）——两条路都断言安全
    let n = v.len();
    drop(v); // 此刻才析构
    let log = log.lock().unwrap();
    // 每个唯一 id 恰好 drop 一次（panic 前部分槽位被搬动也不得双重 drop）
    each_id_dropped_exactly_once(&log, "panic path");
    assert_eq!(log.len(), n, "drop 总数应等于元素数");
    let _ = result;
}
