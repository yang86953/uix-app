# UIX-PERF-012 稳定 keyed 预检单次身份查询

## 结论

PERF-011 后的新一轮指令采样中，`WidgetTree::get_raw` 占 3.90% self，实际热点指令集中在 tree scope、generation 和节点占用校验。稳定 keyed 同序预检原先先按 `child_id` 查询 key，随后 `can_reuse` 又按同一身份查询组件类型和内联组件作用域；同一个不可变判断因此重复执行完整身份验证。

本轮把“已经取得当前节点后的可复用判定”提取为窄 helper，让同一 `BoxedWidget` 借用同时完成 key、组件类型和 `uix_widget_scopes` 比较。父节点长度与子序列也合并到同一次父借用中。根协调仍通过 `WidgetTree::get` 验证身份；快路协调循环仍在每次写入前重新读取当前父子关系，没有缓存跨越可变协调边界的裸槽位或节点引用。

在 512 个稳定 keyed 同级节点、每轮 24 次完整协调的 release 场景中，两轮共十组交替配对有七组候选更快。合并中位从 374,935.00 ns/次降至 370,449.30 ns/次，改善约 1.20%；成对比值的几何均值改善约 1.26%。每次协调仍为 5 次申请、4,680 B 累计申请、4,480 B 峰值存活、0 次 key-width 申请和 1 个保留 Layout 根。

## 边界与契约

该路径属于 UI System 的协调 Module。运行时树仍唯一拥有节点槽位、generation、tree scope 与生命周期；协调 Module 只复用一次已经由树公开安全查询验证的不可变节点借用，不取得槽位表或身份验证实现。

实现保持以下契约：

- `can_reuse(tree, id, node)` 仍先调用 `WidgetTree::get`，根协调与通用调用方不绕过身份验证；
- 稳定 keyed 预检对每个 child 只查询一次，并在该借用上依次验证精确 key、组件具体类型和完整组件作用域序列；
- 父子数量、顺序、key 唯一性与精确碰撞回退条件不变；
- 预检借用只存在于不可变判断闭包内，不跨越 `cancel_pending_removal` 或递归协调；
- 协调循环继续在每个 child 写入前按父节点读取真实 `child_id`，保留生命周期回调和失败停止边界；
- 重复 key、类型变化、作用域变化、缺失节点和结构变化仍进入原通用协调路径。

## 基线与配对方法

基线产品代码固定为提交 `067c0d8660a8dac165d7c790e9d9827f7d9c1c13`；当前文档基线提交 `249dafefaa9514634b4bd99aeda4b96741495114` 只新增 PERF-011 文档，产品代码相同。候选使用独立 `CARGO_TARGET_DIR`：

```bash
CARGO_PROFILE_RELEASE_STRIP=none \
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
RUSTFLAGS='-C force-frame-pointers=yes' \
rtk cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness --no-run
```

每轮按 `B,A,A,B,B,A,A,B,B,A` 顺序交替运行两个独立二进制；首次五组出现小幅收益但 perf 样本方向冲突，因此按同一方法补充第二轮五组裁决，而不是用单次结果决定。

## 候选结果

| 配对 | 基线 B | 候选 A | A 相对 B |
|---|---:|---:|---:|
| 1 | 372,159.17 ns | 365,664.08 ns | -1.75% |
| 2 | 379,032.04 ns | 371,816.04 ns | -1.90% |
| 3 | 370,227.62 ns | 374,249.33 ns | +1.09% |
| 4 | 382,600.00 ns | 371,059.17 ns | -3.02% |
| 5 | 373,184.00 ns | 367,577.25 ns | -1.50% |
| 6 | 377,618.96 ns | 383,403.58 ns | +1.53% |
| 7 | 376,686.00 ns | 364,967.75 ns | -3.11% |
| 8 | 385,080.46 ns | 366,525.29 ns | -4.82% |
| 9 | 369,268.33 ns | 375,340.96 ns | +1.64% |
| 10 | 371,774.29 ns | 369,839.42 ns | -0.52% |
| 合并中位 | 374,935.00 ns | 370,449.30 ns | -1.20% |

## 指令证据与限制

基线采集 461 个 CPU 样本、丢失 0 个，`get_raw` 为 17 个 self annotate 样本、3.90%；候选采集 450 个样本、丢失 0 个，分别为 25 个和 5.76%。该采样规模下 self 样本没有随计时收益下降，因此不能把占比变化当作收益证明。

反汇编提供了更稳定的结构证据：完整 `reconcile_children` 中静态 `get_raw` 调用点从 14 个降至 13 个，目标预检片段从 3 个降至 2 个。最终保留依据是十组成对计时方向、约 1.20% 合并中位收益和已确认调用点消除的共同结果；收益仍属于小幅优化，后续基线重建时应继续观察。

## 验证

```bash
rtk cargo test --test keyed_reconcile_allocation_contract \
  --features test-harness -- --exact stable_keyed_reconcile_profile --nocapture
rtk proxy rustfmt --edition 2024 --check \
  src/ui/coordination/adapter/mod.rs \
  src/ui/coordination/adapter/coordination.rs
rtk git diff --check
```

精确测试命中 1 项并通过；同一测试同时覆盖摘要碰撞下的精确重排、状态与焦点保留、真实移除，以及稳定快路的身份顺序和资源契约。两个代码文件均低于 1,500 行。

该热点是重复的安全身份查询，不是可由手写算术或 SIMD 指令改善的内核，没有证据支持内联汇编。
