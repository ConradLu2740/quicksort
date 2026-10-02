# quicksort — 「快排进化论」

一个泛型 `T: Ord` 的**原地、不稳定、零分配**快速排序（Rust），外加 53 代
完整演化日志：每一代的改动、数据、归因，含死路表与路由地图。

这不是一个"写个快排"的项目，而是一个**用 53 代迭代 + 四家独立外部评估
把实现逼到证据边界**的实验：所有性能声明都附带口径，所有被推翻的结论
都公开撤回并记录原因（含外部误判）。

## 快速开始

```bash
cargo test                # debug 门禁（12 套件）
cargo test --release      # release 门禁（含 200k mega stress）
cargo run --release --bin bench   # 判分器 + 三组 informational 体检
```

`bench` 输出四类信息，**都必须绑定口径阅读**：

| 行 | 含义 |
|---|---|
| `SPEED SCORE` | 23 分布几何平均对标 std `sort_unstable`（ipnsort，rustc ≥1.81），n ≤ 10k，热端 |
| `info few-unique seed-sweep` | 重复密集输入的种子方差体检（单种子读数不是统计量） |
| `info cold/warm` | 冷数据 vs 同数据重复的对照（热身偏差只存在于 n ≤ 1e4） |
| `info scale-ext` | 规模扩展到 1e5/1e6（小规模≈打平、大规模慢两成的真相在这里） |

## 性能（对标 std `sort_unstable`，全是实测，无单点外推）

| 切面 | 结果 |
|---|---|
| 23 分布（n≤10k，热端） | ~1.0x（≈打平） |
| 7 分布 × 1e5/1e6 | ~0.84x（慢约两成） |
| 冷数据 random（n=1k/10k） | 0.48~0.56x |
| **nearly-sorted（全规模）** | **1.31~1.35x（唯一确定赢项）** |
| two-run / few-unique 大基数 | 0.49x / 0.38x（已知弱项） |

诚实声明：**任何单一总分都没有对外意义**——必须绑定（规模区间 × 冷热口径
× 分布集）三要素，这是四轮外部评估打出来的共识。

## 安全与正确性证据

- 双 profile 门禁 + clippy + `cargo fmt --check` 三门禁
- **Miri Stacked Borrows + Tree Borrows 双模型干净**（含 panic 注入路径）
- panic 安全：`cmp` panic 时所有权守恒 + 合法排列（测试证据分层，见
  `src/lib.rs` 契约注释），约 16 万次外部注入点旁证
- 非 Copy/Drop 类型分区路径覆盖（`tests/branchless_drop.rs`，n=100/10000
  × 多种子 + String 堆载荷 + 恰好一次 drop）
- 非全序/非自反 `Ord` 契约实测（`tests/non_reflexive_oob.rs`）
- 四家独立外部评估史：命中 8 项真实问题（全部修复/撤回/量化）、误判 3 项
  （Miri 裁决记录在案）——见下面「文档」

## 仓库结构

```
src/lib.rs           # 全部算法（约 800 行，含 SAFETY 论证与代际注释）
src/bin/bench.rs     # 判分器 + 三组 informational 体检
tests/               # 差分 / panic 安全 / 非 Copy 分区 / 非自反 / exotic / two-run
EVOLUTION.md         # 53 代演化日志：分数表、路由地图、死路表、评估史
share/               # 外发评估包：自包含单文件源码 + 委托文档 v5 + 四家外部评估报告原文
```

**必读**：改路由或参数前先读 `EVOLUTION.md` 开头的「路由地图」与文末
「死路表」；18+ 个方向已被实测否决，每个都有机制级归因。

## 已知弱项与下一步

- random 大数组慢约两成（外部两家证据指向 DNF 重扫的额外内存趟数，
  不是比较次数——organ_pipe 少 4% 比较却慢 43% 的反例已入档）
- 重复密集输入的 DNF 双峰（5 倍种子悬崖）
- **Gen 54 候选**：重复密集数据直接进三路分区（branchless 循环顺带统计
  等值数，pdqsort/ipnsort 同款），同时瞄准上述两个弱项

License：MIT（见 [LICENSE](LICENSE)）。
