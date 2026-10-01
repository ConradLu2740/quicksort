# quicksort_Gen49 独立第三方评估报告

> 评估者：独立 AI 评审（ZCode）。评估对象：`quicksort_Gen49.rs`（2026-10-01，Gen 49）及随附评审包。
> 环境：rustc/cargo 1.96.0（与作者声明一致），x86_64-pc-windows-gnu（stable），另有 nightly-msvc 用于 Miri。
> 评估副本：`C:\ai-workspace\zcode\qs-eval\`（对原文件做了两处标注改动，见附录 A；`C:\ai-workspace\kimi\sort\share\` 下的原文件未动）。

---

## 0. 总评（TL;DR）

**这是一份工程质量明显高于平均水平的实现，其核心安全声明经独立实证成立，但 Miri 抓到一个真实的别名模型违规（一行可修），且两项关键性能声明（总体 ~0.97x 打平、few-unique 反超）未能在独立分布集下复现，"random 落后是类型特化差距"的归因不成立。**

| 维度 | 结论 |
|---|---|
| 正确性 | **通过**。~990 个差分配置（10 分布 × 32 尺寸 × 3 种子 × 多类型）与 `sort_unstable` 全等 |
| panic 安全 | **通过**（原生压测 7272/7272 注入点：每元素恰好 drop 一次 + panic 后数组为合法排列） |
| 内存安全 | **一项真实缺陷**：`InsertHole::shift` 违反 Stacked Borrows（Miri 实测），修复一行后 Miri 全绿 |
| 最坏 O(n log n) / 栈深 ≤ log2 n | **成立**（分析验证：预算约束 + 小侧递归 + heapsort 叶子质量不相交） |
| 性能 | random 落后 **1.85–1.94x**（与作者自述一致）；few-unique 落后 **2.0–2.2x**（与作者"反超"声明相反）；two_run **领先 2.5–3.1x**；总体几何平均落后 **~1.20x**（12 配置 @1M） vs 声明的 0.97x |
| 诚实度 | 评审包的自我披露经核对基本属实，且实测确实踩中了它自己披露的"未跑 Miri"盲区 |

---

## 1. 正确性 —— 通过

差分测试（`tests/differential.rs`）：10 种分布（random / few-unique(4) / two-distinct / sorted / reverse / all-equal / organ-pipe / nearly-sorted(1%) / two-run / sawtooth）× 32 个尺寸（0–10 000，含全部路由阈值边界 31/32/33/63/64/65/127/128/129）× 3 种子，与 `sort_unstable` 结果全等；另覆盖 `String`（堆类型）、带 Drop 计数的非 Copy 类型（无泄漏、多重集不变）、ZST、derive-Ord 包装类型。大数组（5 万 / 20 万）随机补测通过。**未发现任何错误。**

复杂度声明的分析验证：

- **最坏 O(n log n) 成立**。小侧递归使任意根-叶路径的分区数 ≤ 3·log2(n)（预算），每层代价 O(m)；heapsort 叶子的质量互不相交，合计 ≤ O(n log n)；叶级插入排序合计 O(CUTOFF·n)。即使模式粉碎失效（对抗输入），depth budget 也兜底。
- **栈深 ≤ log2(n) 成立**。两条递归路径（均衡分区递归较小侧；坏分区递归 DNF 较小侧）递归侧质量都 ≤ len/2。
- 分区器不变量逐一核验：Hoare（pivot 位置跟踪）扫描双向有阻挡元素、k=j+1 ∈ [1, len-1] 保证进展；branchless Lomuto 的 i ≤ j 不变量不依赖比较器合法性（i 每轮至多追平 j）；DNF 的 pivot 位置跟踪在 p==lt/p==gt 等角点正确；`quicksort_rec` 的三个出口（完成/反转/DNF 全等）均正确终止。

## 2. 内存安全与 panic 安全 —— 一项真实缺陷（Miri 发现），修复已验证

### 2.1 原生 panic 压测（`src/bin/panicstress.rs`）

带堆载荷（`Box<u8>`）+ drop 计数的元素类型，比较器在注入点 panic。**7272/7272 通过**：6 分布 × 12 尺寸（2…5000）× 每配置 100 个注入点（先数出该输入的真实比较总数，再在其范围内注入，保证 panic 真的落在各代码路径上——覆盖洞式插入、Hoare、branchless Lomuto、DNF、heapsort、ninther 全部比较点）。每例断言：① panic 后数组仍是输入多重集的合法排列；② 每元素恰好 drop 一次（无 double-free、无泄漏）。

### 2.2 Miri 发现（本评估的核心新发现）

Miri（nightly 2026-09-30，Stacked Borrows）在 `InsertHole::shift` 报告未定义行为：

```
error: Undefined Behavior: attempting a read access using <152417> at alloc48133[0x18],
but that tag does not exist in the borrow stack for this location
  --> src\lib.rs:194  std::ptr::copy(
          self.arr.as_ptr().add(pos - 1),   // 先创建 SharedReadOnly
          self.arr.as_mut_ptr().add(pos),   // 函数入口 Unique retag 使其失效
          1)                                 // 再经失效指针读 → UB
```

同一表达式内先取 `as_ptr()`（共享只读）再取 `as_mut_ptr()`（唯一可变），后者使前者失效。评审包自述"未跑过 Miri"——这正是该盲区里的一个真实命中。**定性**：属于实验性别名模型（Stacked Borrows）违规，当前编译器无实际 miscompile 证据（原生测试全绿），但"与 `slice::sort_unstable` 同级"的契约意味着要与 std 同一门槛——std 是过 Miri 的。

**修复（一行，已在我的评估副本上验证）**：

```rust
unsafe fn shift(&mut self) {
    let pos = self.pos;
    let base = self.arr.as_mut_ptr();          // 单一指针派生读写
    unsafe { std::ptr::copy(base.add(pos - 1), base.add(pos), 1) }
    self.pos -= 1;
}
```

修复后 Miri 全绿：panic 注入 240 点 × 6 尺寸（drop 恰好一次 + 排列恢复，Miri 直接抓 double-free/泄漏）、三类非全序比较器（恒 Less / 伪随机 / 石头剪刀布循环，n ≤ 100，无 UB）、小规模全分布冒烟。仓库其余 unsafe（`branchless_partition` 先取一次 `base` 复用、`try_reverse_sorted` 只读扫描、pivot 的 ManuallyDrop peek、InsertHole guard 本体）经审查均无此模式，Miri 亦未再报。

### 2.3 非全序 `Ord`

与声明一致：最坏是 `hoare_partition` 的边界检查 panic（`arr[i]` 可越过 pivot 位跑到 len），不产生内存不安全；`branchless_partition` 的裸指针不变量不依赖比较器合法性。**实测（含 Miri）无 UB。**

另注：Gen49 的 panic 契约（"恢复为合法排列"）**强于** std（std 只保证不 UB、不泄漏，不保证顺序），且实测兑现了。

## 3. 性能 —— 部分声明确认，两项反转

协议：预热 2s；每配置内 ours/std 严格交替 9 轮取中位比值；配置块顺序随机化；结尾复测校验漂移（random 1e6：首测 1.885 vs 复测 1.905，稳定）。默认 release 配置（未开 target-cpu=native），双方同旗标。u64，n=1e6，ratio = ours/std（<1 为 Gen49 胜）：

| 分布 | ratio | 对照作者声明 |
|---|---|---|
| two_run | **0.322** | 未强调（这是最大的赢项） |
| nearly_sorted | 0.966（@10k 为 0.76–0.96，小规模真赢） | ✓ "反超"部分成立 |
| reverse | 0.959 | ✓（打平偏胜） |
| sorted | 0.71–1.28（跨 run 波动，判平） | ~ |
| all_equal | 1.12 | ~ |
| sawtooth | 1.210 | 未声明 |
| organ_pipe | **1.451**（@100k 1.80） | 未声明为胜项；专用机制未追平 std |
| few_unique(4) | **1.975**；uniq2 **2.171**；uniq16 1.808；uniq256 1.498 | ✗ **与"few-unique 反超"相反** |
| random | **1.885**（100k：1.94；10M：1.85） | ✓ 与"约 0.5x"一致 |
| **几何平均（12 配置 @1M）** | **1.201** | ✗ 声明 "~0.97x 打平" |

（规模联测 n=1e3/1e4 因进入微秒区，比值噪声大，不作为结论依据；100k 起数字稳定。）

**关于"0.97x 打平"与"few-unique 反超"**：我的分布集与作者的 23 分布套件不可比，不排除其套件含大量 Gen49 占优的结构化分布而拉低总几何平均；但在独立选择的 12 配置集上，打平声明不成立，且 few-unique 方向相反（重复度越低越好、越高越差：2 值 2.17x）。若作者坚持该声明，需要公开其分布定义与原始数据以供复现——评审包刻意自包含却未附基准数据，这是可复核性的缺口。

**机制归因（用计数器实测 + std 源码核对）**：

- random：50 504 次分区 / 1M 元素 ≈ 2n/32，**完全健康**；DNF 仅 2.7%。1.85x 差距是纯每节点常数差。std 1.96 的主分区是 `partition_lomuto_branchless_cyclic`（`library/core/src/slice/sort/unstable/quicksort.rs:249`，`MAX_BRANCHLESS_PARTITION_SIZE=96` 按 `size_of::<T>` 分派）：**cyclic/gap 写法每元素至多一次写**，而 Gen49 的无条件 swap 每元素 2 读 2 写——写流量约两倍。作者自己的论点"大数组瓶颈是内存带宽"恰恰预言了这一点。
- few-distinct：uniq2 全程只有 **3 次分区、0 次 DNF**，few_unique(4) 6 次分区——劣势不是坏分区重扫，而是 Gen49 缺少 std 的等值快速完成路径（std 借等值检测近乎一趟收工，Gen49 根分区后还要每侧整扫，~3n vs ~n）。

**"特化差距"归因不成立**：std 的主分区与小分区阈值分派只依赖 `size_of::<T>`（≤96B branchless、≤16B 双路展开），u64 与 8 字节手写 Ord 的 NewU64 在同一交替协议下无稳定差异。差距来自算法常数（写侧流量、等值路径），不是类型特化。作者在声明 4 里"纯泛型实现的常数差距部分是特化差距"的自我开脱，实测只占很小份额。

**BlockQuicksort 五次失败的归因**：对"瓶颈是带宽/净指令数而非交换 ALU"的判断正确；且 Rust std（ipnsort 一系）同样放弃了 block partition 转向 cyclic branchless——作者的实测失败与 std 的设计演进互相印证，这部分归因可信。**但推不出"random 0.5x 无解"**：正解是换掉无条件 swap 的写侧模式（cyclic 分区同为泛型、原地、零分配、无特化），而不是继续在 prefetch/blocking 上打转。

## 4. 对评审包五个委托问题的直接回答

1. **random 0.5x 是"分布自适应 vs 常数特化"的取舍吗？** 不是。框架本身错位：std 的领先主要来自泛型可用的 cyclic 分区与等值路径，不是类型特化；分布自适应的路由（two_run、nearly-sorted 的收益为证）与这两项改进不冲突。可解，且不需要放弃任何设计约束。
2. **BlockQuicksort 归因可信吗？** 一半可信。"瓶颈在带宽"正确；"五次失败 → 方向已死"过强——std 也有先例放弃 block partition，但那是转向了 cyclic，不是退回 branchy。作者失败的真实原因很可能是基线分区本身写流量过大，block 化救不了写侧。
3. **7 条路由的复杂度值得吗？** 按我的分布集：不值得（总体落后 1.20x，且 std 用更少的路径做得更好）。按作者的分布集：无法判断——其套件未随包公开。two_run 0.32x 和 nearly-sorted@10k 的收益证明路由机制有真实价值，但 current evidence 下，更简单的 std 式结构（partial insertion + 单一 cyclic 分区器 + 等值处理 + heapsort 兜底）是更优的复杂度/收益点。
4. **InsertHole guard 是否消除了所有 panic 路径 UB？** panic 路径本身：逻辑推演 + 7272 例原生压测 + Miri 240 例注入，全部成立，guard 设计正确。但"所有 unsafe 面无 UB"不成立——`shift` 的 Stacked Borrows 违规与 panic 无关，是别名模型问题，恰在 guard 内部。修一行后干净。
5. **各参数在当前二进制上是否仍最优？** 无法证伪（这是他们自己的架构内的局部最优），但需指出：参数扫描全部在自家变体之间比较，是"在错误基线上找局部最优"。例如 CUTOFF=32 的叶代价 O(32²) 次移动，对比 std 特化小排序（Copy 类型排序网络/特化插入）天然偏贵；这类结构性差距不是调参能弥补的。

## 5. 其他代码质量发现（次要）

- `hoare_partition` 文档注释 "arr[k..] > pivot" 应为 "≥ pivot"（等值元素可落在右段）；实现正确，文档不准。
- `break_patterns_sides` 右侧判据 `arr.len() - gt >= 8` 实为"右段 ≥ 7"，与左侧 `lt >= 8` 不对称（内部 `break_patterns` 的 len<8 兜底使它无害，纯小疵）。
- `try_reverse_sorted` 的 raw 指针扫描是不必要的 unsafe（安全索引即可，性能无差）。
- 随包附带的 `#[cfg(test)]` 只有 3 个测试，与"49 代、每代有数据"的宣称相比，单文件的可复核性偏薄（作者说明完整仓库另有测试，本包自包含原则下应有更厚的测试层）。
- 注释密度、SAFETY 论证质量、参数演化记录：**显著高于平均水平**，这在同类"AI 生成+迭代"项目中少见。

## 6. 结论与建议

**结论**：算法正确性成立，panic 契约真实兑现且强于 std，理论复杂度声明全部成立；存在一个真实但低危、一行可修的 Miri 级缺陷；性能总体落后 std 约 20%（几何平均，独立分布集），落后的两大来源（写侧交换模式、等值快速路径缺失）都有明确、不违背作者设计约束的修法；"总体打平"与"few-unique 反超"两项声明当前不可复现。

若继续演进，按性价比排序：
1. **必修**：`shift` 单指针化（本文 §2.2，已验证）。
2. **高收益**：branchless 分区改 cyclic/gap 写法；为重复密集数据加等值快速完成路径。预计可收窄 random 与 few-distinct 的大部分差距，且不触碰任何路由机制。
3. **流程**：Miri 进 CI（stable 上不可用，需 nightly 组件）；评审包随附分布定义 + 原始基准数据，使 0.97x 声明可复核。
4. **文档**：修正 Hoare 注释的等值归属描述。

## 附录 A：评估副本的改动清单

对 `C:\ai-workspace\kimi\sort\share\quicksort_Gen49.rs` 的唯一算法性改动是 §2.2 的一行修复（Staked Borrows 违规），其余为评估设施，均不影响被测语义：
- 基准与压测在**修复后**的副本上运行（该修复仅改变指针派生方式，memmove 语义相同，性能影响在噪声内）；
- cfg(feature="eval-counters") 门控的分区/DNF 冷路径计数器（§3 机制归因用，默认不编译）。

## 附录 B：方法与局限

- 差分/压测/基准均使用确定性种子，可复现；评测 crate 位于 `C:\ai-workspace\zcode\qs-eval\`。
- Windows 定时器精度限制了对 n ≤ 10⁴ 的比值结论（已从结论中剔除）；会话级漂移用随机化块序 + 首尾复测控制（±1%）。
- 未开 target-cpu=native；不同旗标可能改变绝对比值（双方同旗标，配对比较仍公平）。
- Miri 依据的 Stacked Borrows 仍是实验模型；但"与 std 同级"的契约应按同一工具链标准检验。
- 我的分布集与作者 23 分布套件不可比；本报告只声明"在独立分布集 X 下的结果"，不声称作者数据造假。
