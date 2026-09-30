//! 基准输入生成器 —— 判分器（bench）与正确性门禁（tests/differential.rs）共用，
//! 保证两边喂给排序的输入完全一致。固定 seed，结果可复现。

/// 确定性 PRNG（splitmix64），零依赖、跨平台可复现。
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }
}

/// 基准覆盖的数据分布。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dist {
    // 随机均匀 —— 平均性能锚点
    Random,
    // 全等 —— Lomuto / 末元素 pivot 的灾难场景（分区永远只剩 n-1）
    AllEqual,
    // 仅 5 种不同值 —— 海量重复的代表场景
    FewUnique,
    // 已排序 —— 末元素 pivot 的灾难场景（pivot 永远是最大值）
    Sorted,
    // 逆序 —— 末元素 pivot 的镜像灾难
    Reverse,
    // 近乎有序（1% 扰动）—— 现实最接近的一类
    NearlySorted,
    // 风琴形 1..k..1 —— 静态 median-of-3 killer（Musser 构型）：
    // 首/中/尾三点的中位数恒为 1，pivot 恒取到极小值，分区极度不均
    OrganPipe,
}

pub const DISTS: [Dist; 7] = [
    Dist::Random,
    Dist::AllEqual,
    Dist::FewUnique,
    Dist::Sorted,
    Dist::Reverse,
    Dist::NearlySorted,
    Dist::OrganPipe,
];

impl Dist {
    pub fn name(self) -> &'static str {
        match self {
            Dist::Random => "random",
            Dist::AllEqual => "all-equal",
            Dist::FewUnique => "few-unique",
            Dist::Sorted => "sorted",
            Dist::Reverse => "reverse",
            Dist::NearlySorted => "nearly-sorted",
            Dist::OrganPipe => "organ-pipe",
        }
    }

    /// 各分布的规模上限。病态分布在朴素实现上是 O(n²)：
    /// 100k 病态输入会让 Gen 0 跑几分钟甚至几小时，因此病态分布固定测到 10k。
    /// 此上限从 Gen 0 起恒定不变，保证跨代可比。
    pub fn sizes(self) -> &'static [usize] {
        match self {
            Dist::Random => &[100, 1_000, 10_000, 100_000, 1_000_000],
            _ => &[100, 1_000, 10_000],
        }
    }

    /// 计时迭代次数：小输入多跑取中位数，大输入少跑控总时长。
    pub fn iters(self, n: usize) -> usize {
        match n {
            100 => 2_000,
            1_000 => 500,
            10_000 => match self {
                Dist::Random | Dist::NearlySorted => 100,
                _ => 50,
            },
            100_000 => 10,
            1_000_000 => 3,
            _ => 10,
        }
    }

    /// 生成该分布的确定性输入（u32）。
    pub fn make(self, n: usize) -> Vec<u32> {
        let mut rng = Rng::new(
            0xC0FF_EE00
                ^ (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ ((self as u64) << 32),
        );
        match self {
            Dist::Random => (0..n).map(|_| rng.next_u32()).collect(),
            Dist::AllEqual => vec![42; n],
            Dist::FewUnique => (0..n).map(|_| rng.next_u32() % 5).collect(),
            Dist::Sorted => (0..n as u32).collect(),
            Dist::Reverse => (0..n as u32).rev().collect(),
            Dist::NearlySorted => {
                let mut v: Vec<u32> = (0..n as u32).collect();
                if n > 1 {
                    let swaps = (n / 100).max(1);
                    for _ in 0..swaps {
                        let i = (rng.next_u64() as usize) % n;
                        let j = (rng.next_u64() as usize) % n;
                        v.swap(i, j);
                    }
                }
                v
            }
            Dist::OrganPipe => (0..n)
                .map(|i| {
                    let half = if i < n / 2 { i } else { n - 1 - i };
                    (half + 1) as u32
                })
                .collect(),
        }
    }
}
