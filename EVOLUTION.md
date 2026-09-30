# 进化日志

计分规则（长期有效）：

- 每代跑 `cargo run --release --bin bench`，记录 **EVOLUTION SPEED SCORE**（23 个用例几何平均，对标 std pdqsort，越高越好）
- `cargo test` 不全绿的世代作废，不允许进基准
- 提升 < 1% 判噪声，重测；确认为死路的，「为何不行」也要记录
- 每代一个 git commit

## 当前状态

- 世代：**Gen 12**
- EVOLUTION SPEED SCORE：**~0.57x**（三轮 0.561/0.575/0.577，噪声带内持平；random 绝对耗时 -3~5%）
- 正确性：7 个测试全绿

## 分数历史

| 世代 | 分数 | 关键变化 | 日期 |
|---|---|---|---|
| Gen 0 | 0.022178x | 教科书朴素版基线 | 2026-09-30 |
| Gen 1 | 0.137721x | Hoare 分区 + 中间 pivot（6.2 倍提升） | 2026-09-30 |
| Gen 2 | 0.207548x | median-of-three pivot（+51%） | 2026-09-30 |
| Gen 3 | 0.327240x | 插入排序 cutoff=16（+58%） | 2026-09-30 |
| Gen 4 | 0.321654x | introsort 保险（持平，换最坏 O(n log n) 硬保证） | 2026-09-30 |
| Gen 5 | 0.494956x | pdqsort 三件套（DNF + partial insertion + 模式粉碎，+54%） | 2026-09-30 |
| Gen 6 | 0.514827x | 混合分区：默认 Hoare + 坏分区升级 DNF（+4%） | 2026-09-30 |
| Gen 7 | ~0.51x（持平） | 负结果代：等值计数升级 + CUTOFF 扫描，双双数据否决 | 2026-09-30 |
| Gen 8 | ~0.51x（持平） | 负结果代：Hoare 指针化否决（-9%），定位真瓶颈=分支预测 | 2026-09-30 |
| Gen 9 | 0.604114x | 双分区 + 下降沿信号：branchless Lomuto × Hoare（+16%） | 2026-09-30 |
| Gen 10 | ~0.59x（持平） | 信号扩三档，nearly-sorted 绝对耗时 -51% | 2026-09-30 |
| Gen 11 | ~0.58x（持平） | 退化分区强粉碎：organ-pipe 绝对耗时砍半 | 2026-09-30 |
| Gen 12 | ~0.57x（持平） | branchless 分区裸指针化：random 绝对耗时 -3~5% | 2026-09-30 |

## Gen 0：教科书朴素版（基线）

**改动**：Lomuto 分区 + 末元素 pivot + 朴素递归。故意保持最简，建立整条进化曲线的起点。

**基准数据**（ours vs pdqsort，ms，speedup = pdqsort/ours）：

| distribution | n | ours | pdqsort | speedup |
|---|---|---|---|---|
| random | 100 | 0.0003 | 0.0003 | 0.992x |
| random | 1k | 0.0049 | 0.0050 | 1.019x |
| random | 10k | 0.3985 | 0.0675 | 0.169x |
| random | 100k | 4.9818 | 0.9054 | 0.182x |
| random | 1M | 52.1792 | 10.6014 | 0.203x |
| all-equal | 100 | 0.0020 | 0.0000 | 0.009x |
| all-equal | 1k | 0.1631 | 0.0002 | 0.001x |
| all-equal | 10k | 15.3046 | 0.0021 | 0.000x |
| few-unique | 100 | 0.0005 | 0.0002 | 0.378x |
| few-unique | 1k | 0.0400 | 0.0015 | 0.038x |
| few-unique | 10k | 3.2128 | 0.0132 | 0.004x |
| sorted | 100 | 0.0020 | 0.0000 | 0.009x |
| sorted | 1k | 0.1682 | 0.0002 | 0.001x |
| sorted | 10k | 15.4033 | 0.0021 | 0.000x |
| reverse | 100 | 0.0019 | 0.0000 | 0.012x |
| reverse | 1k | 0.1430 | 0.0003 | 0.002x |
| reverse | 10k | 13.2669 | 0.0024 | 0.000x |
| nearly-sorted | 100 | 0.0012 | 0.0002 | 0.201x |
| nearly-sorted | 1k | 0.1185 | 0.0045 | 0.038x |
| nearly-sorted | 10k | 0.7408 | 0.0639 | 0.086x |
| organ-pipe | 100 | 0.0004 | 0.0003 | 0.675x |
| organ-pipe | 1k | 0.0311 | 0.0046 | 0.149x |
| organ-pipe | 10k | 1.9004 | 0.0677 | 0.036x |

**暴露的问题**：

1. sorted / reverse / all-equal / few-unique 全面 O(n²)：末元素 pivot 在这些输入上恒取到极值，每次分区只剥掉 1 个元素
2. 递归深度 ≈ n：病态输入下 10k 就有 1 万层递归，bench 目前靠 512MB 大栈线程兜底
3. random 大输入也只有 pdqsort 的 ~0.2x：Lomuto 交换开销 + 小分区递归常数

**下一代方向**：Gen 1 上 Hoare 双指针分区（先吃掉最大的常数项），Gen 2 上 median-of-three（消灭病态退化）。

## Gen 1：Hoare 双指针分区 + 中间元素 pivot

**改动**：Lomuto → Hoare 分区；pivot 取 `(len-1)/2` 用「位置跟踪」法（swap 碰到 pivot 位时 p 跟随移动，保证 `a[p]` 恒为初始 pivot 值，泛型 `T: Ord` 无需 `Clone`）。

**动机**：Gen 0 暴露的两大病灶——Lomuto 交换次数多（random 只有 0.2x）；全等/有序输入 O(n²)。

**基准数据**：

| distribution | n | ours(ms) | pdqsort(ms) | speedup | Gen0 speedup |
|---|---|---|---|---|---|
| random | 100 | 0.0004 | 0.0003 | 0.633x | 0.992x |
| random | 1k | 0.0058 | 0.0041 | 0.700x | 1.019x |
| random | 10k | 0.4030 | 0.0554 | 0.137x | 0.169x |
| random | 100k | 5.1136 | 0.9131 | 0.179x | 0.182x |
| random | 1M | 60.1521 | 10.5994 | 0.176x | 0.203x |
| all-equal | 100 | 0.0005 | 0.0000 | 0.040x | 0.009x |
| all-equal | 1k | 0.0066 | 0.0002 | 0.033x | 0.001x |
| all-equal | 10k | 0.0859 | 0.0021 | 0.024x | 0.000x |
| few-unique | 100 | 0.0004 | 0.0002 | 0.494x | 0.378x |
| few-unique | 1k | 0.0066 | 0.0015 | 0.234x | 0.038x |
| few-unique | 10k | 0.0842 | 0.0133 | 0.158x | 0.004x |
| sorted | 100 | 0.0003 | 0.0000 | 0.067x | 0.009x |
| sorted | 1k | 0.0039 | 0.0002 | 0.055x | 0.001x |
| sorted | 10k | 0.0467 | 0.0020 | 0.044x | 0.000x |
| reverse | 100 | 0.0003 | 0.0000 | 0.075x | 0.012x |
| reverse | 1k | 0.0040 | 0.0003 | 0.064x | 0.002x |
| reverse | 10k | 0.0500 | 0.0024 | 0.049x | 0.000x |
| nearly-sorted | 100 | 0.0003 | 0.0002 | 0.877x | 0.201x |
| nearly-sorted | 1k | 0.0039 | 0.0045 | 1.142x | 0.038x |
| nearly-sorted | 10k | 0.0494 | 0.0642 | 1.300x | 0.086x |
| organ-pipe | 100 | 0.0008 | 0.0003 | 0.351x | 0.675x |
| organ-pipe | 1k | 0.0717 | 0.0046 | 0.065x | 0.149x |
| organ-pipe | 10k | 5.2813 | 0.0674 | 0.013x | 0.036x |

**EVOLUTION SPEED SCORE：0.022178x → 0.137721x（6.2 倍）**

**结论**：

1. 全部病态分布从 O(n²) 进入 O(n log n)：all-equal / sorted / reverse / few-unique 提升 4~33 倍
2. nearly-sorted 在 1k/10k 反超 pdqsort（模式识别是 pdqsort 的固定开销，小输入被我们白捡）
3. random 持平（比较次数主导，Hoare 省的是交换不是比较）
4. organ-pipe 仍是 0.013x：中间元素恰为序列最大值，每次只剥 1 个 —— median-of-3 同样救不了（首/中/尾中位数 = 1 = 最小值）。真正的解法是 Gen 4 深限保护或 Gen 6 模式识别

**测量噪声备忘**：同配置重跑，pdqsort 单 Cases 读数有 ±10~20% 波动（如 random 10k 的 std 读数 0.055~0.068ms），但 23 case 几何平均稳定，分数趋势可信。

**下一代方向**：Gen 2 median-of-three pivot —— 进一步压病态分布的分区均衡性（random 大输入收益为主），并为 Gen 3 插入排序 cutoff 铺路。

## Gen 2：median-of-three pivot

**改动**：分区前三比较把首/中/尾排成 `a[0] ≤ a[mid] ≤ a[hi]`，pivot 取中位值；`len ≤ 3` 直接返回（三元素排序即全序）；Hoare 扫描从 `i=1, j=hi-1` 起（首尾元素充当哨位）。

**动机**：Gen 1 的随机大输入只有 0.15~0.18x；中位数 pivot 把比较次数期望从 ~1.39n·log n 压到 ~1.19n·log n。

**基准数据**：

| distribution | n | ours(ms) | pdqsort(ms) | speedup | Gen1 speedup |
|---|---|---|---|---|---|
| random | 100 | 0.0003 | 0.0003 | 0.973x | 0.633x |
| random | 1k | 0.0046 | 0.0037 | 0.809x | 0.700x |
| random | 10k | 0.3927 | 0.0481 | 0.122x | 0.137x |
| random | 100k | 5.1509 | 0.7921 | 0.154x | 0.179x |
| random | 1M | 61.1554 | 9.0893 | 0.149x | 0.176x |
| all-equal | 100 | 0.0003 | 0.0000 | 0.071x | 0.040x |
| all-equal | 1k | 0.0049 | 0.0002 | 0.045x | 0.033x |
| all-equal | 10k | 0.0592 | 0.0021 | 0.035x | 0.024x |
| few-unique | 100 | 0.0003 | 0.0002 | 0.778x | 0.494x |
| few-unique | 1k | 0.0048 | 0.0013 | 0.273x | 0.234x |
| few-unique | 10k | 0.0624 | 0.0112 | 0.179x | 0.158x |
| sorted | 100 | 0.0003 | 0.0000 | 0.147x | 0.067x |
| sorted | 1k | 0.0046 | 0.0002 | 0.047x | 0.055x |
| sorted | 10k | 0.0567 | 0.0020 | 0.036x | 0.044x |
| reverse | 100 | 0.0003 | 0.0000 | 0.081x | 0.075x |
| reverse | 1k | 0.0040 | 0.0003 | 0.065x | 0.064x |
| reverse | 10k | 0.0417 | 0.0024 | 0.058x | 0.049x |
| nearly-sorted | 100 | 0.0002 | 0.0002 | 1.246x | 0.877x |
| nearly-sorted | 1k | 0.0037 | 0.0042 | 1.139x | 1.142x |
| nearly-sorted | 10k | 0.0421 | 0.0574 | 1.365x | 1.300x |
| organ-pipe | 100 | 0.0004 | 0.0003 | 0.742x | 0.351x |
| organ-pipe | 1k | 0.0082 | 0.0042 | 0.505x | 0.065x |
| organ-pipe | 10k | 0.1445 | 0.0560 | 0.388x | 0.013x |

**EVOLUTION SPEED SCORE：0.137721x → 0.207548x（+51%）**

**结论**：

1. **意外之喜：organ-pipe 0.013x → 0.388x（30 倍）**。原预测 median-of-3 对静态风琴形杀手仍然 O(n²)，但实测大幅好转——三元素排序把 `(1, max, 1)` 换成 `(1, 1, max)` 的那次 `swap(mid, hi)` 本身就是一次「模式粉碎」：分区第一轮就把风琴形结构打碎，后续输入退化成近似随机。**median-of-3 的预排序兼有 pattern-breaker 的作用**
2. random 大输入在 ±10% 噪声带内持平（pdqsort 侧单 case 读数 9.1~10.6ms 波动；我们侧 60.2 vs 61.2ms 也是噪声）。小输入（100/1k）明显受益
3. all-equal / few-unique / reverse 均有 10%~40% 提升
4. **最差 case 首次易主**：从 organ-pipe（0.013x）变成 all-equal（0.035x），23 个 case 全部脱离灾难区

**下一步方向**：Gen 3 插入排序 cutoff（小分区停止递归、改用插入排序，砍掉小分区的递归与分区常数）——这是常数项最大的一块，预期全面上涨。

## Gen 3：小分区插入排序 cutoff（CUTOFF = 16）

**改动**：`len ≤ 16` 的子区间不再分区递归，直接交换式插入排序收摊；Gen 2 的 `len ≤ 3` 早退被吞并。

**动机**：小叶子占全部叶子的绝大多数，每个都要付「3 次中位比较 + Hoare 分区 + 两次递归」的固定成本——当时最大的常数项浪费。

**基准数据**：

| distribution | n | ours(ms) | pdqsort(ms) | speedup | Gen2 speedup |
|---|---|---|---|---|---|
| random | 100 | 0.0002 | 0.0003 | 1.147x | 0.973x |
| random | 1k | 0.0033 | 0.0040 | 1.217x | 0.809x |
| random | 10k | 0.3131 | 0.0550 | 0.176x | 0.122x |
| random | 100k | 4.3651 | 0.9069 | 0.208x | 0.154x |
| random | 1M | 52.2541 | 10.6425 | 0.204x | 0.149x |
| all-equal | 100 | 0.0002 | 0.0000 | 0.119x | 0.071x |
| all-equal | 1k | 0.0030 | 0.0002 | 0.073x | 0.045x |
| all-equal | 10k | 0.0516 | 0.0020 | 0.040x | 0.035x |
| few-unique | 100 | 0.0002 | 0.0002 | 1.343x | 0.778x |
| few-unique | 1k | 0.0034 | 0.0015 | 0.442x | 0.273x |
| few-unique | 10k | 0.0655 | 0.0131 | 0.201x | 0.179x |
| sorted | 100 | 0.0001 | 0.0000 | 0.209x | 0.147x |
| sorted | 1k | 0.0017 | 0.0002 | 0.124x | 0.047x |
| sorted | 10k | 0.0291 | 0.0020 | 0.070x | 0.036x |
| reverse | 100 | 0.0001 | 0.0000 | 0.182x | 0.081x |
| reverse | 1k | 0.0019 | 0.0002 | 0.132x | 0.065x |
| reverse | 10k | 0.0304 | 0.0024 | 0.080x | 0.058x |
| nearly-sorted | 100 | 0.0001 | 0.0002 | 2.755x | 1.246x |
| nearly-sorted | 1k | 0.0018 | 0.0045 | 2.454x | 1.139x |
| nearly-sorted | 10k | 0.0325 | 0.0636 | 1.956x | 1.365x |
| organ-pipe | 100 | 0.0003 | 0.0003 | 0.931x | 0.742x |
| organ-pipe | 1k | 0.0070 | 0.0046 | 0.659x | 0.505x |
| organ-pipe | 10k | 0.1152 | 0.0672 | 0.583x | 0.388x |

**EVOLUTION SPEED SCORE：0.207548x → 0.327240x（+58%）**

**结论**：

1. random 大输入 +20~40%，小输入（100/1k）和 few-unique 100 **首次跑赢 pdqsort**——cutoff 路径短 vs pdqsort 模式识别的固定开销
2. sorted / reverse 近乎翻倍；nearly-sorted 扩大到 1.96x~2.76x（近乎有序切片是插入排序主场）
3. organ-pipe 0.388x → 0.583x
4. 最差 case 仍是 all-equal（0.040x）：Hoare 在全等输入上仍成对交换，pdqsort 的三路分区识别「全等即完成」直接返回 —— Gen 5  Dutch flag 的领域

**下一步方向**：Gen 4 尾递归消除 + 递归深限（最坏 O(n log n) 的硬保证），随后 Gen 5 三路分区攻 all-equal / few-unique。

## Gen 4：introsort 式保险（小侧递归 + 深度预算 + heapsort fallback）

**改动**：① 分区后总是递归较小侧、循环较大侧——入栈切片至多上轮一半，栈深结构性 ≤ log₂(n)；② 深度预算 `c·log₂(n)` 耗尽时该切片 fallback 堆排序，最坏时间复杂度获得 O(n log n) 硬保证。新增 2 个 lib 单元测试直达 fallback 私有路径。

**动机**：此前的实现两侧都递归，pivot 持续极劣时递归深度 ≈ n，既有爆栈风险时间也退 O(n²)。

**深度预算常数实验（数据驱动）**：先用经典 Musser 常数 `2·log₂(n)`，实测 organ-pipe 10k 从 0.583x（Gen 3）跌到 0.203x——排查发现 organ-pipe 分区质量本就贴着悬崖（70/30 式持续轻度不均衡，分区树深 ~25~26 层 ≈ 预算 28），过早触发 fallback。改为 `3·log₂(n)` 后 organ-pipe 恢复 0.501x（我们绝对耗时 0.279→0.112ms，真实 2.5 倍改善，与 std 侧噪声无关），且不破坏保证（树深 ≤ c·log₂n ⇒ 总量 ≤ O(c·n·log n)）。

**基准数据（3·log₂(n) 定稿，三轮中位记录）**：

| 用例 | ours(ms) | pdqsort(ms) | speedup | Gen3 speedup |
|---|---|---|---|---|
| random 100k | 4.339 | 0.794 | 0.183x | 0.208x |
| random 1M | 52.847 | 9.035 | 0.171x | 0.204x |
| all-equal 10k | 0.048 | 0.002 | 0.043x | 0.040x |
| sorted 10k | 0.029 | 0.002 | 0.071x | 0.070x |
| nearly-sorted 10k | 0.032 | 0.057 | 1.811x | 1.956x |
| organ-pipe 10k | 0.112 | 0.056 | 0.501x | 0.583x |

（全表见 git 历史；本轮三次总分：0.3200x / 0.3217x / 0.3339x，中位 **0.321654x**）

**结论**：

1. 分数与 Gen 3 统计持平（-1.7% 在 ±2% 经验噪声带内）——本代买的是**结构性保证**而非吞吐：栈深 ≤ log₂n（1M 输入 ~20 层，理论上不可能爆栈）、最坏 O(n log n)
2. 深度预算常数选择有真实权衡：2·log₂ 在「持续轻度不均衡」输入上误触 fallback；3·log₂ 兼顾速度与保证
3. 测量噪声再确认：sorted 1k/10k 的总分抖动完全来自 pdqsort 侧亚微秒读数（我们绝对耗时不变，std 读数 0.0002 vs 0.0004ms），**跨代比较必须看我们自己的绝对耗时**
4. 最差 case 仍是 all-equal（0.043x）——Gen 5 三路分区的主攻目标

**下一步方向**：Gen 5 荷兰旗三路分区（all-equal / few-unique 的专用杀器：全等即识别、直接返回）。

## Gen 5：pdqsort 三件套（DNF 三路分区 + 两段式 partial insertion sort + 坏分区模式粉碎）

**改动**：Hoare → DNF 三路分区（泛型免 Clone：pivot 位置跟踪扩展到 DNF）；分区前两段式 partial insertion sort；坏分区后模式粉碎。

**动机**：all-equal（0.043x）和 few-unique（0.25x）是当时最差的两个 case，都是 Hoare 对重复元素做无用功。

**这一代的翻车与修复（完整记录，教训比结果重要）**：

1. **DNF 单独上线 → 总分 -37%（0.322x → 0.204x）**。sorted 输入墙钟慢了 21 倍、耗时按 ~n^1.4 增长。用计数比较器的诊断测试定位：sorted 10k 比较次数 848,905 次（6.38 n log n，Gen 4 只要 111,022 / 0.84）。
2. **根因**：DNF 的 `>` 清扫在右区留下「有序 + 队尾一个错位元素」→ 下层三元素排序把**次小值**送进 pivot 位 → pivot 贴边 → 右区又是「有序+错位」——自相似退火链，每层只剥 1 个元素（TRACE 实测：len=255 时 lt=1, gt=1, left=1, right=253）。
3. **只加模式粉碎 → 6.38 降到 4.44 n log n，不够**：2 对交换只打乱 4/253 个元素，采样位仍被错位元素劫持。
4. **加 blind partial insertion → random/nearly-sorted/organ-pipe 三个 regime 全部受伤**（random 1M 0.171→0.131、nearly-sorted 1k 2.26→0.26、organ-pipe 0.50→0.15）：单段边扫边插时，nearly-sorted 第一个错位元素的插入就能跑几千步，成本已经 O(n) 才数到第二个下降沿；organ-pipe 更是 1 个下降沿配 O(n²) 插入路程。
5. **修正为两段式 → 问题全消**：先纯比较数下降沿、超 8 立即退场（乱序输入 ~16 次比较退场，不碰交换），确认近乎有序后才一趟插入。退火链被「有序+少量错位」节点的直接完成从根上剪断。

**基准数据（定稿版）**：

| distribution | n | ours(ms) | pdqsort(ms) | speedup | Gen4 speedup |
|---|---|---|---|---|---|
| random | 100 | 0.0003 | 0.0003 | 0.846x | 1.168x |
| random | 1k | 0.0064 | 0.0036 | 0.571x | 1.135x |
| random | 10k | 0.4017 | 0.0469 | 0.117x | 0.148x |
| random | 100k | 5.2839 | 0.7852 | 0.149x | 0.180x |
| random | 1M | 63.0825 | 8.8200 | 0.140x | 0.171x |
| all-equal | 10k | 0.0037 | 0.0020 | 0.547x | 0.043x |
| few-unique | 10k | 0.0131 | 0.0109 | 0.836x | 0.245x |
| sorted | 10k | 0.0040 | 0.0030 | 0.748x | 0.071x |
| reverse | 10k | 0.0098 | 0.0023 | 0.238x | 0.075x |
| nearly-sorted | 10k | 0.0830 | 0.0562 | 0.677x | 1.811x |
| organ-pipe | 10k | 0.2987 | 0.0550 | 0.184x | 0.501x |

**EVOLUTION SPEED SCORE：0.321654x → 0.494956x（+54%）**

**结论**：

1. all-equal 0.043x → 0.55x（13 倍），few-unique 0.25x → 0.84x，sorted 0.071x → 0.75x（跑赢 pdqsort 的 partial insertion 路径），reverse 0.075x → 0.24x（3 倍）
2. **固化了一个比较次数回归测试**（tests/diag.rs：断言 sorted/reverse/random ≤ 3 n log n）——这次 21 倍回退如果有它在，第一次 cargo test 就会报警
3. 仍低于 Gen 4 的：random 全线（DNF 固有 1.44 倍比较开销，1.58 vs 1.10 n log n，已量化）和 nearly-sorted 10k（1.81→0.68）、organ-pipe（0.50→0.18）
4. 下一代的明确方向：**hybrid 分区**（pdqsort 正式版架构）——默认 Hoare + 等值检测第二遍，只在检测到海量重复时才升级 DNF，拿回 random 的吞吐

## Gen 6：混合分区（默认 Hoare + 坏分区升级 DNF）

**改动**：分区策略改为 pdqsort 正式版架构——默认 Hoare 扫描（random 1.10 n log n），分区失衡（某侧 < len/8）时整段重跑 DNF 冻结 ==pivot 中段；partial insertion / 模式粉碎 / 深限 fallback 全部保留。

**动机**：Gen 5 全程 DNF 每层全量扫描（1.58 n log n），random 大输入只有 0.12~0.15x；但 Hoare 对重复密集数据没有等值冻结——用「失衡才升级」把两者的好处拼起来。

**基准数据（absolute：我们自己的耗时，ms）**：

| case | Gen 5 ours | Gen 6 ours | pdqsort | speedup(Gen6) |
|---|---|---|---|---|
| random 1k | 0.0064 | 0.0049 | 0.0045 | 0.911x |
| random 1M | 63.08 | 56.37 | 8.93 | 0.159x |
| all-equal 10k | 0.0037 | 0.0040 | 0.0020 | 0.501x |
| few-unique 10k | 0.0131 | 0.0213 | 0.0130 | 0.576x |
| sorted 10k | 0.0040 | 0.0040 | 0.0020-0.0030 | ~0.75x |
| reverse 10k | 0.0098 | 0.0082 | 0.0024 | 0.292x |
| nearly-sorted 10k | 0.0830 | 0.0342 | 0.0559 | 1.635x |
| organ-pipe 10k | 0.2987 | 0.3928 | 0.0549 | 0.140x |

**EVOLUTION SPEED SCORE：0.494956x → 0.514827x（+4%，三次运行 0.495/0.515/0.525）**

**结论**：

1. random 全线真实提速（1M 绝对耗时 63.1→56.4ms，1k 0.906→0.911x；1k 级别从 0.571 跳到 0.90+）——Hoare 扫描 + 少触发升级生效
2. nearly-sorted 10k 0.68→1.64x（partial insertion 完成更多中层切片）；reverse 0.238→0.292x
3. **few-unique 回吐（0.836→0.576）**：它的分区按值域是均衡的（2/5 vs 2/5），永远不触发失衡升级，等值段无人冻结。这是明确的下一代目标
4. organ-pipe 0.184→0.140 小幅回吐（升级通道在 organ-pipe 上多付了 2n 过路费，最终仍靠 heapsort fallback 收场）

**本代事故记录（门禁当场抓住，提交前已修复）**：抽 `break_patterns_if_unbalanced` helper 时把 break_patterns 误作用于**整段**而非左右两侧，打乱了 DNF 的三段边界 → random 数据输出未排序。bench 的 per-case sortedness 检查和 diag 测试同时报警。修复后新增 1 个 lib 单测直接压测升级通道（3 值重复密集数据）。教训：模式粉碎只能作用于分区边界之内。

**下一步方向**：few-unique 的等值探测——Hoare 的扫描停止点上加 `== pivot` 检查做等值外推（Bentley-McIlroy 风格，分支预测器对 distinct 数据的恒假分支几乎免费），用真实计数替代已否决的采样探测。

## Gen 7：负结果代（等值计数升级 × CUTOFF 扫描，双双否决——不痛下杀手也是产出）

**动机**：Gen 6 遗留两个待办——few-unique 回吐（0.836→0.576）与 CUTOFF=16 未经验证。

**实验 1：Hoare 停止点等值计数升级（目标 few-unique）**

在 Hoare 每次扫描停止点加 `cmp == Equal` 计数，`eq_hits·16 ≥ len` 触发 DNF 升级。推理模型预测 few-unique 命中率 ~1/6 应稳触发；实测：阈值放到 1/8 时 **few-unique 纹丝不动（0.576 → 0.558）**，random 全线白付 5%（1M 绝对耗时 56.4→59.1ms，每交换 2 次 cmp），总分 -9%。模型与实测不符，整体回退。

**实验 2：CUTOFF 参数扫描**

| CUTOFF | 三轮分数 |
|---|---|
| 8 | 0.5026 |
| 12 | 0.5266 / 0.4900 / 0.5059（中位 0.5059） |
| 16 | 0.5173 / 0.5306 / 0.4985（中位 0.5173） |
| 24 | 0.5274 / 0.5032 / 0.4923（中位 0.5032）|
| 32 | 0.5098 |

差异全部 ≤3%，而实测运行间噪声带已达 **±5%**——无统计显著收益，维持 16，扫描表已写进 `CUTOFF` 注释防止无数据重搞。

**测量环境结论**：跨代比较的显著性门槛修正为 >5%（批量计时后噪声从初版的 ±10~20% 收窄到 ±5%）。Gen 5→6 的 +4% 方向一致但已贴门槛。

**下一步方向**：few-unique 的真问题可能在「单值/等值区域冻结时机」而非触发信号。候选：（a）3-sort 后若三个采样点只有两种取值（如 a[0]==a[hi]<a[mid]），零比较成本拿到先验信号直接升级 DNF；（b）few-unique 距 pdqsort 是 0.021 vs 0.012ms，2 倍空间，值得专项。

**动机**：random 大输入与 pdqsort 差 6.3 倍（1M：56.4ms vs 8.9ms ≈ 2.4ns/次比较 ≈ 8 周期/次）。假设：pivot 位置跟踪的两个不可预测分支 + 索引边界检查是主因。

**实验：指针化 Hoare 分区**——pivot 值 `ptr::read` 进 `ManuallyDrop` 本地副本（免 Clone、不双重 drop），`swap(0, mid)` 把 pivot 元素钉在 0 号位（3-sort 的 `a[0] <= pivot <= a[hi]` 充当扫描哨兵，边界有完整证明），扫描与交换全部裸指针无界检查。

**实测**：random 1M 仅 -3.4%（56.4→54.5ms），但 reverse 10k +54%（0.0082→0.0126ms）、sorted 10k +25%，总分 0.517→0.472（-9%）。比较次数诊断：reverse 从 0.23 → 0.30 n log n（分区结构改变，parked pivot 不再参与扫描、失去提前停止效应）。**整体回退，死路结论写进 `hoare_partition` 注释。**

**核心诊断（本代真正的产出）**：random 1M 的 6 倍差距，瓶颈**不是**边界检查/位置跟踪（去掉它们只赚 3.4%），而是**分支预测失败**——Hoare 扫描是数据依赖分支，random 数据上 ~50% mispredict，≈8 周期/次比较；pdqsort 的近无分支分区只要 1-2 周期/次。**下一代的正确靶心：无分支分区（branchless partition）。**

**下一步方向**：branchless Hoare/DNF 分区——用条件移动/算术替代数据依赖分支的比较循环（C++ std::sort 的同款技术）。注意与 Gen 7 结论合并看：等值信号两条路（采样/计数）都没收益，真正的两块大肉是（1）random 大输入的分支less 化；（2）few-unique 的等值冻结时机。

## Gen 9：双分区 + 下降沿密度信号（branchless Lomuto×Hoare）

**改动**：新增分支免费版 Lomuto 分区（无条件 swap + cmov 条件自增）；`partial_insertion_sort` 改为返回 bail 位置充当「逆序密度」信号（前 10 位攒够 9 个下降沿 → 走 Hoare，否则走 Lomuto）；两分区统一输出切分点 k。

**动机**：Gen 8 定位的真瓶颈——Hoare 扫描的数据依赖分支在 random 上 ~50% mispredict（≈8 周期/次比较， vs pdqsort 近无分支分区的 1-2 周期）。

**基准数据（absolute，ms）**：

| case | Gen 6 ours | Gen 9 ours | pdqsort | speedup(Gen9) |
|---|---|---|---|---|
| random 1k | 0.0049 | 0.0046 | 0.0036 | 0.798x |
| random 10k | 0.3744 | 0.1126 | 0.0470 | 0.417x |
| random 100k | 4.70 | 2.20 | 0.79 | 0.358x |
| random 1M | 56.37 | 22.70 | 8.93 | 0.394x |
| all-equal 10k | 0.0040 | 0.0040 | 0.0020 | 0.502x |
| few-unique 1k | 0.0022 | 0.0015 | 0.0015 | 0.952x |
| few-unique 10k | 0.0213 | 0.0150 | 0.0122 | 0.813x |
| sorted 10k | 0.0040 | 0.0042 | 0.0040 | 0.947x |
| reverse 10k | 0.0082 | 0.0073 | 0.0032 | 0.432x |
| nearly-sorted 10k | 0.0342 | 0.0765 | 0.0563 | 0.736x |
| organ-pipe 10k | 0.3928 | 0.2554 | 0.0548 | 0.214x |

**EVOLUTION SPEED SCORE：~0.52x → 0.604114x（三轮中位，+16%）**

**结论**：

1. **random 2.4~3.3 倍**（1M 54.5→22.7ms、10k 0.37→0.11ms）——分支预测诊断完全兑现，current worst case 从 random 变成 organ-pipe（0.214x）
2. **few-unique 历史最佳**（0.81~1.06x）：等值归右 + Hoare 被信号让位后，Lomuto 的扫描顺序搬运对重复数据反而有利
3. **reverse 完整恢复且更好**（0.290→0.432x）：bail_pos=9 ≤ 10 的信号精准命中 Hoare 路径
4. 无牺牲项：all-equal / sorted 持平；唯一让步是 nearly-sorted 10k（1.598→0.736x，Lomuto 每层全量交换的流量成本，小规模 1.66~2.18x 仍赢 pdqsort）
5. 误触率实测符合模型：random 走 Lomuto（bail_pos ~17+），reverse 走 Hoare（bail_pos=9）

**下一步方向**：（a）nearly-sorted 10k 的 Lomuto 交换流量问题——条件存储版 branchless（cmov 值而非无条件 swap）；（b）organ-pipe（0.214x）的新 worst case；（c）random 与 pdqsort 仍有 2.4 倍差距（21.9 vs 8.9ms），下一步可试 Ninther pivot 与块预取。

## Gen 10：信号扩三档（稀疏下降沿也走 Hoare）

**改动**：Gen 9 的两档信号（bail_pos ≤ 10 → Hoare）扩展为三档：新增 `bail_pos ≥ 48`（下降沿极稀疏 = 近乎有序）→ Hoare。逻辑：Hoare 吃有结构的数据（逆序→左区有序、近有序→扫描提前收工），Lomuto 吃无结构数据；random ~17、few-unique ~22、organ-pipe ~n/2+9 落在中间档继续走 Lomuto。

**动机**：Gen 9 的 nearly-sorted 10k 因 bail_pos ≈ 900（稀疏档未定义）被路由到 Lomuto，付全量交换流量回退到 0.736x；Gen 6 的 Hoare 实测 1.598x。

**基准数据（absolute，ms）**：

| case | Gen 9 ours | Gen 10 ours | speedup(Gen10) |
|---|---|---|---|
| nearly-sorted 100 | 0.0001 | 0.0001 | 1.788x |
| nearly-sorted 1k | 0.0025 | 0.0020 | 2.029x |
| nearly-sorted 10k | 0.0765 | 0.0376 | 1.527x |
| random 1M | 22.70 | 22.82 | 0.394x |
| random 10k | 0.1126 | 0.1070 | 0.445x |
| organ-pipe 1k | 0.0118 | 0.0102 | 0.406x |

**EVOLUTION SPEED SCORE：四轮 0.554/0.599/0.573/0.620，中位 0.586x（Gen 9 中位 0.604，噪声带内持平）**

**结论**：

1. **nearly-sorted 结构性恢复**：10k 绝对耗时 -51%（0.0765→0.0376ms，speedup 0.736→1.527x），1k -20%（2.03x）——bail_pos≈900 稳定落入稀疏档
2. random 无损（bail_pos ~17 稳居中档）；organ-pipe 小规模上涨（bail_pos ≈ n/2+9 也入稀疏档走 Hoare）
3. 总分未过噪声门槛，但按 Gen 2 确立的「跨代比对我们自己的绝对耗时」原则，这是确定性收益：目标 case 的真实耗时砍半
4. 测量噪声结论再确认：±5-6% 的运行间抖动主要由 pdqsort 侧亚 10µs 读数贡献，几何平均对单 case 结构改进的分辨率不足——判定改进以 absolute time 为准

**下一步方向**：（a）organ-pipe（当前 worst 0.19~0.41x，高方差）——bail_pos≈n/2+9 的晚期密集模式值得专项（三分区本就该处理它，问题在 3-sort 选了 min 作 pivot）；（b）random 与 pdqsort 的 2.4 倍残余差距（Hoare 式交换效率 × 无分支扫描的合流，即 BlockQuicksort 方向）；（c）Ninther pivot。

## Gen 11：退化分区强粉碎（分块轮换击败 organ-pipe 剥层链）

**改动**：DNF 升级后若某侧 ≤ len/64（pivot 恰为区间极值的铁证），对大侧调用 `scramble_patterns`：首四分之一 ↔ 末四分之一整块轮换（len/4 对 swap）。小侧 <128 自动 no-op 自限。

**动机**：organ-pipe 三采样 (1, max, 1) → pivot 恒为最小值 → 每层剥 1~2 个 → 弱 break_patterns（2 对交换，Gen 5 已证明打不碎 99% 两段有序）无效 → 落 heapsort（0.26~0.38ms）。

**基准数据（absolute，ms，三轮）**：

| case | Gen 10 ours | Gen 11 ours | pdqsort | speedup(Gen11) |
|---|---|---|---|---|
| organ-pipe 1k | 0.0102 | 0.0088 | 0.0045 | 0.515x |
| organ-pipe 10k | 0.2554~0.3817 | 0.1330~0.1338 | 0.0658 | 0.492x |
| random 1M | 22.70~22.82 | 22.28~22.60 | 8.9~10.3 | ~0.46x |
| random 10k | 0.1070~0.1126 | 0.1083~0.1316 | 0.054~0.067 | ~0.51x |

**EVOLUTION SPEED SCORE：四轮 0.581/0.589/0.588/0.566，中位 0.583x（Gen 10 中位 0.586，持平）**

**结论**：

1. **organ-pipe 10k 绝对耗时 0.26~0.38 → 0.133ms（砍半）**，三轮极稳定；成本落到 heapsort 线（Gen 4 实测纯 heapsort ≈ 0.112ms）——分块轮换成功把剥层链打断为均衡递归。speedup 从 0.19~0.41x 的混沌区间收敛到 0.49x
2. random 无损：scramble 期望成本 ~0.4 次交换/节点（P(pivot 恰为极值) ≈ 3/(2n)），本轮观察到的 ±20% 摆动来自代码布局/机器负载——连 pdqsort 侧同向摆动 16~42%，非本代引入
3. worst case 从 organ-pipe（0.19x）交棒给 reverse（0.235x， pdqsort 侧 0.0024ms 级读数噪声放大）

**测量噪声再确认**：亚毫秒 case 的噪声带实为 ±20%（此前记的 ±5% 偏乐观）；判定改进仍以我们自己的 absolute time 多轮一致性为准。

**下一步方向**：（a）organ-pipe 距 pdqsort 仍有 2 倍（0.133 vs 0.066ms）——scramble 后的第一层分区质量可再教研（如退化时换 Ninther 重取 pivot 而非粉碎）；（b）random 与 pdqsort 的 2.4 倍残余（1M 22.4 vs 8.9ms）是最大单块肉；（c）reverse 读数噪声治理（bench 侧增量：对小 n 用例加密重复）。

## Gen 12：branchless 分区裸指针化（去边界检查，小胜）

**改动**：`branchless_partition` 的比较与交换从安全索引改为裸指针（`*base.add(j)` / `ptr::swap`），不动任何结构（无条件 swap + cmov 自增保持不变）。SAFETY 依据：不变式 `i ≤ j < len`（i 每轮至多追平 j）。

**动机**：random 与 pdqsort 仍有 2.4 倍差距（1M 22.4 vs 8.9ms）。分解：Lomuto 每层 n 次交换（Hoare 式 ~n/4），且每次 swap 带 2 次边界检查。

**基准数据（absolute，ms，多轮）**：

| case | Gen 11 best | Gen 12 best | pdqsort |
|---|---|---|---|
| random 1k | 0.0045 | 0.0042 | 0.0040 |
| random 10k | 0.1070 | 0.1070~0.1128 | 0.054 |
| random 100k | 2.15 | 2.08 | 0.90 |
| random 1M | 22.28 | 21.36~21.54 | 8.9~10.3 |

**EVOLUTION SPEED SCORE：0.561/0.575/0.577（中位 0.575，持平）；random 绝对耗时 -3~5%**

**结论**：

1. random 五项全部达到或优于历史最佳（1M -3~4%、100k -3%、1k -6%），方向与理论一致但**单 case 幅度在 ±20% 噪声带内**——记为小胜
2. 边界检查不是主瓶颈（与 Gen 8 的指针实验结论合并印证）；**真正的剩余差距是交换次数本身**：Lomuto 每层 n 次 swap vs pdqsort 家族（ipnsort/BlockQuicksort）的 Hoare 式 ~n/4 + 块扫描
3. 下一目标明确且是最大单块肉：random 1M 21.5ms vs pdqsort 8.9ms 的 2.4 倍差距，合流方案 = BlockQuicksort 式块分区（块内偏移统计替代逐元素分支/无条件交换）

## 死路记录

| 方案 | 结论 | 原因 |
|---|---|---|
| DNF 单独使用（无 partial insertion / 模式粉碎） | 死路，总分 -37% | sorted 输入自相似退火链，6.38 n log n，墙钟 21 倍回退 |
| blind partial insertion（单段边扫边插） | 死路 | nearly-sorted/organ-pipe 的大位移插入陷井，三 regime 全面回退 |
| break_patterns 单独作为退火链防线 | 不够 | 只把 6.38 降到 4.44 n log n，2 对交换打不碎 99% 有序的结构 |
| 分区后「等值采样探测」（两侧各 4 位，命中≥2 即升级 DNF） | 否决，总分 -4% | few-unique 部分恢复（0.514→0.608）但 random 全线付 ~10%（每节点 8 次比较），净负。回退 |
| Hoare 停止点等值计数升级 | 否决，总分 -9% | few-unique 无收益（0.576→0.558），random 付 5%，模型与实测不符 |
| CUTOFF ∈ [8,32] 扫描 | 无显著收益 | 差异 ≤3% < 噪声带 ±5%，维持 16 |
| 「3-sort 三点只有两值」先验信号 | 未实验，估算否决 | organ-pipe 会被迫走 DNF（0.112→~0.3ms），亏的比 few-unique 赚的多（净 -10% 估算） |
| Hoare 指针化（ManuallyDrop pivot + 钉位 + 裸指针） | 否决，总分 -9% | random 仅 -3.4%，reverse/sorted +25~54%；真瓶颈是分支预测而非边界检查 |
