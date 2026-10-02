// ============================================================================
// quicksort_Gen49 —— 「快排进化论」最终版（Gen 49, 2026-10-01）
//               → Gen 51 增量：外部三家复评——hoare 双扫描显式边界
//                 （非自反 Ord 越界 panic 见证加固，金丝雀零成本）
//               → Gen 50 增量：外部三家评估后的一行修复（Stacked Borrows）
//
// 自包含单文件版本：从 src/lib.rs 摘出，无任何外部依赖（仅 std），
// 去掉 pub mod inputs; 一行后即可放入任意 crate 直接 cargo build。
//
// Gen 50 变更（外部三家独立评估反馈）：
// - InsertHole::shift 改单指针派生 —— 原写法先 as_ptr() 再 as_mut_ptr()，
//   实参从左到右求值使可变 retag 作废共享 tag，Stacked Borrows UB
//   （Miri 实测；Tree Borrows 下原写法合法。修复后两模型均干净）
// - bail_pos 双生产者语义、非全序契约按实测入文档
//
// 算法：原地、不稳定、零分配的泛型 T: Ord 快排。
// 主要机制（按入口 quicksort_rec 的执行顺序）：
//   1. len <= 32        -> 洞式插入排序（InsertHole drop guard，panic 安全）
//   2. depth_budget = 0 -> heapsort fallback（introsort 式 O(n log n) 硬保证）
//   3. len >= 128 粗糙预筛（8 采样点三态路由）：
//      粗糙 -> 直达 ninther + 分支免费 Lomuto
//      其余 -> 两段式 partial insertion（0 下降沿 = 已有序直达；
//             <=8 下降沿 = 带位移预算的插入排完；>8 = bail 进三档路由）
//   4. 三档路由（bail_pos）：<=10 逆序密度（先试整段反转检测）-> Hoare；
//      >=48 稀疏无降尾（近乎有序）-> Hoare；>=48 有连续降尾（organ-pipe 家族）
//      / 中间密度 -> Lomuto + ninther
//   5. 坏分区（任一侧 < len/8）：DNF 三路冻结 ==pivot 段；退化分区
//      （某侧 <= len/64）分块轮换强粉碎，否则弱粉碎两侧
//   6. 均衡分区：小侧递归（栈深 <= log2 n）+ 大侧循环
//
// 契约：T: Ord 必须为全序，违反时 panic 不产生内存不安全；cmp panic 时
// 数组恢复为合法排列（每元素恰好存活一次）。均与 slice::sort_unstable 同级。
// ============================================================================

// 小分区 cutoff：len <= 此值的子区间改用插入排序，不再分区递归。
// 参数扫描（U32 几何平均分数三轮中位）：
// - Gen 15（洞式插入架构）：8 -> 0.668、12 -> 0.680、16 -> 0.728、24 -> 0.741、32 -> 0.748
// - Gen 23（+零下降沿跳过/逆序检测）：16 -> 0.887、24 -> 0.927、32 -> 0.936、48 -> 0.908
//   （完成路径更便宜后 32 微弱占优；48 起叶级 O(CUTOFF^2) 反超。三重确认）
const CUTOFF: usize = 32;

/// 降序游程检测（Gen 24）：bail 点前 8 个位置是否连续严格递减。
/// 用于在「稀疏下降沿」档位里区分近乎有序（连续 9 个递减概率 ~0）与
/// 大块降序尾（organ-pipe 家族必然连续，应改走 Lomuto+ninther）。
#[inline(never)]
fn descending_run_at_bail<T: Ord>(arr: &[T], bail_pos: usize) -> bool {
    let n = arr.len();
    let end = bail_pos.min(n - 1);
    let start = bail_pos.saturating_sub(8);
    for j in (start + 1)..=end {
        if arr[j] >= arr[j - 1] {
            return false;
        }
    }
    true
}

/// 粗糙数据预筛（Gen 25）：8 个等距采样点的 7 个相邻对里下降沿数为 2~6
/// -> 判定「粗糙」-> 跳过下降沿扫描，直达 Lomuto+ninther。
///
/// 三态设计：
/// - 7/7 全下降 = 逆序密度（reverse 家族）-> 必须走完整路径
/// - <=1 下降 = 平滑（sorted / all-equal / nearly-sorted）-> 必须走完整扫描
/// - 2~6 下降 = 粗糙（random 落入概率 ~94%；organ-pipe 等距采样 desc=4 同档）
///
/// 筛子只选择分区器、不参与正确性：任何档位走的都是合法分区路径。
#[inline(never)]
fn route_direct_lomuto<T: Ord>(arr: &[T]) -> bool {
    let n = arr.len();
    if n < 16 {
        return false; // 小切片完整扫描本来就便宜
    }
    let step = (n - 1) / 7;
    let mut desc = 0usize;
    for i in 1..8 {
        let p = (i * step).min(n - 1);
        let q = ((i - 1) * step).min(n - 1);
        if p != q && arr[p] < arr[q] {
            desc += 1;
        }
    }
    (2..7).contains(&desc)
}

/// 分区路由决策（Gen 24 外描版）：返回 true 走 Hoare，false 走 branchless Lomuto。
///
/// 三档下降沿密度信号：
/// - bail_pos <= 10：逆序密度 -> Hoare
/// - bail_pos >= 48 且 bail 点前无连续降序游程：近乎有序 -> Hoare（扫描提前收工）
/// - bail_pos >= 48 且有连续降序游程：大块降序尾（organ-pipe）-> Lomuto+ninther
/// - 中间密度：branchless Lomuto
///
/// **bail_pos 语义注意（Gen 50 外部评估发现）**：这个阈值有**两个语义不同的
/// 生产者**，本文件只记录了其一：
/// - `partial_insertion_sort` -> 第 9 个下降沿的下标（阈值标定时假设的语义）
/// - `insertion_with_budget` -> 移位预算耗尽时的外层循环下标（另一个量，
///   实测 nearly-sorted 上占 ~74%）
/// 三档阈值的标定是在两种信号混合的分布上完成的。若要重调阈值，必须先
/// 区分来源重新标定，否则是对着错误分布拟合。
///
/// 整个决策 #[inline(never)] 外描：热循环里只剩一次调用，
/// 规避「热函数体积变化 -> 代码布局位移 -> 无关 case 劣化」。
#[inline(never)]
fn wants_hoare<T: Ord>(arr: &[T], bail_pos: usize) -> bool {
    if bail_pos <= DESCENDING_DENSE_BAIL {
        return true;
    }
    if bail_pos >= SPARSE_DESCENT_BAIL {
        return !descending_run_at_bail(arr, bail_pos);
    }
    false
}

/// 分区策略选择：三档信号 + 逆序检测 + 降序游程精化。
/// 原理：Hoare 吃有结构数据（逆序输入左区天然有序），Lomuto 吃无结构数据；
/// organ-pipe 是「对称采样只能取到 min」的例外，必须换 ninther 跳出剥层链。
fn partition_router<T: Ord>(arr: &mut [T], bail_pos: usize) -> Option<usize> {
    if bail_pos <= DESCENDING_DENSE_BAIL && try_reverse_sorted(arr) {
        return None; // 整段逆序，已反转成升序
    }
    Some(if wants_hoare(arr, bail_pos) {
        median_of_3_sort(arr); // Hoare 的哨兵不变量要求三位置有序
        hoare_partition(arr)
    } else {
        lomuto_pivot(arr);
        branchless_partition(arr)
    })
}

/// partial insertion sort 的容忍乱序数：扫描中「下降沿」超过此值即放弃。
/// 必须两段式（先数下降沿、后插入）：单段边扫边插时，nearly-sorted 的
/// 第一个错位元素可能插入位移上千步，成本 O(n) 才数到第二个下降沿
/// （实测让 nearly-sorted 慢 8 倍）。两段式下乱序输入 ~2*LIMIT 次比较即退场。
const PARTIAL_INSERTION_LIMIT: usize = 8;

/// 逆序密度信号阈值：bail 位置 <= 此值 -> 逆序/近逆序。
/// 逆序数据第 9 个位置即攒够 9 个下降沿；random 要约 17+，误触率 ~1%。
const DESCENDING_DENSE_BAIL: usize = 10;

/// 稀疏下降沿信号阈值：bail 位置 >= 此值 -> 近乎有序。
/// nearly-sorted 的 1% 扰动密度下第 9 个下降沿约在 ~900 位；random ~17±5。
/// 近乎有序数据 Hoare 扫描大幅提前收工（实测 nearly-sorted 10k 1.6x vs 0.74x）。
const SPARSE_DESCENT_BAIL: usize = 48;

/// 分区失衡阈值：任一侧 < len/8 视为坏分区。
const UNBALANCED_DIV: usize = 8;

/// 原地快速排序（升序，不稳定，零分配）。
///
/// # 复杂度
/// - 期望时间 O(n log n)
/// - 最坏时间 O(n log n)：深度预算 3*log2(n) 耗尽时该切片 fallback 堆排序，
///   栈深结构性 <= log2(n)（小侧递归）
/// - 空间 O(1) 额外
///
/// # 契约（与 std slice::sort_unstable 同级）
/// - `T: Ord` 必须是全序（cmp 自反、反对称、传递）。非全序比较器下
///   哨兵逻辑可能失效，行为为「静默产出无意义结果」。触发越界跑飞需要
///   **非自反**（cmp(x,x) != Equal，比「非传递」更窄——石头剪刀布型
///   不触发）。Gen 51 前非自反输入曾构造出 hoare 左扫描越界 panic 见证
///   （公开 API，n >= CUTOFF+1，安全索引边界检查非 UB）；Gen 51 已给
///   双扫描加显式边界：合法 Ord 下哨兵先触发、零行为变化，非自反输入
///   下不再 panic/挂起，多重集守恒。
/// - 若 `cmp` panic：drop guard 保证数组仍恢复为合法排列（panic 后
///   每元素恰好存活一次，可安全 drop）；不再继续排序。
pub fn quicksort<T: Ord>(arr: &mut [T]) {
    if arr.len() > 1 {
        // 深度预算：3*log2(n)。正常输入 balanced 分割只用 log2(n)，永远碰不到；
        // 病态/对抗输入耗尽后触发 heapsort fallback，保证最坏 O(n log n)。
        // 常数取 3 而非经典 Musser 的 2：2 在「持续轻度不均衡」的输入
        // （实测 organ-pipe 分区树深 ~25~26 层）上会过早触发 fallback。
        let depth_budget = 3 * (usize::BITS - arr.len().leading_zeros()) as usize;
        quicksort_rec(arr, depth_budget);
    }
}

/// 洞式插入的 Drop guard（Gen 49 soundness 修复）：`saved` 是从洞位
/// ptr::read 取出的值（ManuallyDrop，绝不主动 drop），guard 析构时把它
/// 写回当前洞位。
///
/// 为什么必须存在：洞式插入进行中被 shift 过的元素，其 ghost 副本仍留在
/// 已腾空的槽位里；若用户的 `Ord::cmp` 在此时 panic，unwind 会照长度 drop
/// 整个数组 —— 每个被 move 过的元素的 ghost 副本与 real 副本各被 drop
/// 一次（对 `T: Drop` 即 double-free），而 saved 值泄漏零次。guard drop
/// 在 panic 路径上把 saved 写回洞位、填上最后的 ghost 槽，恢复「数组每
/// 元素恰好存活一次」。正常路径上 guard drop 就是插入落位本身。
struct InsertHole<'a, T: Ord> {
    arr: &'a mut [T],
    pos: usize,
    saved: std::mem::ManuallyDrop<T>,
}

impl<'a, T: Ord> InsertHole<'a, T> {
    /// SAFETY: `pos < arr.len()`；`arr[pos]` 的值被移入本地 `saved`，
    /// 调用方须通过 `shift` 推进洞位或让 guard drop 落位。
    unsafe fn new(arr: &'a mut [T], pos: usize) -> Self {
        // SAFETY: pos < arr.len()
        let saved = std::mem::ManuallyDrop::new(unsafe { std::ptr::read(arr.as_ptr().add(pos)) });
        Self { arr, pos, saved }
    }

    /// 洞位前一个元素是否大于洞中待插值（`arr[pos-1] > saved`）。
    fn should_shift(&self) -> bool {
        debug_assert!(self.pos > 0);
        // SAFETY: 调用方保证 pos > 0
        unsafe { *self.arr.as_ptr().add(self.pos - 1) > *self.saved }
    }

    /// 把 `arr[pos-1]` memmove 进洞位，洞位前移一格。
    ///
    /// SAFETY: `pos > 0`
    /// 把 `arr[pos-1]` memmove 进洞位，洞位前移一格。
    ///
    /// SAFETY: `pos > 0`。指针派生纪律（Gen 50 修，外部评估 Miri 发现）：
    /// 源与目标指针必须从**单一** `as_mut_ptr()` 派生——若先取 `as_ptr()`
    /// 再在同表达式里取 `as_mut_ptr()`，实参从左到右求值会让可变 retag
    /// 作废共享 tag，随后的读即 Stacked Borrows UB。
    unsafe fn shift(&mut self) {
        let pos = self.pos;
        let base = self.arr.as_mut_ptr(); // 单一 unique retag，派生读写两端
        // SAFETY: pos > 0，pos < len
        unsafe { std::ptr::copy(base.add(pos - 1), base.add(pos), 1) }
        self.pos -= 1;
    }
}

impl<T: Ord> Drop for InsertHole<'_, T> {
    fn drop(&mut self) {
        // 正常路径：写入最终洞位 = 插入落位。panic 路径：恢复不变量。
        // SAFETY: pos 始终在 [0, len) 内（构造时 pos = i < len，只递减）
        unsafe {
            std::ptr::copy_nonoverlapping(
                &*self.saved as *const T,
                self.arr.as_mut_ptr().add(self.pos),
                1,
            );
        }
    }
}

/// 小分区收尾：洞式插入排序（移位从交换式的 3 次移动降为 1 次 memmove）。
///
/// SAFETY：读出的值由 guard 的 ManuallyDrop 持有；guard drop 时写回洞位，
/// 即使中途 panic（unwind）也恢复「每元素恰好存活一次」——见 InsertHole。
fn insertion_sort<T: Ord>(arr: &mut [T]) {
    for i in 1..arr.len() {
        // SAFETY: i >= 1；洞位由 guard 收尾
        unsafe {
            if *arr.as_ptr().add(i) < *arr.as_ptr().add(i - 1) {
                let mut hole = InsertHole::new(&mut *arr, i);
                while hole.pos > 0 && hole.should_shift() {
                    // SAFETY: should_shift 已确认 pos > 0
                    hole.shift();
                }
            }
        }
    }
}

/// partial insertion sort（两段式）：先纯比较扫描数「下降沿」，
/// 超过 PARTIAL_INSERTION_LIMIT 立即返回 Some(第九个下降沿的位置)
/// （乱序输入 ~2*LIMIT 次比较退场，不碰任何交换）；确认近乎有序后才
/// 插入排序排完返回 None。disorder == 0 时扫描本身已证明数组非递减，
/// 直接跳过插入（all-equal / sorted 的顶层完成路径 1 趟比较）。
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
    if disorder > 0 {
        insertion_with_budget(arr)
    } else {
        None
    }
}

/// 带移位预算的洞式插入：disorder <= LIMIT 时执行，全程数总移位，
/// 超过 budget 中途放弃（guard drop 已把洞位填上、数组仍合法排列，
/// 返回 Some(i) 走分区路径）。
///
/// 为什么必须数移位：「两段有序」输入（[升序 run | 升序 run]）只有 1 个
/// 下降沿但插入位移 O(n^2/4)，实测 n=100k 要 259ms（应为 ~0.3ms）。
#[inline(never)]
fn insertion_with_budget<T: Ord>(arr: &mut [T]) -> Option<usize> {
    // 预算 n/8+32 —— 对「少量错位元素」足够排完；对「整段位移型」中途放弃。
    let budget = arr.len() / 8 + 32;
    let mut shifts = 0usize;
    let n = arr.len();
    for i in 1..n {
        // SAFETY: i >= 1；洞位由 guard 收尾（含 panic 路径）
        unsafe {
            if *arr.as_ptr().add(i) < *arr.as_ptr().add(i - 1) {
                let mut hole = InsertHole::new(&mut *arr, i);
                while hole.pos > 0 && hole.should_shift() {
                    // SAFETY: should_shift 已确认 pos > 0
                    hole.shift();
                    shifts += 1;
                    if shifts > budget {
                        // 放弃：guard drop 回填洞位，数组仍是合法排列
                        return Some(i);
                    }
                }
            }
        }
    }
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

/// 三比较把首/中/尾排成 a[0] <= a[mid] <= a[hi]。Hoare 分区专用
/// （其哨兵不变量要求 pivot 在 mid 且两端有序）。
fn median_of_3_sort<T: Ord>(arr: &mut [T]) {
    let len = arr.len();
    sort3(arr, 0, (len - 1) / 2, len - 1);
}

/// Ninther 阈值：len >= 此值用九采样中位的的中位，否则中位三。
/// ninther 把 pivot 秩次方差再压一档：random 1.26 -> ~1.10 n log n
///（比较与交换次数同降 ~13%）。
const NINTHER_MIN: usize = 64;

/// 逆序检测（Gen 21，ipnsort 同款）：整段非严格递减则反转成升序、返回
/// true。只在「下降沿密集」信号下调用；假阳约 2 次比较即失败。
///
/// #[inline(never)]：冷路径外描，避免增大 quicksort_rec 主体导致热循环
/// 代码布局位移（实测：布局位移可让 random 1M 劣化 ~11%）。
#[inline(never)]
fn try_reverse_sorted<T: Ord>(arr: &mut [T]) -> bool {
    let n = arr.len();
    let base = arr.as_ptr();
    let mut i = 1usize;
    // SAFETY: 条件先查 i < n 再解引用；i-1 >= 0
    unsafe {
        while i < n && *base.add(i) <= *base.add(i - 1) {
            i += 1;
        }
    }
    if i < n {
        return false;
    }
    arr.reverse();
    true
}

/// 为 branchless Lomuto 选 pivot：len < 64 中位三；否则 Tukey ninther
/// （3 组三采样各取中位，再对 3 个中位取中位）。pivot 值放到 mid 位。
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
/// 返回切分点 k = j+1：arr[..k] <= pivot，arr[k..] >= pivot（等值元素可落
/// 任两侧；哨兵不变量 a[0] <= pivot <= a[hi] 在严格弱序下保证终止）。
/// Gen 51：双扫描加显式边界 i < hi / j > 0（防非自反 Ord 跑飞）。
///
/// 非全序 `Ord` 下哨兵不变量可能失效，左扫描可越过 pivot 位跑到 len ——
/// 用的是安全索引，结果是 panic 而非内存不安全。
///
/// 定位为「逆序数据专家」：逆序输入下 Hoare 分区产出的左区恰好有序，
/// 可被 partial insertion sort 免费接住（近线性）；而 branchless Lomuto
/// 的左区是逆序、需全额递归。
fn hoare_partition<T: Ord>(arr: &mut [T]) -> usize {
    let hi = arr.len() - 1;
    let mut i = 1usize; // a[0] <= pivot，左扫描从 1 开始（pivot 位天然挡住越界）
    let mut j = hi - 1; // a[hi] >= pivot，右扫描从 hi-1 开始
    let mut p = hi / 2;

    loop {
        // i < hi / j > 0 边界（Gen 51）：合法 Ord 下哨兵先触发、永不生效；
        // 非自反 Ord（cmp(x,x)==Less，pivot 位失去阻挡）下把越界 panic 从
        // 「可能」变成「不可能」。零行为变化、零正确性影响。
        while i < hi && arr[i] < arr[p] {
            i += 1;
        }
        while j > 0 && arr[j] > arr[p] {
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

/// 分支免费版 Lomuto 分区（Gen 9 branchless 化；Gen 12 裸指针去边界检查）。
/// 调用方须先做 lomuto_pivot/median_of_3_sort，pivot 取中位值 = a[mid]。
///
/// 做法：每步「无条件 swap(i, j) + 条件自增 i」——比较结果只走 cmov 不进分支：
/// - arr[j] < pivot：正常 Lomuto，小于者换到左段，i 前进
/// - arr[j] >= pivot：swap(i, j) 交换的是两个都 >= pivot 的元素，无副作用，
///   i 不动 —— 不变量 [0, i) < pivot、[i, j+1) >= pivot 保持
///
/// 免 Clone：pivot 值 ptr::read 进 ManuallyDrop 本地副本（绝不 drop，
/// 泛型 T 可能有 Drop 实现，避免双重 drop）。
///
/// 返回切分点 i：arr[..i) < pivot，arr[i..] >= pivot（等值归右段——
/// 全等/重复密集时 i 趋 0，会被调用方的失衡检测捕获并升级 DNF）。
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
        // SAFETY: 不变式 i <= j < len（i 每轮至多追平 j、从不反超）
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
/// 返回 (lt, gt)：arr[..lt) < pivot，arr[lt..=gt] == pivot（已就位），
/// arr[gt+1..) > pivot。
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
            debug_assert!(gt > 0); // 不变量（Gen 51）：gt 递减到 p 时 pivot 已被换到 i 位、比较转相等而退出，合法 Ord 下不可达
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

/// 强模式粉碎：退化分区专用 —— pivot 恰为区间最小/最大值时
/// （一侧 <= len/64），分区每层只剥 1~2 个元素，弱交换（2 对）打不碎
/// 「99% 两段有序」的 organ-pipe 结构。改用分块轮换：首四分之一与末四分之一
/// 整块互换（len/4 对 swap，O(len)），两段式结构被拦腰打碎。
/// 索引只依赖长度不依赖数据；小侧 len < 128 自动 no-op（自限）。
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
/// 右阈 `>= 9`（Gen 51）：右切片 `arr[gt+1..]` 长度 = len-gt-1，与守卫值
/// `len-gt` 差 1，取 `>= 9` 是守卫与语义的精确对齐（slice len >= 8）。
/// 旧的 `>= 8` **本就行为正确**（切片长 7 时 break_patterns 内部 no-op），
/// 仅为多调用一次必然 no-op 的函数——是精度对齐，不是修 bug。
fn break_patterns_sides<T>(arr: &mut [T], lt: usize, gt: usize) {
    if lt >= 8 {
        break_patterns(&mut arr[..lt]);
    }
    if arr.len() - gt >= 9 {
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
        // 粗糙数据预筛：n >= 128 才付费采样（小切片完整扫描本来就 <=17 次）。
        // 8 采样点 7 个相邻对 2~6 下降沿 -> 粗糙，跳过下降沿扫描直达
        // Lomuto+ninther（random 的 bail_pos ~17 次扫描/节点纯属浪费）。
        let k = if len >= 128 && route_direct_lomuto(arr) {
            depth_budget -= 1;
            lomuto_pivot(&mut *arr);
            branchless_partition(&mut *arr)
        } else {
            // 平滑/逆序密度：走完整路径（partial insertion 的下降沿扫描 +
            // 近乎有序完成 + 三档路由 + 逆序检测 + 降序游程精化）
            let Some(bail_pos) = partial_insertion_sort(arr) else {
                return;
            };
            depth_budget -= 1;
            let Some(k) = partition_router(&mut *arr, bail_pos) else {
                return; // 整段逆序，已反转成升序
            };
            k
        };
        let left_len = k;
        let right_len = len - k;

        if left_len < len / UNBALANCED_DIV || right_len < len / UNBALANCED_DIV {
            // 坏分区升级通道：重复密集数据在此现身（pivot 是常见值 ->
            // 一侧近乎为空）。整段重跑 DNF，把 ==pivot 中段冻结，
            // 递归只处理 < / > 两侧。
            let (lt, gt) = dnf_partition(&mut *arr);
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
                    break_patterns_sides(&mut *arr, lt, gt);
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
        // 直接压测升级通道：pivot 恒为常见值 -> Hoare 必失衡 -> DNF 冻结等值段
        let mut v: Vec<u32> = (0..10_000).map(|i| (i % 3) as u32).collect();
        quicksort(&mut v);
        assert!(is_sorted(&v));
        assert_eq!(v.len(), 10_000);
    }
}
