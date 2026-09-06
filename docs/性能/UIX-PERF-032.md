# UIX-PERF-032：软件动态扩展引擎参考负载基线

2026-09-06 在 Linux / Cachyos（rustc 1.98.1 stable）上，对 P1-P3 交付的
Scheme 扩展引擎（`extensions` feature）建立参考负载基线。这是新能力的
首份量化档案，不是优化决策：登记冷装载、类型化命令端到端与解释器步速，
供后续回归对照与 P0 合同资源上限的校准依据。

## 场景与构建

- 框架基线：`798faa8d`（P3 提交后）；扩展引擎以同主人小贝
  `agent/src/scheme/`（`0fa72644`）移植并补齐标准缺口，无其它变更。
- 参考负载 A（无窗口纯逻辑扩展）：文本转换命令（`reverse` +
  `string->list/list->string`）+ 状态计数；负载 B：尾递归计数循环；
  负载 C：循环 pair 分配丢弃（回收参与）。
- 计量：`ExtensionHost`（同步模式、默认配额放宽燃料与墙钟）内
  `Instant` 计时；release 与 dev（debug）双口径，各重复采样。
- 复现：把 `docs/性能/evidence/UIX-PERF-032-bench.rs` 复制为
  `tests/extension_bench_public_api.rs` 后
  `cargo test --release --features extensions --test extension_bench_public_api -- --nocapture`
  （临时测量文件，不入项目测试；bench 使用公开 API 形态仅作计量载体）。

## 数据

| 指标 | release（3 次采样） | dev（1 次） |
|---|---|---|
| 冷装载 prepare（含候选求值） | 110 / 118 / 249 μs | 947 μs |
| 冷装载 activate | 33 / 35 / 75 μs | 274 μs |
| 1000 次类型化命令（字符串转换） | 7-8 ms（≈7-8 μs/次） | 44 ms |
| 10 万次尾循环 | 215-218 ms（≈2.2 μs/迭代） | 1010 ms |
| 循环分配 ×2000 后实例 | 命令继续可用（回收不阻断） | 同左 |

环境：i7 级桌面 CPU；解释器为显式栈机器树遍历（无 JIT），每步含燃料、
取消与墙钟检查。dev 约慢 5 倍属预期。

## 归因与含义

- 端到端命令延迟由跨边界值构造（登记分配）与机器步进共同构成；
  7-8 μs/次满足 P0 合同「命令端到端」基线用途，未设定生产 SLO。
- 10 万次尾循环不触发 8192 帧深上限：proper tail calls 语义与资源
  边界同时成立（公开测试 `r7rs_semantics_samples` 行为面覆盖）。
- 循环分配负载下标记-清扫在安全点运行，实例保持可用；存活字节
  配额（默认 4 MiB）为硬拒绝。
- 本记录不构成优化承诺；后续解释器性能工作须以本基线做 A/B 对照。

## SMC 与停止边界

- 测量只覆盖 `extensions` Module 公开门面与引擎私有实现的组合行为；
  不涉及 ui / graphics System。
- 停止边界：解释器语义（R7RS 受限宿主环境）与资源边界的正确性由
  `tests/extensions_public_api.rs` 持有；本记录只登记量级。
