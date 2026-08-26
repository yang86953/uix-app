# UIX-PERF-007 空处理器协调快路

## 结论

稳定 keyed 协调的下一个可隔离 CPU 热点是处理器签名分组。没有旧处理器且新声明仍为空时，旧路径仍会重复查询节点并构造两个临时分组表。本轮在一次稳定借用中读取旧签名，并让空到空声明直接返回未变更。

在 512 个稳定 keyed 同级节点、每轮 24 次完整协调的 release 场景中，三个独立进程的合并中位耗时从 464,994.92 ns/次降至 452,390.88 ns/次，改善约 2.71%。每次协调仍为 5 次申请、4,680 B 累计申请、4,480 B 峰值存活和 0 次 key-width 申请；本轮没有用额外内存换取速度。

## 边界与契约

该路径属于 UI System 内部协调 Module：`ViewAdapter` 拥有声明与运行时节点的协调次序，`WidgetTree` 拥有节点和处理器 sidecar 的实例生命周期。

实现仅收窄未变更判定：

- 旧签名和新处理器均为空时，不解析签名、不构造分组表，也不更新 sidecar；
- 节点存在的非空路径复用同一次不可变借用，仍执行签名稳定性与分组等价检查；
- 节点不存在时仍从新声明建立签名并视为变更；
- 变更路径仍由 `WidgetTree` 清理、替换并注册处理器，未改变实例所有权、更新顺序或卸载语义。

## 基线与剖析

标准 release 基准命令：

```bash
UIX_RECONCILE_PROFILE_LABEL=<label> \
rtk proxy cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness -- --exact stable_keyed_reconcile_profile --nocapture
```

当前提交在修改前用三个独立进程重建基线：

```text
465391.08 ns/reconcile
463621.96 ns/reconcile
464994.92 ns/reconcile
合并中位数 = 464994.92 ns/reconcile
范围 = 463621.96–465391.08 ns/reconcile
```

所有基线进程均为 5 次申请/协调、4,680 B 申请/协调、4,480 B 峰值存活和 0 次 key-width 申请。修改前的函数级采样中，`handler_signature_groups` 占 2.64%，与本轮可隔离范围一致。

## 候选结果

候选后三个独立进程均实际命中 1 项：

```text
452390.88 ns/reconcile
450350.38 ns/reconcile
459447.46 ns/reconcile
合并中位数 = 452390.88 ns/reconcile
范围 = 450350.38–459447.46 ns/reconcile
```

相对重建基线，合并中位数减少 12,604.04 ns/次，改善约 2.71%；全部资源指标与基线一致。

候选后保留行表和帧指针的 `perf` 采样命中 1 项并获得 595 个 CPU 样本；`handler_signature_groups` 已退出占比 0.8% 以上的平铺热点列表，`reconcile_existing` 从修改前的 11.51% 降至 9.71%。这与三进程 A/B 方向一致，支持收益归因。

本阶段只处理这一个热点，不继续微调哈希、节点查询或无效化传播。后续轮次必须重新建立基线和剖析证据。

## 验证

```bash
rtk proxy rustfmt --edition 2024 --check src/ui/coordination/adapter/mod.rs
rtk cargo test --test ui_public_api --features test-harness \
  ui_test_harness_drives_the_same_public_semantic_path -- --exact
rtk proxy cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness -- --exact stable_keyed_reconcile_profile --nocapture
rtk git diff --check
```

格式检查通过；公开处理器协调契约实际命中并通过 1 项；keyed profile 每个独立进程均实际命中并通过 1 项；差异检查通过，没有新增警告。

库内更宽的 FloatButton 生命周期 exact 测试在编译阶段被两处与本轮无关的既有 `TestApp::tree()` 缺失错误阻断，因此不计为本轮通过项；可实际编译和运行的公开契约已单独验收。
