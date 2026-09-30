# 设计：Rust 快速排序「进化论」

日期：2026-09-30
状态：已批准，执行中

## 目标（Goal）

持续迭代优化 Rust 快速排序实现：每个 goal turn 完成一代有据可查的优化（或负结果分析），全程保持全部正确性测试通过，直到用户喊停。

**完成条件**：用户明确说「停」；在此之前不得主动结束、不得跳过验证。

## 已确认的决策

- 主战场：Rust（cargo 1.96.0 / rustc 1.96.0；本机无 gcc，clang 为 Android 版不适合）
- 靶心：又快又稳 —— 综合基准评分 + 最坏情况稳健，对标 `std::sort_unstable`（pdqsort）
- 模式：开放式 goal，用户喊停才停

## 架构与文件职责

```
Cargo.toml                       # lib + bin(bench)，零第三方依赖
src/lib.rs                       # quicksort 主体，顶部 // GENERATION: N 标记当前世代
src/inputs.rs                    # 输入生成器（Rng + Dist），bench 与 tests 共用同一份
src/bin/bench.rs                 # 判分器：批量计时 + 中位数 + 几何平均进化分
tests/differential.rs            # 正确性门禁：差分测试 vs std sort_unstable
EVOLUTION.md                     # 进化日志：每代改动、动机、前后数据、死路记录
```

零第三方依赖的理由：基准可复现、不依赖网络；计时与统计逻辑本身透明可审。

## 判分器设计（详情）

- 分布 × 规模 = 23 个用例：
  - random：100 / 1k / 10k / 100k / 1M
  - 其余 6 种病态分布（all-equal, few-unique, sorted, reverse, nearly-sorted, organ-pipe）：100 / 1k / 10k
- 病态分布在朴素实现上为 O(n²)，100k 规模会让 Gen 0 跑几分钟以上，因此病态分布规模上限固定为 10k。此上限从 Gen 0 起恒定，保证跨代可比。
- 计时：批量计时（单次排序在小 n 低于 Windows QPC 分辨率）：连续排 k 个一次计时再除以 k，拷贝基线单独测量扣除；每组 4 批次（1 预热 + 3 测量）取中位数。
- 进化分：`geomean(pdqsort_ms / ours_ms)`，23 个用例；计时器分辨率兜底下限 1ns。
- 每轮计时后校验输出有序，乱序直接非零退出。

### 为什么用 organ-pipe 作为「静态 median-of-3 killer」

McIlroy 的 killer adversary（*A Killer Adversary for Quicksort*, SPE 29(4) 1999）是**在线对手**：在排序过程中根据比较结果现场构造输入，需要插桩排序过程，无法固化为静态基准文件。静态场景下，Musser 构型（风琴形 1..k..1）即可让「首/中/尾取中位数」型 pivot 策略恒取极小值、退化到 O(n²) —— 即 organ-pipe 分布。对 pdqsort 这类带模式识别 + 深度限制的实现，任何静态杀手最多逼出 O(n log n) 的常数退化，这正是第 6 代要防御的东西。

## 正确性门禁设计

- 边界：空 / 单元素 / 双元素 / 全等 / 小数组
- 全部 7 种分布 × 10 个规模 × 20 个种子
- 2000 例随机数组，判据：与 `sort_unstable` 输出逐元素相等
- 铁律：`cargo test` 不全绿的世代作废，不允许进基准

## 进化路线图（滚动调整，每 goal turn 推进 1-2 代）

| 代 | 内容 | 预期收益 |
|---|---|---|
| 0 | naive Lomuto + 末元素 pivot + 朴素递归（✅ 基线 0.0222x） | 建立基线 |
| 1 | Hoare 双指针分区 | 交换次数约减半 |
| 2 | median-of-three pivot | 消除 sorted/reverse 退化 |
| 3 | 小数组插入排序 cutoff | 小分区子树开销骤降 |
| 4 | 尾递归消除 + 递归深限 | 栈 O(log n)，最坏 O(n log n) 保证 |
| 5 | 三路分区（Dutch flag） | all-equal / few-unique 不再退化 |
| 6 | pdqsort 风格模式识别 | 有序类输入直接插入排序收尾 |
| 7+ | 低级微优化（无分支分区、预取、get_unchecked 等） | 常数项压缩 |

后续可按需调研引入 ips4o / glidesort / crumsort 等业界思路，作为滚动条目。

## 每代迭代协议（goal turn SLA）

1. 读当前实现 + EVOLUTION.md，选定下一代目标
2. `cargo test` 确认起点绿
3. 实现该代改动（更新 GENERATION 标记）
4. `cargo test` 全绿（否则修复或回滚，死路也记录）
5. `cargo run --release --bin bench`，前后对比写入 EVOLUTION.md
6. 提升 < 1% 判噪声：重测；确认为死路：记录「为何不行」后转向
7. `git commit` 该代

## 边界与风险

- 计时噪声：Windows 后台干扰；对策 = 固定 seed + 批量中位数 + 可疑重测；< 1% 的提升不声称为优化
- `unsafe`：允许，但必须有不变式注释 + 差分测试覆盖
- 不允许：为跑分特化输入、删测试、伪造数据
- 全程零网络依赖
