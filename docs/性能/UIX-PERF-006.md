# UIX-PERF-006 自定义组件快照分派快路

## 结论

稳定 keyed 协调的首个可隔离 CPU 热点是未知自定义组件的快照分派：默认 `Widget::snapshot_fields` 会顺序尝试全部内置组件类型，最后才返回 `SnapshotFields::Unknown`。本轮让未登记组件直接返回 `Unknown`，并让已有类型化快照的内置组件显式调用自身 `SnapshotSource`。

在 512 个稳定 keyed 同级节点、每轮 24 次完整协调的 release 场景中，中位耗时从 685,367 ns/次降至 493,440 ns/次，改善 28.0%。每次协调仍为 5 次申请、4,680 B 累计申请和 4,480 B 峰值存活；本轮没有用额外内存换取速度。

## 边界与契约

该路径属于 UI System：协调 Module 的 `ViewAdapter::patch_widget` 通过 Widget 公开快照契约比较当前与下一声明，`widget_snapshot` Module 拥有类型化字段分派。自定义 Widget 未登记内置字段时，优化前后的公开结果都为 `SnapshotFields::Unknown`。

实现只改变分派方式：

- `Widget` 默认实现直接返回 `Unknown`，不再扫描内置类型表；
- `widget!` 生成的 Label、Input、Container、Grid 直接调用各自 `SnapshotSource`；
- `impl_widget!` 默认保持未知快照，并允许已有 `SnapshotSource` 的组件显式接线；当前 Button 使用该端口；
- 其他 `widget!` 内置组件继续使用原有类型化分派，未改变快照字段、失效语义、实例所有权或生命周期。

## 基线与剖析

标准 release 基线命令：

```bash
rtk proxy cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness -- --exact stable_keyed_reconcile_profile --nocapture
```

基线九轮中位数：

```text
median_ns_per_reconcile=685366.92
allocations_per_reconcile=5.00
allocated_bytes_per_reconcile=4680.00
peak_live_bytes=4480
key_width_allocations_per_reconcile=0.00
```

为函数级 CPU 样本构建保留行表和帧指针的临时二进制，并用 `perf` 采样：

```bash
CARGO_TARGET_DIR=/tmp/uix-keyed-prof-target \
CARGO_PROFILE_RELEASE_STRIP=none \
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
RUSTFLAGS='-C force-frame-pointers=yes' \
rtk cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness --no-run

rtk proxy perf record -F 4000 -g --call-graph fp \
  -o /tmp/uix-keyed-prof-target/perf.data \
  /tmp/uix-keyed-prof-target/release/deps/keyed_reconcile_allocation_contract-<hash> \
  --exact stable_keyed_reconcile_profile
```

基线的 677 个 CPU 样本中，`snapshot_fields_from_any` 为第一热点，占 18.61%；`reconcile_existing` 占 11.20%，SipHash 写入占 5.00%。调用链确认热点来自 `ReconcileProbe::snapshot_fields` 在 `patch_widget` 中对当前和下一组件重复执行内置类型扫描。

## 候选结果

候选后的两个独立进程中位数分别为 496,862 ns/次和 493,440 ns/次，资源指标完全一致。相对 685,367 ns/次的基线，改善范围为 27.5% 至 28.0%。

候选后二次 `perf` 获得 546 个 CPU 样本，`snapshot_fields_from_any` 已退出占比 1% 以上的热点列表；新的第一热点为 `reconcile_existing`（11.51%）。这一变化与源码边界、总测试时长从约 0.17 s 降至 0.13 s 共同支持因果归因。

本阶段只处理一个热点，不继续微调 `reconcile_existing`、SipHash 或节点查询。后续轮次必须重新建立基线和剖析证据。

## 验证

```bash
rtk proxy rustfmt --edition 2024 --check \
  src/ui/widget_runtime/traits.rs \
  src/ui/macros/widget.rs \
  src/ui/macros/mod.rs \
  src/ui/widgets/general/button/mod.rs \
  tests/widget_snapshot_dispatch_contract.rs
rtk cargo test --release --test widget_snapshot_dispatch_contract
rtk proxy cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness -- --exact stable_keyed_reconcile_profile --nocapture
rtk git diff --check
```

格式检查通过；快照分派合同实际命中并通过 2 项；keyed profile 实际命中并通过 1 项；差异检查通过。构建只保留既有警告，没有新增警告。
