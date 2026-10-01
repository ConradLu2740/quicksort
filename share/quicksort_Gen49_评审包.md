# quicksort Gen 49 —— 评估委托包

> 用途：把「最终版快排」交给另一个 AI agent 独立评估。包含三部分：
> **A. 背景与诚实声明**、**B. 完整源码**（单文件，零依赖）、**C. 建议的验证方法**。

---

## A. 背景与诚实声明

**这是什么**：一个泛型 `T: Ord` 的原地、不稳定、零分配快速排序，Rust 实现。
仓库是「快排进化论」项目：从教科书朴素版起迭代 49 代，每代记录改动/数据/原因，
最终与 Rust 标准库 `slice::sort_unstable` 对标的 23 个基准分布几何平均 **~0.97x**
（总体打平），其中 nearly-sorted / reverse / few-unique / sorted 等分布反超，
**random 100k/1M 约 0.5x（落后一半）是已知的公开弱项**——这不是遗漏，作者认为
"分布自适应 vs 常数特化"是设计取舍，请把结论是否认这一点作为评估题之一。

**作者方已知的设计取向**（可能带偏见，请独立判断）：
- pdqsort/ipnsort 族机制 + 自己的信号路由（粗糙预筛三态 × partial insertion
  两段式 × 双分区器 × 坏分区 DNF 升级 × 退化分区强粉碎 × 深度预算 heapsort）
- 每个参数（CUTOFF=32 / NINTHER_MIN=64 / PARTIAL_LIMIT=8 / UNBALANCED=8 /
  SPARSE=48 / DEPTH_CONST=3）都经过多轮扫描确认，非默认值拍脑袋
- 5 次尝试 BlockQuicksort 方向均实测更差（归因：大数组瓶颈是内存带宽/净指令数，
  不是交换 ALU）——该归因是否可信也是一个评估点

**诚实声明（已知缺陷与边界，非遗漏）**：
1. panic 路径：曾被发现 cmp panic 时洞式插入 double-free（对 Drop 类型），
   已用 InsertHole guard 修复并有回归测试；但 **未跑过 Miri**。
2. 非全序 `Ord`：可能 panic 不 UB，与 std 同级，未加哨兵断言。
3. 复杂度与可维护性：热路径单函数 + 7 条路由路径，改动者须通读路由决策；
    这是用复杂度换来的分布适应性。
4. 泛型通用性：无 Copy/size_of 特化，std 对 ≤96B 类型用 branchless Lomuto+
   循环置换、小 Copy 类型用排序网络——**纯泛型实现的常数差距部分是特化差距**。
5. `panic = "abort"` 只影响作者自测的 workspace，不会传导给下游。

**测量可靠性声明**：作者记录过 "编译后立即测 bench" 会造成 ~17% 的相位污染
假回退，正确做法是同 session 交替 A/B。评估测得的绝对数字请以此为戒。

**已知的评估盲区**（作者做过第三方测评但请勿直接采信）：
- 审计发现并已修复：panic 路径 double-drop（已有红→绿回归测试）
- 已量化并保留：panic=abort 让自测数字有利己偏差（+3.3% @1M / +4.9% 总分）
- 未处理：计时顺序不交替、black_box 粒度过粗、无 Miri、无 license、
  非 no_std、`inputs` 脚手架挂在公共 API（已 doc(hidden)）

---

## B. 完整源码

**优先：直接发送同目录下的 `quicksort_Gen49.rs`** —— 它就是最终版全量代码
（单文件、零外部依赖、`cargo build` 即可编译、保留全部设计注释与 SAFETY 论证）。

如果发送渠道只支持粘贴一段文本，把该 `.rs` 文件全文粘贴即可（约 26KB）；
文末「C. 验证方法」自带最小差分测试骨架。

---

## C. 建议的验证方法

### C1. 正确性（差分测试，最小编码量）
```rust
fn main() {
    for n in [0usize, 1, 2, 3, 4, 5, 31, 32, 33, 63, 64, 65, 127, 128, 129,
              1000, 10_000, 100_000] {
        let mut v: Vec<u64> = (0..n).map(|i| (i.wrapping_mul(2654435761) % 997) as u64).collect();
        v.sort_unstable();
        quicksort(&mut v);
        assert!(v.windows(2).all(|w| w[0] <= w[1]), "random n={n}");
    }
    // 分布：sorted / reverse / all-equal / few-unique / organ-pipe / two-run / 锯齿波
}
```

### C2. panic 安全（该实现的核心声明）
实现一个会 panic 的 `Ord`（如某元素比较时 panic）+ 一个记 drop 次数的
`Drop` 类型，`catch_unwind` 后断言：**每个 id 恰好被 drop 一次**。
（本仓库 tests/panic_safety.rs 有两个已验证用例，但请独立重写验证。）

### C3. 非全序安全性
用 NaN 包装型或不传递的 cmp，断言「顶多 panic，不 UB」（可配合 Miri）。

### C4. 性能建议（如做基准）
- 对照：`slice::sort_unstable` 同机器同编译器；注意 Rust 1.81 后它是 ipnsort
- 必须交替测量（ours/std 交替、每侧多轮取中位）；禁止编译后立即测量
- 建议规模点 n = 1e3/1e5/1e6/1e7 × 分布：random / sorted / reverse /
  all-equal / few-unique / organ-pipe / nearly-sorted / two-run

### C5. 可重点质疑的方面
1. random 大数组 0.5x——「设计取舍」的说法是否成立？真无解吗？
2. BlockQuicksort 五次失败的归因（带宽/净指令数）是否符合学术共识？
3. 7 条路由的复杂度是否值得？是否存在更简单的同性能结构？
4. InsertHole guard 是否真的消除了所有 panic 路径 UB？还有别的 unsafe 面吗？
5. 各参数（CUTOFF=32 等）在当前二进制上是否仍然最优？

---

**版本信息**：Gen 49（2026-10-01），rustc 1.96.0，x86_64 Windows。
项目仓库含 49 代完整演化日志（EVOLUTION.md，含死路表与路由地图），
如需可另取——但本委托包刻意自包含，不依赖仓库任何其他文件。
