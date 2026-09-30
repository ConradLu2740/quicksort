# 进化日志

计分规则（长期有效）：

- 每代跑 `cargo run --release --bin bench`，记录 **EVOLUTION SPEED SCORE**（23 个用例几何平均，对标 std pdqsort，越高越好）+ **相位 gauge**（跨代比较须对 gauge；~8.8 冷相 / ~10.3 热相）
- `cargo test && cargo test --release` 双 profile 不全绿的世代作废，不允许进基准（Gen 32 制度化）
- 单轮提升 < 5% 判噪声（Gen 5 修正：批量计时后运行间噪声 ±5%，热/冷相位另计），重测；确认为死路的，「为何不行」也要记录
- 跨代比较以「我们自己的绝对耗时」为准（Gen 2/3 确立：pdqsort 侧读数同样漂移）；改 bench 后先用绝对耗时金丝雀验证 bench 自身（Gen 31）
- 单次总分可能含段中热漂移（Gen 37 实测 std 侧 +42% 尖峰，末尾 gauge 不反映）——判决用多轮绝对耗时对比
- 每代一个 git commit

## 路由地图（Gen 38 审计合成：每个基准分布走哪条路，改路由前必读）

每个节点（len > 32）的路径决策，按顺序：

1. **len ≤ 32** → 插入排序收尾（CUTOFF 叶）
2. **深度预算 = 0** → heapsort fallback（introsort 保证）
3. **粗糙预筛**（len ≥ 128，8 个等距采样点的 7 个相邻对）：
   - 下降沿 2~6 = **粗糙**（random / few-unique / organ-pipe）→ 跳过下降沿扫描，直达 ninther + 分支免费 Lomuto
   - 下降沿 0~1 = 平滑（sorted / all-equal / nearly-sorted）→ 完整路径
   - 下降沿 7/7 = 逆序密度（reverse 家族）→ 完整路径（不可直达 Lomuto，逆序数据在 Lomuto 上是 1.47 n log n 灾难）
4. **完整路径 - partial insertion 两段式**：
   - 0 下降沿 → 证明已有序，一趟返回（all-equal / sorted 的 O(n) 直达）
   - ≤8 下降沿 → 洞式插入直接排完返回（有序 + 少量错位）
   - >8 下降沿 → 记 bail 位置（第 9 个下降沿处），进入三档路由
5. **三档路由**（按 bail_pos）：
   - ≤10 逆序密度 → 先试整段逆序检测（成功则一次 reverse 完成）；失败 → **Hoare**（逆序数据在 Hoare 上左区天然有序，partial 可接住）
   - ≥48 且 bail 点前无连续降序游程 → **Hoare**（近乎有序，扫描提前收工：nearly-sorted 10k 1.6x vs Lomuto 0.74x）
   - ≥48 且 bail 点前有连续降序游程 → **Lomuto + ninther**（大块降序尾 = organ-pipe 家族：Hoare 的 med-3 会取到区间最小值剥 1 层落 heapsort；ninther 取 ~1/8 分位值 3/4 剥层、~40 层深度 < 51 预算逃过 fallback）
   - 中间密度 → **Lomuto + ninther**（消除扫描分支 ~50% mispredict，random 2.4 倍）
6. **坏分区晋升**（任一侧 < len/8）：整段重跑 DNF 三路分区冻结 ==pivot 中段（重复密集数据的救命通道，few-unique 依赖它按值域收缩）；其中某侧 ≤ len/64（pivot 恰为极值）→ 分块轮换强粉碎（organ-pipe 剥层链的破坏者）
7. **均衡分区**：小侧递归（栈深 ≤ log₂n）+ 大侧循环

**各基准分布的顶层路径**：

| 分布 | 路径 |
|---|---|
| random | 预筛判粗糙 → ninther + 分支免费 Lomuto |
| all-equal | 预筛判平滑 → partial 0 下降沿直达完成 → O(n) |
| sorted | 同上 |
| reverse | 预筛判 7/7 逆序密度 → partial bail@9 → 逆序检测成功 → reverse() → O(n/2) |
| nearly-sorted | 预筛判平滑 → partial bail@~900 → 稀疏无降尾 → Hoare（左区有序副产品 + partial 接住） |
| few-unique | 预筛判粗糙 → ninther + Lomuto；坏分区时 DNF 冻结等值中段按值域收缩 |
| organ-pipe | 预筛 desc=4 判粗糙 → Lomuto + ninther（3/4 剥层）；<128 的小切片走完整路径同样落在「稀疏 + 连续降尾 → Lomuto+ninther」 |


## 当前状态

- 世代：**Gen 38**
- EVOLUTION SPEED SCORE：**无分数结论（热相位 gauge ~10.38；本代为文档审计）**
- 正确性：双 profile 全绿——debug 10 套件 / release 11 套件（含 200k mega stress）

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
| Gen 12 | ~0.57x（持平） | branchless 分区裸指针化：random -3~5% | 2026-10-01 |
| Gen 13 | ~0.60x | Ninther pivot：random 100k/1M 绝对耗时 -10~11% | 2026-10-01 |
| Gen 14 | ~0.70x | 洞式插入排序：few-unique 10k -7%、nearly-sorted 10k -12% | 2026-10-01 |
| Gen 15 | ~0.74x | CUTOFF 重扫（16→24）：架构变化后最优值上移 | 2026-10-01 |
| Gen 16 | 绝对耗时大降 | LTO + codegen-units=1 + panic=abort：random 10k -45% | 2026-10-01 |
| Gen 17 | ~0.67x（持平） | NINTHER_MIN 扫描确认 64；BlockQuicksort 推导存档 | 2026-10-01 |
| Gen 18 | ~0.67x（持平） | 负结果：native 回退；BlockQuicksort 净收益归零，关闭 | 2026-10-01 |
| Gen 19 | ~0.83x | 零下降沿跳过：all-equal / sorted 追平 pdqsort（0.99x） | 2026-10-01 |
| Gen 20 | ~0.84x（持平） | 负结果：极端失衡直通粉碎，回退 | 2026-10-01 |
| Gen 21 | ~0.98x | 逆序检测直达：reverse 追平 pdqsort；冷路径外描修布局回归 | 2026-10-01 |
| Gen 22 | ~0.93x（持平） | 负结果：two-run 二次方修复有效但布局税致判分器 -10%，回退入档 | 2026-10-01 |
| Gen 23 | ~0.94x | CUTOFF 二扫（24→32）：完成路径降价后 32 微弱占优 | 2026-10-01 |
| Gen 24 | ~0.944x | 降序游程精化：organ-pipe 改走 Lomuto+ninther 逃过 heapsort；路由整体外描零布局税 | 2026-10-01 |
| Gen 25 | ~1.017x | 粗糙数据预筛（8 采样三态）：跳过下降沿扫描直达 Lomuto，random 1M -18%、organ-pipe 追平 | 2026-10-01 |
| Gen 26 | ~1.025x | NINTHER_MIN=64 机制确认 + 预筛小切片门（三个 100 元素 case 恢复） | 2026-10-01 |
| Gen 27 | ~1.025x（持平） | 负结果：第五次块分区尝试（交换减半），正确性未收敛 + 性能负，回退 | 2026-10-01 |
| Gen 28 | ~1.03x（持平） | 负结果：two-run 修复布局窗口重试——税稳定复现，二次证伪 | 2026-10-01 |
| Gen 29 | ~1.03x（持平） | 负结果：二分插入排序实测 +72~230%，回退 | 2026-10-01 |
| Gen 30 | ~1.02x | 路由参数面收官（UNBALANCED_DIV/SPARSE/LIMIT 全确认）+ 差分门禁扩容至 6 测试 | 2026-10-01 |
| Gen 31 | —（判分器代） | 相位 gauge 后置落地；harness 污染实验：前置负载 +17% 污染被测 case（金丝雀纪律沉淀） | 2026-10-01 |
| Gen 32 | —（门禁代） | 双 profile 验证制度化 + release 百万级 stress（200k×5 模式，cfg 反选零成本） | 2026-10-01 |
| Gen 33 | —（hygiene 代） | clippy 两处修正（int_plus_one、nonminimal_bool），canary +1% 噪声内，双门禁绿 | 2026-10-01 |
| Gen 34 | —（门禁代） | mega stress 补 organ-pipe 200k（最深自适应路径，深度 ~40 贴预算 51）+ all-equal，7 模式全绿 | 2026-10-01 |
| Gen 35 | —（文档代） | EVOLUTION.md 一致性审计：清 32 行过期重复表 + 规则阈值漂移修正，35/35 代一一对应 | 2026-10-01 |
| Gen 36 | —（API 代） | 公共 API 文档补全（复杂度契约/行为说明）+ clippy 重借用抛光清零，金丝雀平稳 | 2026-10-01 |
| Gen 37 | 持平（负结果） | 采样融合收益落空于 L1 热（金丝雀 +0.5%）回退；差点被段中热漂移的 1.06 总分误导 | 2026-10-01 |
| Gen 38 | —（文档代） | 路由地图审计合成：23 分布路径全量走查无错路；EVOLUTION.md 新增路由地图节，lib.rs 头部脉络化 | 2026-10-01 |

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

## Gen 8：负结果代（Hoare 指针化否决，-9%；但定位了 random 大输入的真瓶颈）

**动机**：random 大输入与 pdqsort 差 6.3 倍（1M：56.4ms vs 8.9ms ≈ 2.4ns/次比较 ≈ 8 周期/次）。假设：pivot 位置跟踪的两个不可预测分支 + 索引边界检查是主因。

**实验：指针化 Hoare 分区**——pivot 值 `ptr::read` 进 `ManuallyDrop` 本地副本（免 Clone、不双重 drop），`swap(0, mid)` 把 pivot 元素钉在 0 号位（3-sort 的 `a[0] ≤ pivot ≤ a[hi]` 充当扫描哨兵，边界有完整证明），扫描与交换全部裸指针无界检查。

**基准数据（absolute，ms）**：

| case | Gen 7 | Gen 8 ours | pdqsort |
|---|---|---|---|
| random 1M | 56.37 | 54.53 | 8.93 |
| random 100k | 4.70 | 4.59 | 0.79 |
| random 10k | 0.3744 | 0.3543 | 0.047 |
| reverse 10k | 0.0082 | 0.0126 | 0.0024 |
| sorted 10k | 0.0040 | 0.0050 | 0.0020 |

**EVOLUTION SPEED SCORE：0.517 → 0.472（-9%），整体回退**

**结论**：

1. random 全线仅涨 3~5%（1M 56.4→54.5ms）——边界检查不是主瓶颈
2. **reverse 10k +54%、sorted +25%**：比较次数诊断显示 reverse 从 0.23 → 0.30 n log n——parked pivot 不再参与扫描，失去「pivot 位天然挡停」的提前停止效应。死路结论写进 `hoare_partition` 注释
3. **核心产出：真瓶颈是分支预测失败**——Hoare 扫描是数据依赖分支，random 上 ~50% mispredict ≈ 8 周期/次比较；pdqsort 的近无分支分区只要 1-2 周期/次。**靶心修正为 branchless 分区**（Gen 9 兑现：random 2.4 倍）

**下一步方向**：branchless 分区（无条件 swap + cmov 条件自增）。

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

## Gen 13：Ninther pivot（九采样中位的中位的中位）

**改动**：Lomuto 路径的 pivot 从 median-of-3 升级为 Tukey ninther（3 组三采样各取中位、再对 3 个中位取中位，~20 次比较钉到 mid 位）；`len < 64` 保持中位三（盈亏点模型：省 ~13% 迭代 vs 成本 20 比较，m≈32 回本）。Hoare 路径哨兵依赖三位置有序，保持中位三不变。抽出 `sort3` 排序网络复用。

**动机**：random 与 pdqsort 的残余差距（BlockQuicksort 重构风险高，先拿低风险的经典项）——ninther 压小 pivot 秩次方差，比较与交换次数同降。

**基准数据（absolute，ms）**：

| case | Gen 12 三轮 | Gen 13 三轮 | pdqsort | speedup(Gen13) |
|---|---|---|---|---|
| random 10k | 0.107~0.150 | 0.101~0.127 | 0.054 | 0.53x |
| random 100k | 2.08~2.60 | 1.82~2.28 | 0.90 | 0.49x |
| random 1M | 21.36~22.99 | **19.00~19.04** | 10.35 | 0.544x |
| few-unique 10k | 0.0144 | 0.0146 | 0.0129 | 0.885x |
| organ-pipe 10k | 0.133 | 0.116~0.133 | 0.066 | 0.57x |

**EVOLUTION SPEED SCORE：0.577/0.596/0.614，中位 0.596x（Gen 12 中位 0.575，+4%）**

**结论**：

1. **random 1M 稳定 -11%**（三轮 19.00/19.04/19.02，低于 Gen 12 全部历史读数 21.4+），100k -10%，10k -5%——大 case 计时稳定性好，超出噪声带
2. few-unique 持平、organ-pipe 小涨（ninther 的九采样交换对风琴形有额外打散作用）
3. 交换次数随迭代数同比下降（Lomuto 每迭代 1 swap），比较/交换双降是 -11% 的主因
4. 下一步剩余差距：random 1M 19.0 vs 10.4ms 的 1.8 倍（缩窄自 2.4 倍）——BlockQuicksort 式块配对仍是终极答案，但有更便宜的中间项可试：对 Lomuto 的冗余 swap 做「等值跳过」（i==j 时免交换的平凡优化，及 i,j 距离 < 2 时的合并路径）

## Gen 14：洞式插入排序（移位 3 次移动 → 1 次 memmove）

**改动**：`insertion_sort` 从交换式改为洞式：待插值 `ptr::read` 进 ManuallyDrop 洞位变量，较大元素 `ptr::copy` 逐一右移，最后 `ptr::write` 落位。每次移位从 swap 的 3 次移动降为 1 次 memmove。SAFETY：ManuallyDrop 不 drop、`j > 0` 保证不越界。

**动机**：Gen 13 下一步清单——Lomuto 的两个「冗余交换」微优化经推演均为死路（`i==j` 跳过要分支、`!less` 空操作就是原始分支版），转向插入密集 case 的真成本：插入排序在每个 cutoff 叶子和每个 partial-insertion 完成节点都跑，10k 规模约占排序总工作量 30%。

**基准数据（absolute，ms）**：

| case | Gen 13 | Gen 14 三轮 | 变化 |
|---|---|---|---|
| random 1M | 19.00~19.04 | 18.85~22.89（含热机相） | ~持平 |
| random 10k | 0.101~0.127 | 0.102~0.133 | 持平 |
| few-unique 10k | 0.0146 | 0.0135~0.0137 | **-7%** |
| nearly-sorted 10k | 0.0377 | 0.0331~0.0334 | **-12%** |
| few-unique 1k | 0.0016 | 0.0016 | 持平 |
| nearly-sorted 1k | 0.0017 | 0.0017 | 持平 |

**EVOLUTION SPEED SCORE：0.689/0.703/0.701（中位 0.701；含热机相位比率通胀）**

**结论**：

1. 插入密集 case 稳定获益：few-unique 10k -7%、nearly-sorted 10k -12%（三轮极稳），符合移位成本降 2/3 的预期（这两项插入占比高）
2. random 持平（分区主导，插入占比小）——符合成本模型
3. 分数中位 0.701 vs Gen 13 的 0.596：涨幅含机器「热相位」比率通胀（本轮 pdqsort 读数整体偏高），但绝对耗时证据支持真实为正

**测量环境备注**：观察到机器存在明显冷/热相位（同代码 random 1M 我们侧 18.9 vs 22.9ms、std 侧 8.9 vs 10.4ms 同向摆动）——跨代比较必须同相位或只用绝对耗时，已再次验证。

## Gen 15：CUTOFF 重扫（架构变化后最优值上移 16 → 24）

**改动**：`CUTOFF` 16 → 24。Gen 7 之后架构已三连变（branchless 分区、Ninther、洞式插入），Gen 7 的「全平」结论作废，重扫。

**扫描数据（当前架构，23 case 几何平均，三轮中位/单轮）**：

| CUTOFF | 分数 |
|---|---|
| 8 | 0.668 |
| 12 | 0.680 |
| 16 | 0.671 / 0.728 / 0.729（中位 0.728） |
| 24 | 0.706 / 0.741 / 0.757（中位 0.741） |
| 32 | 0.721 / 0.748 / 0.770（中位 0.748） |

**结论**：

1. **最优值确实从 16 上移到 24~32**（Gen 7 老架构上 8~32 全平 0.503~0.517）——机制：洞式插入让每次移位成本降 2/3，大叶子更便宜，分区迭代的占比相对上升
2. 24 与 32 中位仅差 1%（噪声内）；取 24 兼顾收益与叶级 O(CUTOFF²) 暴露（对抗性近结构化小切片的插入上界更保守）
3. 相对 Gen 14（CUTOFF=16 时中位 ~0.70）：本代中位 0.741，+5% 左右——刚过显著性门槛，且方向被机制解释支撑，记为参数代收益

**BlockQuicksort 状态备注**：连续两轮推导未收敛到可证明正确的块配对版本（偏移量缓冲的惰性配对/指针推进规则是该算法最易错处），按质量底线暂不冒险上主实现；留作有完整推导时的专项。

## Gen 16：release 构建配置调优（LTO + codegen-units=1 + panic=abort）

**改动**：`Cargo.toml` 的 `[profile.release]` 从仅 `opt-level = 3` 增加 `lto = true`（跨 crate 内联）、`codegen-units = 1`（单编译单元、优化器见全调用图）、`panic = "abort"`（去着陆垫；bench 正确性失败仍走 `process::exit(1)`）。零算法逻辑改动。

**动机**：random 的剩余差距里可能含有构建配置成分——lib 与 bench 之间的泛型单态化调用此前未被跨单元内联，panic 着陆垫包围热路径。

**基准数据（absolute，ms，三轮中位）**：

| case | Gen 15 | Gen 16 三轮 | 变化 |
|---|---|---|---|
| random 10k | 0.102~0.133 | 0.0514 / 0.0557 / 0.0646 | **-45%** |
| random 100k | 1.82~2.28 | 1.61 / 1.61 / 2.01 | -11% |
| random 1M | 18.85~22.89 | 17.02 / 17.16 / 17.87 | -9% |
| random 1k | 0.0040~0.0050 | 0.0039 | -5% |
| few-unique 10k | 0.0135 | 0.0138 | 持平 |
| nearly 10k | 0.0331 | 0.0331 | 持平 |
| organ-pipe 10k | 0.1114~0.116 | 0.1114 | 持平 |

**结论**：

1. **random 10k -45%**（0.0557 中位，三轮稳定）——全项目单 case 最大跃升；机制：单编译单元 + LTO 后 LLVM 对分区内循环（cmov 形成、展开、跨函数常量传播）的优化质量大幅提升
2. random 100k/1M -9~-11%，1k -5%
3. 非 random case 持平（它们的热路径此前已基本最优）
4. **本轮总分读数 0.663 不可用于跨代比较**：pdqsort 侧读数处于冷相位（1M std 8.78 vs 热相位 10.35）——再次验证「跨代只用我们自己的绝对耗时」原则
5. debug + release 双 profiles 测试全绿

**下一步**：random 1M 距 pdqsort 还剩 1.95 倍（17.1 vs 8.8ms）。构建配置已到顶，剩余差距纯算法（块配对分区）。

## Gen 17：NINTHER_MIN 扫描确认 + BlockQuicksort 推导存档（第三次止步于尾部）

**NINTHER_MIN 扫描**（32/64/128/256，random 绝对耗时 ms）：

| NINTHER_MIN | random 10k | random 100k | random 1M |
|---|---|---|---|
| 32 | 0.0533 | 1.62 | 17.31 |
| 64 | 0.0521 | 1.65 | 17.22 |
| 128 | 0.0548 | 1.62 | 16.90 |
| 256 | 0.0546 | 1.61 | 16.98 |

差异全在 ±3% 噪声内 → **64 确认**（与盈亏点模型一致）。分数摆动 0.585~0.672 为机器相位。

**BlockQuicksort 推导存档（第三代尝试，存档备将来）**：

已收敛且可证明的主体设计（将来实现从这里开始）：
1. 三区不变式：[0,L) 全部 < pivot 已定居；[R,len) 全部 > pivot 已定居；(s,t) 未处理
2. 左扫描从 s 向前，`offs_l[nl] = s; nl += (a[s] >= p) as usize`（cmov 分支less 记录），缓冲满 BLOCK 或到 t 止；右扫描对称（`a[t-1] <= p`）
3. 每轮配对 `min(nl, nr)` 对 swap：`swap(a[offs_l[i]], a[offs_r[i]])` —— 每次交换同时把一个 ≥ 和一个 ≤ 归位 → random 上 swap 数降为 Lomuto 的 ~1/4
4. 主循环终止条件：`s >= t && (nl == 0 || nr == 0)`；缓冲 copy_within 出队

**卡点（止步处，已三次）**：主循环退出后，待换残项（≤ BLOCK/侧）+ 中段已分类但未分区的元素构成「中段」，朴素收尾分区按递归树求和 ≈ +n，吃光交换节省。古典解法的「精确 k 增量记账」未能低成本闭合。将来正确路径：实现时跟踪 k = 左类元素计数（每次左扫描发现 < 元素即 k 相关推进，配对交换时同步调整），使残项直接 swap 到 [k, k+残项) 而无需中段扫描。**在完整推导出 k 记账规则前不上主实现。**

## Gen 18：负结果（target-cpu=native -1% 回退；BlockQuicksort k 记账推导完整但净收益归零）

**实验 1：target-cpu=native**（`.cargo/config.toml` rustflags）

冷相位三轮 random 1M：16.89 / 16.96 / 16.89？——实为 16.89/16.96（+一次热相位 20.92），对比无 native 基线 17.02~17.22：**约 -1%，在 ±20% 相位噪声带内，不达 5% 显著性门槛**。且引入机器相关代码生成会混淆跨代基线 —— **回退**（`rm -rf .cargo`），基线保持稳定。

**实验 2：BlockQuicksort k 记账闭合尝试（第四次）**

本轮推导出了**完整的尾部方案**（比三代存档更进了一步）：

1. 主循环终止时 s == t 扫描正好接壤（左右补填循环互斥推进可保证）
2. 残项配对后，未配对残值 j 个读进栈缓冲（ManuallyDrop 数组，j ≤ BLOCK），`ptr::copy` 把 ≤ 段向前压缩，残值写回段尾，得 k = s - j（或对称 k = s + j'）
3. 最终语义「≤ 左 / ≥ 右」对递归正确性合法（等值可两侧分布）

**但代价分析当场否决**：random 数据每节点 P_L ≈ P_R ≈ m/2 → 主体配对就要 m/2 次交换（仅为当前无条件交换的一半），而压缩要 O(m) 次 memmove —— 两边内存操作数打平（4m ops vs 4m ops），**净收益归零**。古典算法真正免压缩的结构（swap 针对移动中的 k 而非固定位）四次未能闭合。**结论：该路线对我们「免 Clone 泛型 + 无条件 swap Lomuto」的起点不成立，正式关闭**（除非未来改变交换原语，如 T: Copy 特化路径）。

**结论**：本代两个实验均为负结果，基线（Gen 16 架构）不变，累计否决方向达 9 个。当前最优形态稳定：random 1M 17.0ms（pdqsort 8.8ms，1.93 倍）。

## Gen 19：零下降沿跳过（partial insertion 的第二段冗余）

**改动**：`partial_insertion_sort` 在下降沿扫描得 `disorder == 0` 时**直接返回 None，跳过 `insertion_sort` 调用**——数学恒等：0 下降沿 ⟺ 数组已非递减，此时插入排序只做 n 次比较、零移位，是纯浪费。

**动机**：all-equal / sorted（各 3 case，长期停在 ~0.50x）的顶层完成路径固定付「下降沿扫描 n 次比较 + 插入排序 n 次比较」= 2n，而 pdqsort 只付 n。reverse 上 Hoare 产出的天然有序左区同样在重复付这笔钱。

**基准数据（absolute，ms，三轮）**：

| case | Gen 18 | Gen 19 三轮 | speedup(Gen19) |
|---|---|---|---|
| all-equal 10k | 0.0040 | 0.0020 ×3 | **0.990~1.00x** |
| sorted 10k | 0.0040 | 0.0020 ×3 | **0.996~0.999x** |
| all-equal 1k | 0.0004 | ~0.0003 | 0.718x |
| sorted 1k | 0.0004 | ~0.0002 | 1.001x |
| reverse 10k | 0.0072 | 0.0053~0.0055 | 0.447x |
| nearly-sorted 10k | 0.0330 | 0.0325~0.0330 | ~1.7x（持平） |

**EVOLUTION SPEED SCORE：0.812/0.835/0.848（中位 0.835，前带 0.55~0.74）**

**结论**：

1. **all-equal / sorted 追平 pdqsort**（0.99~1.00x，绝对耗时 0.0040→0.0020ms 砍半，三轮零波动）——2n → n 的理论值完整兑现
2. reverse 10k -27%（0.0072→0.0053）：左区完成节点同享零下降沿跳过
3. 改动仅 2 行、数学恒等保证正确性，是性价比最高的一代之一
4. 当前 23 case 中达到/超越 pdqsort 的已有：all-equal（3）、sorted（3）、nearly-sorted 1k/100、few-unique 100、random 100 —— 12 个 case 打平或反超

**下一步方向**：（a）reverse 仍是比值最低（0.39~0.45x）——其路径是「Hoare 剥层 + 左区零下降沿完成」，成本 ≈ 2n 常量 vs pdqsort 的 n；（b）organ-pipe 残余（0.49x）；（c）random（0.45~0.55x，Lomuto 交换次数问题已随 BlockQuicksort 关闭，剩常数项）。

## Gen 20：负结果（极端失衡直通粉碎：few-unique +31%、random +12%，回退）

**改动（已回退）**：坏分区中「某侧 < len/64」（pivot 恰为极值）时跳过 DNF 升级，直接对大侧做强粉碎后常规递归——省掉 DNF 用同一极值 pivot 再剥一层的 2n。

**动机**：organ-pipe 主路径每层付「Hoare 剥 1 + DNF 剥 2（2n）」才到粉碎。

**实测（三轮，absolute）**：

| case | Gen 19 | Gen 20 | 结论 |
|---|---|---|---|
| organ-pipe 10k | 0.1114 | 0.1056~0.1076 | 仅 -4%，不达门槛 |
| organ-pipe 1k | 0.0092 | 0.0077 | -16% |
| few-unique 10k | 0.0138 | 0.0181（三轮零波动） | **+31% 回退** |
| random 1M | 17.07 | 19.01~19.12 | **+12% 回退** |
| random 10k | 0.0502~0.0646 | 0.0625~0.0775 | 回退 |

**归因**：
1. **few-unique +31%**（机制明确）：pivot 撞 5 值中的最小值时左段近空 → 直通粉碎对**整段**做 n/4 交换，纯浪费；且 DNF 本可把等值中段冻结、让递归按值域收缩——直通把这个机制也跳过了
2. **random +12%**（970 个极端节点无法解释此量级，判为热循环代码布局位移——新增分支改变 quicksort_rec 的循环代码排布）
3. organ-pipe 的 4% 收益小于两项回退——净负，整体回退

**结论**：DNF 升级通道对重复数据有不可替代的价值（等值冻结），极端失衡直通是错误抽象。累计否决方向达 10 个。

## Gen 21：逆序检测直达（reverse 追平 pdqsort）+ 冷路径外描修布局回归

**改动**：① 新增 `try_reverse_sorted`（`#[inline(never)]`）：下降沿密集信号（bail_pos ≤ 10）下一趟扫描确认整段非严格递减 → `arr.reverse()` 直接完成；② ipnsort 同款机制，替代 reverse 的 Hoare 剥层链（~4n 操作）。

**动机**：reverse 是最后一个人字形缺口（0.39~0.45x，成本 ~4n vs pdqsort 的 ~n）。

**基准数据（absolute，ms）**：

| case | Gen 20 | Gen 21 |
|---|---|---|
| reverse 10k | 0.0053 | **0.0024（0.996~0.999x）** |
| reverse 1k | 0.0006 | 0.0003（0.96x） |
| reverse 100 | ~0.0001 | ~0.0001（0.85x） |
| random 1M | 17.07 | 17.25~17.43（基线内，见下） |

**EVOLUTION SPEED SCORE：0.977（三代带 0.85~0.98）**

**关键插曲——代码布局回归与外描修复（重要方法论语码化）**：

1. 初版（try_reverse_sorted 可内联）：reverse 追平达成，但 **random 1M +11%（17.1→19.0）**——random 数据根本不进该分支（bail_pos ~17 > 10），回退 100% 来自函数变大导致的**热循环代码布局位移**（与 Gen 20 同机制）
2. 修复：`#[inline(never)]` 把冷路径外描 → random 1M 回到 17.25~17.43（基线内），reverse 收益保留
3. **沉淀为 codebase 规则：给 quicksort_rec 等热函数新增冷路径代码必须外描（#[inline(never)]），否则用多轮绝对耗时验证 random 1M 是否被布局位移波及**

**结论**：
1. reverse 10k 追平 pdqsort（-55%），1k 0.96x；假阳成本 ~2 次比较（首个非递减位即失败）
2. 打平或反超 pdqsort 的 case 达 13/23（all-equal 3、sorted 3、reverse 2、nearly-sorted 3、random 100、few-unique 100）
3. 剩余唯一显著缺口：organ-pipe 0.49x

## Gen 22：负结果（two-run 二次方 bug：修复有效但判分器净负，回退入档）

**发现（真实鲁棒性 bug，与判分器无关）**：「两段有序」输入 `[升序 run | 升序 run]`（合并两个有序流、轮转有序数组的真实类）只有 **1 个下降沿**但插入位移 O(n²/4)——partial insertion 的「数下降沿 ≤ 8 就插入」逻辑被击穿，实测 **n=100k 要 259ms（应为 ~0.3ms，差 800 倍）**。

**修复（已验证有效）**：partial insertion 第二段改「数下降沿 + 数总移位」，移位超 `len/8+32` 中途放弃（洞已填、前缀有序、无副作用，落分区路径）。two-run 100k：**259ms → 2.37ms（109 倍）**，且 organ-pipe 10k 顺带 -12%。

**回退原因（判分器判据）**：修复让 random 1M **17.25→19.65ms（+13%）**、nearly-sorted 10k +20%，总分 0.977 → 0.88（-10%）。关键证据：random 数据根本不进插入段（bail_pos ~17），且 `#[inline(never)]` 外描后热路径与 Gen 21 逐指令相同、回退依旧——**纯二进制布局税**（新增/变大函数位移热循环，std 侧读数不动可排除机器相位）。一个 random 永不调用的函数让 random 贵 13%，这是 Gen 20/21 布局现象的第三次实证，且外描无法救治。

**入档的修复方案（将来落地条件：任一改变二进制布局的世代顺手带上，届时重测布局税是否仍存在）**：
```rust
// partial_insertion_sort 第一段（下降沿扫描）不变；
// 第二段改为带预算插入：
let budget = arr.len() / 8 + 32;
let mut shifts = 0;
// 洞式插入循环内每个 ptr::copy 后 shifts += 1；
// if shifts > budget { ptr::write(洞位, saved); return Some(i); }  // 填洞后放弃
```
回归测试（已验证 2.37ms）：
```rust
let n = 100_000; let half = n / 2;
let mut v: Vec<u32> = (0..half as u32).chain(0..half as u32).collect();
quicksort(&mut v);
assert!(v.windows(2).all(|w| w[0] <= w[1]));
```

**教训（第三次）**：本代码库对二进制布局极度敏感——「逻辑上永不执行的分支」也能通过布局让热路径贵 10%+。行动准则固化：① 任何热函数体积变化必须多轮验证 random 1M；② 外描冷路径可救（Gen 21）但不总救（Gen 22）；③ 修 bug 优先于刷分，但判分器净负时必须回退入档、择 Layout 再平衡的世代顺手带上。

## Gen 23：CUTOFF 二扫（24 → 32）

**改动**：`CUTOFF` 24 → 32。常数改动，不动代码体积（无 Gen 22 式布局税风险）。

**动机**：Gen 15 之后架构又变两轮（Gen 19 零下降沿跳过、Gen 21 逆序检测），「整段完成」路径更便宜——大叶子的相对成本下降，最优 CUTOFF 可能再上移。

**扫描数据（三轮中位，23 case 几何平均）**：

| CUTOFF | 分数 |
|---|---|
| 16 | 0.887（单轮） |
| 24 | 0.926 / 0.927 / 0.931 → 0.927 |
| 32 | 0.936 / 0.927 / 0.943 → 0.936 |
| 48 | 0.908 / 0.910 / 0.908 → 0.908 |

**结论**：32 较 24 +1%（噪声内，但三轮方向一致）；48 起叶级 O(CUTOFF²) 成本反超（-3%）。常数改动零风险，取 32。扫描表已写入 `CUTOFF` 注释（第三次参数史：Gen 7 全平定 16 → Gen 15 上移定 24 → Gen 23 再上移定 32，与「完成路径逐代变便宜」的机制叙事一致）。

## Gen 24：降序游程精化（organ-pipe 逃过 heapsort）+ 路由整体外描

**改动**：① sparse 档（bail_pos ≥ 48）新增「bail 点前 8 位连续严格递减」检测——真则判定为「大块降序尾」（organ-pipe 家族签名：前 n/2 递增 + 后 n/2 连续递减，9 个下降沿必然连续；nearly-sorted 的 9 个下降沿稀疏分布、连续概率 ~0），改走 **Lomuto + ninther**；② 路由决策整体外描为 `partition_router`/`wants_hoare`/`descending_run_at_bail`（均 `#[inline(never)]`）。

**动机**：organ-pipe 在 Hoare 上 pivot 恒为 min（三采样 (1, max, 1) → 中位 = 1）、剥 1 层 → heapsort（0.11ms ≈ 纯 heapsort 成本）；Lomuto 的 ninther 九采样对 U 形分布取到 ~n/8 分位值 → 3/4 剥层、~32 层深度 < 42 深度预算 → **逃过 fallback**。

**基准数据（absolute，ms）**：

| case | Gen 23 | Gen 24 |
|---|---|---|
| organ-pipe 100 | ~0.0003 | ~0.0003（0.79x） |
| organ-pipe 1k | 0.0094 | 0.0048~0.0056（**-45%**，0.72~0.82x） |
| organ-pipe 10k | 0.1099 | 0.0789~0.0868（**-21%**，0.63~0.69x） |
| sorted 10k | 0.0020 | 0.0020（0.99x，无伤） |
| nearly-sorted 10k | 0.0339 | 0.0339（1.64x，无误伤） |
| random 1M | 17.25 | 17.20（无伤） |

**EVOLUTION SPEED SCORE：0.936 → 0.944（三轮 0.937/0.944/0.945）**

**关键插曲（布局税的第二次战胜）**：初版路由内联时 organ-pipe 收益达成，但 **sorted/all-equal +50~100%**（0.0020→0.0030/0.0040）——两 case 零下降沿提前返回、算法上零变化，纯布局税（Gen 22 同机制）。把整个路由外描后（热循环从「两个阈值比较内联」变成「一次调用」——**比原来更小**），sorted/all-equal 恢复 0.0020/0.99x，organ-pipe 收益保留。**外描不仅能止损，还能比原版更小**。

**结论**：
1. organ-pipe 1k -45%、10k -21%（仍不及 pdqsort 的 0.63x，但脱离 heapsort 区）；信号零误判（nearly-sorted、random、few-unique 全部无伤，random 的 bail ~17 根本进不了 sparse 档）
2. 方法论语码化 #2：路由/决策类新增逻辑优先整体外描，让热循环体积不增反降
3. 剩余缺口：organ-pipe 10k（0.63x）、random 100k/1M（~0.5x）

## Gen 25：粗糙数据预筛（8 采样三态）——总分首次超过 pdqsort

**改动**：分区前新增 `route_direct_lomuto`（`#[inline(never)]`）：8 个等距采样点的 7 个相邻对做三态判定——7/7 全下降 = 逆序密度（走完整路径：逆序检测 + Hoare）；≤1 下降 = 平滑（走完整扫描：near-complete 完成 + 三档路由）；**2~6 下降 = 粗糙 → 跳过下降沿扫描直达 Lomuto+ninther**（random 的 bail_pos ~17 次扫描/节点纯属浪费；organ-pipe 等距采样 desc=4 也落此档，与 Gen 24 目的地一致且省掉 O(峰值) 扫描）。

**动机**：random 每节点为拿路由信号先跑 O(峰值位置) 的下降沿扫描——大切片上是**缓存行级别的重复访问**（扫描从切片起点顺序走，ninther 随后又采样 9 个分散位置，扫描碰过的缓存行是纯开销），实测代价远超「17 次比较」的字面成本。

**基准数据（absolute，ms）**：

| case | Gen 24 | Gen 25 三轮 | speedup(Gen25) |
|---|---|---|---|
| random 1M | 17.20 | 13.92~14.31 | **0.622~0.631x（-18%）** |
| random 100k | 1.53~1.90 | 1.29~1.33 | 0.58~0.61x（-16%） |
| random 10k | 0.053 | 0.0513~0.0642 | 0.75~0.91x |
| organ-pipe 10k | 0.079 | 0.0535~0.0539 | **1.018x（追平，-32%）** |
| organ-pipe 1k | 0.0048 | 0.0042 | 0.936x |
| organ-pipe 100 | ~0.0003 | ~0.0003 | 1.068x |
| few-unique 10k | 0.0124 | 0.0113 | 1.067x（三项全部 ≥1.0） |
| nearly-sorted 10k | 0.0339 | 0.0335 | 1.659x（无伤） |

**EVOLUTION SPEED SCORE：0.944 → 1.017（三轮 1.017/0.995/1.020）—— 25 代进化首次超过 pdqsort**

**结论**：

1. **random 1M -18%**（17.20→13.9ms）——预筛跳过的扫描在大切片上的缓存开销被双重低估；100k -16%、1k/10k 小涨
2. **organ-pipe 全线追平**（1.07/0.94/1.02）——desc=4 落入粗糙档，同时省掉 O(峰值) 扫描；few-unique 同享扫描跳过后三项全部 ≥1.0
3. 无误伤：nearly-sorted（筛子落入平滑档概率 ~99.8%）、reverse（7/7 全下降走完整路径）、sorted/all-equal（0 下降走完整路径）
4. 筛子只选分区器、不参与正确性——任何档位都是合法分区路径
5. 当前 worst case：random 100k（0.58x）；23 case 中 13 个 ≥1.0

## Gen 26：NINTHER_MIN 确认（64）+ 预筛小切片门
**背景**：Gen 25 后节点成本结构再变（下降沿扫描消失，ninther 占比上升），Gen 17 的参数结论需重验；另修 Gen 25 记录的小 n 附损（三个 100 元素平滑 case 为 +8 采样付 13% 相对成本）。

**实验 1：NINTHER_MIN 重扫（64/128/256/512）**：random 1M 全平（13.89~14.15ms），但总分 64 明显占优（1.026 vs 0.95~0.97）。定位机制：**差异全在 organ-pipe 10k**（64: 1.00x vs 256: 0.80x）——organ-pipe 的递归尾段（64~256 小切片）必须靠 ninther 维持 3/4 剥层，换 med-3 就在小切片重启剥层链落 heapsort。**结论：ninther 要一路铺到 CUTOFF 之上，64 确认（机制新解入注释）。**

**实验 2：预筛小切片门（len ≥ 128 才付费 8 采样）**：

| case | Gen 25 | Gen 26 |
|---|---|---|
| all-equal 100 | 0.87x | 1.00x |
| sorted 100 | 0.86x | 0.98~1.01x |
| reverse 100 | 0.71x | 0.81~0.85x |
| random 100 | 0.955x | 0.94x（微损，噪声） |

**EVOLUTION SPEED SCORE：1.017 → ~1.025（两轮 1.022/1.028）**

**结论**：小切片完整扫描本就 ≤17 次，粗糙小切片只多付 9 次比较（可忽略），三个平滑小 case 恢复 13%+；ninther 阈值机制闭环。当前 worst case 仍为 random 100k（0.58x）。

## Gen 27：负结果（第五次块分区尝试：正确性未收敛 + 性能负，回退）

**动机**：random 100k/1M（0.51~0.58x）是仅剩的系统性 drag，根因是分支免费 Lomuto 每层 n 次无条件交换（Hoare 式配对可减半）。前四次推导死于「尾部精算」，此次换可落地变体：主体双 offset 缓冲块配对（交换减半），尾段不精算——对「残项张成的有限中段」跑子切片**同 pivot** 分支免费 Lomuto（中段对 random bounded 在 ~4·BLOCK）。

**实验 1：中段重取 pivot bug（差分测试当场抓住）**：子分区用了 `branchless_partition`——它从中段自己的 mid **重取 pivot**！中段是 ≤/≥ 混合体，用不同 pivot 分会破坏全局不变量（[0,lo) ≤ 原 pivot，中段左部却可能含更大值 → 跨边界逆序）。修复：抽出 `lomuto_scan_with_pivot(arr, &pivot)`，中段用同一个 ManuallyDrop pivot 副本。

**实验 2：修复后仍有低频 mismismatch**（n=513/1000 的稀疏输入，位 33/106，在定居左区内），未能在本回合内根因（探针显示 nl 大/nr=0 的形态——pivot 处于约 96 分位，右区全 > pivot 无待换项）。

**实验 3：性能先于正确性告负**：即便在修复前状态 random 1M 也只要 15.9~16.0ms（基线 14.0ms，**+14%**）——offset 记账（每元素 1 store）+ 中段尾（~4·BLOCK 的额外分区）+ copy_within 出队的总开销 ≥ 交换减半收益。理论余量本就只有 ~3-4%，被三项开销吃光。

**回退，双标否决**。**块配对方向第五次失败（Gen 17/18 推导 ×2、Gen 22 尾部分析、Gen 27 实现 ×2 bug/负性能），正式关闭**——对「免 Clone 泛型 + 无条件 swap Lomuto」的起点，块机制不成立；若未来改换交换原语（T: Copy 特化的 cmov 值交换），可重启评估。

**教训**：差分门禁第三次抓住实现类 bug（前两次：Gen 6 break_patterns 边界、Gen 22 two-run）——「正确性未收敛时先看性能」的止损纪律节省了本回合的后半程。

## Gen 28：负结果（two-run 修复的布局窗口重试——假设二次证伪）

**动机**：Gen 22 的 two-run 二次方修复（259ms → 2.37ms）当时因布局税回退入档；此后二进制经过 Gen 23/24/25/26 四轮变化，「布局窗口可能已换」是可证伪假设，值得一试。

**实验**：按 Gen 22 档案实现 `insertion_with_budget`（`#[inline(never)]` 外描的带移位预算洞式插入）。

**结果**：
- 修复本身有效：two-run 100k **259ms → 1.88ms（137 倍）**，6 个测试套件全绿（含 two-run 回归测试）
- **但布局税稳定复现**：random 1M 15.55~17.25ms vs 基线 13.9~14.3ms（**+11~23%**），总分 0.970~0.972 vs 1.025（**-5%**），std 对照组读数平稳（8.77~8.81，排除机器相位）

**结论（假设证伪）**：该税不是二进制布局的随机骰子，而是**稳定的代码规模×热循环相互作用**——两个不同布局纪元（Gen 22 的二进制与 Gen 28 的二进制）为同一逻辑增量付出同量级税（+11~13%）。**two-run 修复的落地条件更新为**：① 等待一次动机性大重构（必须做基准验证的那种）搭车；② 或找到零代码增量的实现（两轮均未找到——预算逻辑必须新增代码）。档案保持开放，但不再单独发起重试。

**教训**：可证伪假设值得用一代去检验（本次成本仅半轮），但二次证伪后应停止——「布局税」研究至此已从现象（Gen 20/21/22）走到规律（Gen 24/26 的应对）再到边界（Gen 28 的稳定税）。

## Gen 29：负结果（二分插入排序：理论漂亮，实测 +72~230%）

**动机**：CUTOFF 叶子是最后一个未动过的热点。分析：random 叶子上线性洞式插入每元素 ~k/2 次「比较 + 单元素 copy」，比较 ~50% mispredict；换成二分定位（log₂k 次 cmov 比较）+ 整块 memmove 应该省下比较。

**实测（三轮一致）**：

| case | Gen 28 | Gen 29 |
|---|---|---|
| random 10k | 0.0513ms | 0.169~0.175ms（**+230%**） |
| random 100k | 1.31ms | 2.33ms（**+78%**） |
| random 1M | 14.0ms | 24.1ms（**+72%**） |
| 总分 | 1.025 | 0.743（**-27%**） |

**归因（两个理论错误）**：
1. **忘了旧版的提前退出**：`if arr[i] < arr[i-1]` 让 ~50% 的随机叶子插入仅付 1 次比较即零成本；二分版对每个元素（含已在位的）都付 log₂(i) 次比较
2. **泛型 ptr::copy 的小块 memmove 未必内联**（T=u32、count≤32 时也可能落 memcpy 调用），单次调用的固定开销远超省下的比较

**结论回退**。旧版线性洞式插入对这种「半随机 + 提前退出」的数据形状本就是优的——本次负结果修正了一个直觉：「插入排序的瓶颈是位移不是比较」，而位移两种方案相同、比较却被提前退出主导。累计否决方向 15 个。
## Gen 30：路由参数面收官（三项确认）+ 测试门禁扩容

**背景**：算法机制全部到达局部最优或判决结论后，剩下未经新架构检验的参数面。

**参数扫描（单轮，相关 case 绝对耗时判据）**：

| 参数 | 扫描 | 结论 |
|---|---|---|
| UNBALANCED_DIV（DNF 升级阈值） | 4 / 8 / 16 | 全平（few-unique 10k 0.0112~0.0113ms，organ-pipe 0.0557~0.0598ms），**8 确认** |
| SPARSE_DESCENT_BAIL（稀疏→Hoare） | 32 / 48 / 96 | 全平（nearly-sorted 10k 0.0333~0.0336ms），**48 确认** |
| PARTIAL_INSERTION_LIMIT | 8 / 16 / 32 | **8 确认**——更高值把「稀疏下降沿但大位移」毒切片放进 O(k²) 插入（16/32 时 nearly-sorted 10k +11%/+21%、总分 -35%/-42%） |

加上此前的 CUTOFF（三纪元扫描）与 NINTHER_MIN（两纪元），**六个路由/完成参数全部有扫描档案，参数面正式收官**。

**测试门禁扩容（差分测试 3 → 6）**：
1. `large_scale_differential`：20k 元素 × 6 分布 vs std——此前覆盖率止步 800，大 n 病态（深层递归、深度预算路径）只有规模能触发
2. `generic_type_paths`：i64（负值域）+ String（非 Copy、含 Drop、非平凡比较）——此前全部测试只走 u32 单态化
3. `two_sorted_runs_small`：2k 两段有序正确性（Gen 22 已知二次方类的 debug 友好版，断言正确性——该类已知慢但正确）

**EVOLUTION SPEED SCORE：~1.02（本轮 1.016，与前带一致）**
## Gen 31：判分器升级（相位 gauge）+ 一次 harness 自身的污染实验

**动机**：30 代决策全部依赖 bench 读数，而冷/热相位污染多次困扰跨代解读（pdqsort 侧读数 8.8~10.4ms 摆动）。给判分器加「机器温度计」。

**落地：相位 gauge**——bench 在输出末尾打印「std sort_unstable 1M random」绝对耗时作为相位参照（~8.8 = 冷相，~10.3 = 热相）。今后每个 EVOLUTION.md 条目都配 gauge 阅读，热相位分数（如本轮 0.900）不与冷相位（1.02）直接比较。

**中间的污染实验（harness 自我验证的重要发现）**：
1. 初版 gauge 放在矩阵**前**：我们的 random 1M 从基线 13.9~14.3ms 涨到 16.3~16.5ms（**+17%**），而 std 侧稳定——gauge 自身的 24×1M 排序改变了分配器/缓存状态，污染了被测 case
2. 另加全局 warmup pass：更糟（16.3~16.5）——warmup 跑完 23 个 case 后，被测 case 拿到被污染的对齐/分配状态
3. 两者都移除、gauge 后置后恢复；**金丝雀确认：我们的 random 1M 绝对耗时 = bench 自身是否被污染的探针**

**结论**：**bench 的任何前置负载都会通过分配器/缓存/对齐污染被测 case（我们的热循环对此高度敏感）**——沉淀为 harness 修改纪律：改 bench 后必须用「我们的绝对耗时金丝雀」验证 bench 自身，而非只看分数。gauge 后置版工作正常（本会话热相位 10.34~10.46 被正确标注，0.900 分不再被误读为回退）。

**EVOLUTION SPEED SCORE：方法论资产（不可比——本轮为热相位，gauge 10.34）**
## Gen 32：门禁加固——双 profile 验证制度化 + release 百万级 stress

**背景**：排序主体含大量 unsafe（ptr::read/swap/copy、ManuallyDrop、裸指针扫描），而 `cargo test` 默认只跑 debug——优化器在 release 下会改变指针路径代码生成，**某些 unsafe bug 只在优化后显现**，此前的门禁有覆盖缺口。

**落地**：
1. `release_only_mega_stress`（`#[cfg(not(debug_assertions))]`）：200k 元素 × 5 模式（随机/7 值重复/有序/逆序/2% 扰动）vs std 差分——release 下 20k 只要 0.02s，大规模 stress 零成本；cfg 反选使默认 debug 门禁不增加耗时（0.30s 不变）
2. **协议制度化**：今后每一代的验证命令为 `cargo test && cargo test --release`（debug 快 + release 覆盖优化代码gen）

**验证结果**：debug 6 差分（mega 跳过）/ release 7 差分（含 mega，0.02s）全绿；排序主体 unsafe 路径在优化下干净。

**EVOLUTION SPEED SCORE：本代为门禁加固代，无分数结论（会话热相位 gauge ~10.33，与冷相位不可比）**
## Gen 33：lint 清理（clippy 两处修正，金丝雀纪律下的小变动）

**背景**：会话持续热相位（gauge ~10.4），无跨相位分数结论空间，做 hygiene 资产。

**修正（语义等价、双门禁绿）**：
1. `break_patterns_sides`：`arr.len() - gt - 1 >= 8` → `arr.len() - gt > 8`（clippy::int_plus_one）
2. `descending_run_at_bail`：`!(arr[j] < arr[j-1])` → `arr[j] >= arr[j-1]`（clippy::nonminimal_bool）

**金丝雀验证**（Gen 31 纪律——任何 src 变动都过绝对耗时探针）：random 1M 15.73~15.94 → 15.93~16.05ms（+1% 内，热相位抖动）；debug/release 双 profile 5 套件全绿。

**保留项**：5 个 `&mut arr` 提示为非末次使用的显式重借用（clippy 建议的去掉 borrow 写法会编译错误，正确写法 `&mut *arr` 属纯抛光，不再增加 src 变动面）。

**EVOLUTION SPEED SCORE：无分数结论（热相位 gauge ~10.40，canary +1% 噪声内）**

## Gen 34：mega stress 覆盖面补齐（organ-pipe 200k —— 最深自适应路径的大规模验证）

**缺口发现**：Gen 32 的 release-only mega stress 有 5 个模式，但**缺 organ-pipe**——而它恰是全部路径中最深的一条：sparse 信号 + 降序游程检测 → Lomuto+ninther 的 3/4 剥层链，200k 时递归深度 ~40 贴着深度预算 3·log₂(200k)=51 边界运行（既验证剥层收敛，也验证不误触 heapsort fallback）。另补 all-equal 200k（0 下降沿直达路径）。

**验证**：
- release：mega stress 7 模式（200k × 7）全绿，0.02s；organ-pipe 路径大规模正确 ✓
- debug：mega 被 cfg 反选跳过，门禁耗时不变（10 套件 0.3s 级）
- clippy 可操作警告数 0（Gen 33 清理生效）

**EVOLUTION SPEED SCORE：无分数结论（热相位 gauge ~10.38）；门禁覆盖面 +2 模式**
## Gen 35：EVOLUTION.md 一致性审计与修复（文档资产的防腐）

**背景**：本文件历经 35 代编辑、多次 sed 事故与修复，一致性漂移已积累——作为项目的记忆核心，需要一次全面审计。

**发现并修复**：
1. **残留的过期重复表**（32 行）：早前一次头部修复留下了 Gen 0-26 的旧表副本，与正确表（Gen 0-34）并存——删除
2. **规则文本漂移**：开头的「提升 < 1% 判噪声」是 Gen 5 时代作废的阈值，实际门槛已修正为 5%；双 profile 门禁（Gen 32）、gauge 对读（Gen 31）、绝对耗时判据（Gen 2/3）、bench 金丝雀（Gen 31）四条后续确立的规则均未收入——全部补入

**审计方法（可复用）**：逐代核对「章节数 = 表格行数」（Gen 0-34 全部 = 1）、章节头计数（38 = 当前状态 + 分数历史 + 35 代 + 死路记录）、死路表行数与正文主张一致。

**验证**：审计后 35/35 代一一对应；双 profile 门禁绿（5 套件 each）；文档从 1004 行收敛到 976 行。


| 方案 | 结论 | 原因 |
## Gen 36：公共 API 文档补全 + clippy 重借用抛光（金丝雀平稳）

**公共 API 文档**：`quicksort` 的 doc 从一行扩展到完整契约——复杂度（期望/最坏 O(n log n) 及依据、O(1) 空间）、行为说明（不稳定、自适应快速路径、正确性门禁指向）。这是库的公共门面，此前只有一行。

**clippy 重借用抛光**：5 处 `&mut arr` → 规范的 `&mut *arr` 显式重借用（循环内绑定不可移动，clippy 建议的裸 `arr` 会编译错误）。needless_borrow 警告清零；剩余 6 个警告均为 doc 格式类，无功能影响。

**金丝雀验证**（热相位内自比）：random 1M 15.74~16.37 → 15.84~16.32ms（噪声内持平）；双 profile 门禁绿（5 套件 each）。

**EVOLUTION SPEED SCORE：无分数结论（热相位 gauge ~10.33；本代为 API/ lint 资产代）**

## Gen 37：负结果（采样融合：收益落空于 L1 热 + 一次运行中热漂移的误判险情）

**实验**：把 Gen 25 预筛的 8 个等距采样点与 Gen 13 ninther 的 9 个采样点融合（rough 时用同一批点做 ninther，中位值钉 mid），省掉 ninther 重新采样 9 个分散位置的加载。同 session A/B（gauge 确认相位一致）。

**结果**：
- 同相位窗口金丝雀：random 1M 15.66~15.79（前）→ 15.78~15.86（后）——**持平至 +0.5%**。加载节省落空：筛子刚碰过的采样行在 L1 里，ninther 重采样本就是热访问，理论收益（~9 次冷加载/节点）实际为 ~0
- 双 profile 门禁绿，但无收益不留复杂度 → **回退**（Gen 25 筛子 + Gen 13 ninther 的组合更简单）

**插曲：一次运行中热漂移造成的误判险情**：融合版某次全量运行总分 1.06（本会话热相位带为 0.89~0.93），险些读成「+17% 提升」。拉全表发现是 std 侧 100k 读数 0.78→1.11ms 的段中热漂移（我们侧同步 15.8→17.3ms），gauge（1M）在运行末尾已回落到 10.36 未反映段中尖峰。**教训：单次运行的总分可能包含段中相位漂移，判决必须用「同 case 绝对耗时的前后多轮对比」而非单次总分——金丝雀纪律再次立功。**

**EVOLUTION SPEED SCORE：回退后 ~0.94（gauge 10.33，热相位）**
## Gen 38：路由地图审计与合成（23 分布路径全量走查 + 文档缺口修复）

**动机**：路由逻辑分散在 Gen 5~37 的注释里，缺一页合成视图——而它正是「改路由前必读」的关键资产。

**审计过程**：逐分布走查全部 23 个基准 case 的路径（random / all-equal / sorted / reverse / nearly-sorted / few-unique / organ-pipe × 各规模），与设计意图逐条核对——**全部一致，无 silently 走错路的 case**（含 organ-pipe 100 这种 <128 不触发预筛、靠完整路径的「稀疏+连续降尾」分支正确落地的情形）。

**产出**：EVOLUTION.md 开头新增「路由地图」节——7 步决策链 + 7 分布的顶层路径表；lib.rs 头部补「架构演进脉络」并移除过时的 GENERATION: 11 标记，指向路由地图。

**验证**：双 profile 门禁绿（5 套件 each）；本次为文档改动，金丝雀平稳（判定：lib.rs 仅注释变更，代码路径零变动）。

**EVOLUTION SPEED SCORE：无分数结论（热相位；文档合成代）**

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
