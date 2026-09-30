# 进化日志

计分规则（长期有效）：

- 每代跑 `cargo run --release --bin bench`，记录 **EVOLUTION SPEED SCORE**（23 个用例几何平均，对标 std pdqsort，越高越好）
- `cargo test` 不全绿的世代作废，不允许进基准
- 提升 < 1% 判噪声，重测；确认为死路的，「为何不行」也要记录
- 每代一个 git commit

## 当前状态

- 世代：**Gen 0**
- EVOLUTION SPEED SCORE：**0.022178x**
- 正确性：3 个差分测试全绿（vs `std::sort_unstable`）

## 分数历史

| 世代 | 分数 | 关键变化 | 日期 |
|---|---|---|---|
| Gen 0 | 0.022178x | 教科书朴素版基线 | 2026-09-30 |

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

## 死路记录

（暂无）
