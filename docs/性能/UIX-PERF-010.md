# UIX-PERF-010 根布局失效覆盖快路

## 结论

稳定 keyed 协调在上一阶段移除空焦点 sidecar 哈希后，`WidgetTree::propagate_layout_invalidation` 成为新的可隔离热点。布局队列已经含当前真实根节点时，后续任意子节点 Layout 请求都不会扩大布局遍历范围；旧路径仍把子节点加入去重表并再次扫描到根。本轮让 `InvalidationQueue` 直接报告根覆盖事实：覆盖成立时只推进 revision，`ViewAdapter` 不再插入子节点或传播祖先。

在 512 个稳定 keyed 同级节点、每轮 24 次完整协调的 release 场景中，五组交替配对 A/B 的总体中位耗时从 401,178.58 ns/次降至 370,647.17 ns/次，改善约 7.61%，五组候选均快于对应基线。每次协调仍为 5 次申请、4,680 B 累计申请、4,480 B 峰值存活和 0 次 key-width 申请。

候选性能契约最终只保留 1 个 Layout 根。基线算法会在已存在根节点之外继续插入 512 个唯一子节点，因此同一稳定场景会保留 513 个逻辑 Layout 身份，直到布局消费或队列清理；新路径同时减少队列条目和去重表驻留量。

## 边界与契约

该路径属于 UI System 的协调 Module。`InvalidationQueue` Component 唯一拥有失效条目、Layout 去重集合和 revision；`WidgetTree` 持有当前根身份并选择批次或共享队列；`ViewAdapter` 只消费“是否仍需传播”的窄结果，不取得队列内部状态。

实现保持以下契约：

- 只有队列已含当前完整 `root_id` 时才判定整树覆盖，旧树作用域或旧 generation 不会误命中；
- 根覆盖时不新增子节点条目，但 revision 仍推进一次，避免 `clear_if_revision` 清除并发到达的请求；
- 根尚未进入队列时，当前节点仍按原规则去重并继续传播祖先；
- 当前节点已经存在但根尚未存在时仍返回“需要传播”，不会因局部重复漏掉更高祖先；
- 失效批次继续写入树私有 `pending_invalidations`，只在最外层结束时发布，不改变嵌套批次时机；
- Paint、Composite、FullComposite、布局顺序、失败恢复和根生命周期均未改变。

## 基线与配对方法

基线与候选使用不同 `CARGO_TARGET_DIR`，但构建参数完全一致：

```bash
CARGO_PROFILE_RELEASE_STRIP=none \
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
RUSTFLAGS='-C force-frame-pointers=yes' \
rtk cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness --no-run
```

两个独立二进制按 `B,A,A,B,B,A,A,B,B,A` 顺序交替运行。每次命令均只命中一项：

```bash
<binary> --exact stable_keyed_reconcile_profile --nocapture
```

## 候选结果

| 配对 | 基线 B | 候选 A | A 相对 B |
|---|---:|---:|---:|
| 1 | 410,235.12 ns | 370,647.17 ns | -9.65% |
| 2 | 395,105.08 ns | 373,110.96 ns | -5.57% |
| 3 | 401,178.58 ns | 370,147.08 ns | -7.74% |
| 4 | 408,286.92 ns | 370,329.04 ns | -9.30% |
| 5 | 391,389.83 ns | 371,592.29 ns | -5.06% |
| 总体中位 | 401,178.58 ns | 370,647.17 ns | -7.61% |

基线和候选分别采集到 514 与 430 个 CPU 样本，均无丢失样本。逐栈检查中，`propagate_layout_invalidation` 从基线 28 次降至候选 0 次，其中同时包含 `RandomState::hash_one` 的传播栈从 4 次降至 0 次。热点调用链的完整消失与五组一致改善共同支持收益归因。

## 否决的候选

本轮先实测了移除 `layout_ancestor_scratch`、直接向批次或共享队列传播的结构性候选。它虽然删除每棵树的一个 `Vec` 字段，但五组总体中位慢 1.07%，热段申请指标也没有改善，已精确撤回。最终方案保留该暂存，只消除已经被根失效覆盖的整段冗余工作。

## 验证

```bash
rtk cargo test --release --lib \
  draw::renderer::invalidation::tests::layout_until_root_suppresses_child_when_root_is_queued \
  -- --exact
rtk cargo test --release --lib \
  draw::renderer::invalidation::tests::layout_until_root_retries_child_when_root_is_missing \
  -- --exact
rtk cargo test --release --lib \
  ui::widget_runtime::widget::tree_dirty::tests::non_batch_root_layout_suppresses_child_request_and_advances_revision \
  -- --exact
rtk cargo test --release --lib \
  ui::widget_runtime::widget::tree_dirty::tests::batch_root_layout_keeps_child_pending_until_finish \
  -- --exact
rtk proxy cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness -- --exact stable_keyed_reconcile_profile --nocapture
rtk proxy rustfmt --edition 2024 --check \
  src/draw/renderer/invalidation.rs \
  src/ui/coordination/adapter/mod.rs \
  src/ui/widget_runtime/widget/tree_dirty.rs \
  tests/keyed_reconcile_allocation_contract.rs \
  tests/unit/draw/renderer/invalidation__tests.rs \
  tests/unit/ui/widget_runtime/widget/tree_dirty__tests.rs
rtk git diff --check
```

四条队列、树封装和批次契约均精确命中 1 项并通过；性能契约命中 1 项并报告 `retained_layout_roots=1`。额外启用 `test-harness` 编译整套 library tests 时仍会被既有 Modal builder 测试对已移除 `TestApp::tree()` 的两处引用阻断，与本轮文件和默认 library 精确测试无关。

该热点是队列覆盖关系导致的冗余哈希与祖先遍历，不是指令级瓶颈；没有证据支持内联汇编，本轮不引入汇编。
