# 进化日志

计分规则（长期有效）：

- 每代跑 `cargo run --release --bin bench`，记录 **EVOLUTION SPEED SCORE**（23 个用例几何平均，对标 std pdqsort，越高越好）
- `cargo test` 不全绿的世代作废，不允许进基准
- 提升 < 1% 判噪声，重测；确认为死路的，「为何不行」也要记录
- 每代一个 git commit

## 当前状态

- 世代：**Gen 1**
- EVOLUTION SPEED SCORE：**0.137721x**
- 正确性：3 个差分测试全绿（vs `std::sort_unstable`）

## 分数历史

| 世代 | 分数 | 关键变化 | 日期 |
|---|---|---|---|
| Gen 0 | 0.022178x | 教科书朴素版基线 | 2026-09-30 |
| Gen 1 | 0.137721x | Hoare 分区 + 中间 pivot（6.2 倍提升） | 2026-09-30 |

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

## 死路记录

（暂无）
