# UIX-PERF-025 ProviderContext 共享快照身份快返

## 结论

本轮为 `ProviderContext` 的相等比较增加同一 `Arc` 快照身份快返。默认节点及未改变
Provider 的稳定协调共享同一不可变快照；旧实现仍会逐层比较 `WidgetConfig`、`Locale`、
组件令牌哈希表与字符串。候选先用 `Arc::ptr_eq` 判定同一快照，再只对不同快照保留原有
深比较。

候选不改变分配。四场景五对隔离 release 运行中，unkeyed stable、unkeyed structural、
keyed Empty、keyed Button 的配对几何均值分别改善 `27.22%`、`17.18%`、`24.91%`、
`22.44%`，四组均为 B 胜 `5/5`，通过预设的四场景均不得回退门槛。

## 剖析与候选选择

PERF-023 的 stable 调用图把 `ProviderContext::ne/eq` 归因到 `reconcile_existing` 热路，
其子调用继续进入 `ProviderContextValues`、`WidgetConfig`、`Locale`、字符串和
`HashMap<TypeId, Arc<TokenPatch>>` 比较。该调用链约占当次 stable 样本的 `8.27%`。

标准库 `Arc<T>` 的通用 `PartialEq` 实现对 `T: PartialEq` 直接执行深比较；只有 `T: Eq`
的特化才先比较指针。`ProviderContextValues` 包含可为 NaN 的 `TokenPatch` 浮点字段，不能
声明 `Eq`，所以派生的 `ProviderContext::PartialEq` 不会自动获得身份快返。

本轮选择在拥有快照语义的 `ProviderContext` 边界显式快返，未把内部值错误提升为 `Eq`，
也未改变 `WidgetConfig` 或 `TokenPatch` 对不同值对象的比较规则。

## 实现与 SMC 边界

`ProviderContext` Component 唯一拥有 `Arc<ProviderContextValues>` 快照，配置和语言
Provider 通过 `Arc::make_mut` 生成实际覆写后的新快照。候选相等契约为：

1. 两个上下文指向同一不可变快照时立即相等；
2. 指向不同快照时继续比较完整配置与语言值；
3. 实际 Provider 覆写因写时复制而获得新身份，不会被快返吞掉；
4. 不同快照中的 NaN 仍按原有 `PartialEq` 语义判为不等。

该修改没有新增字段、缓存、分配、生命周期或跨 Module 依赖。`reconcile_existing` 仍只
依据 `context_changed` 决定上下文失效，WidgetTree 的所有权与协调顺序不变。

## 基线与候选

```text
A target:  /tmp/uix-perf023-focus-empty-cand.vCa0Yi
A unkeyed: /tmp/uix-perf023-focus-empty-cand.vCa0Yi/release/deps/unkeyed_reconcile_allocation_contract-eaee7fa24b9ccf06
A Build ID: c3561ff6dafa34929c54935494a118bef7f6d26b
A keyed:   /tmp/uix-perf023-focus-empty-cand.vCa0Yi/release/deps/keyed_reconcile_allocation_contract-89c6d97b970d9da1
A Build ID: 616358b5fad06b7502b94069a4e3f79b9b6408df

B target:  /tmp/uix-perf025-provider-ptr-cand.NwRSeZ
B unkeyed: /tmp/uix-perf025-provider-ptr-cand.NwRSeZ/release/deps/unkeyed_reconcile_allocation_contract-a86d430783ff922e
B Build ID: ea00c6eb9cec0baad4a36d2c75da7964f0228401
B mtime:   2026-08-27 07:22:50 +0800
B keyed:   /tmp/uix-perf025-provider-ptr-cand.NwRSeZ/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
B Build ID: 0e46bf3b105ffe2c7698b079939472df9e1b7aba
B mtime:   2026-08-27 07:22:51 +0800
```

四场景均按 `A,B,B,A,A,B,B,A,A,B` 交替运行。每个进程以 exact 方式命中一项；
unkeyed profile 同时报告 stable 与 structural，keyed profile 同时报告 Empty 与 Button。

## 五对耗时

| 对 | Unkeyed stable A → B（ns/次） | 变化 | Unkeyed structural A → B（ns/次） | 变化 |
|---|---:|---:|---:|---:|
| 1 | 290772.38 → 221793.46 | -23.72% | 317967.96 → 259964.58 | -18.24% |
| 2 | 289742.96 → 209019.88 | -27.86% | 318731.46 → 260990.29 | -18.12% |
| 3 | 291764.67 → 209876.08 | -28.07% | 318542.75 → 269829.33 | -15.29% |
| 4 | 287378.92 → 209674.04 | -27.04% | 315941.67 → 259746.71 | -17.79% |
| 5 | 296220.08 → 209474.50 | -29.28% | 315024.08 → 263226.17 | -16.44% |

Stable 中位数为 `290772.38 → 209674.04 ns`，改善 `27.89%`；配对几何均值改善
`27.22%`。Structural 中位数为 `317967.96 → 260990.29 ns`，改善 `17.92%`；
配对几何均值改善 `17.18%`。两组均为 B 胜 `5/5`。

| 对 | Keyed Empty A → B（ns/次） | 变化 | Keyed Button A → B（ns/次） | 变化 |
|---|---:|---:|---:|---:|
| 1 | 328726.04 → 244374.92 | -25.66% | 425907.88 → 317344.29 | -25.49% |
| 2 | 329903.67 → 257295.42 | -22.01% | 423044.17 → 324111.17 | -23.39% |
| 3 | 328886.79 → 246638.38 | -25.01% | 406809.12 → 317656.96 | -21.91% |
| 4 | 328717.25 → 244163.33 | -25.72% | 410233.50 → 338088.17 | -17.59% |
| 5 | 329211.12 → 243340.92 | -26.08% | 411867.79 → 314608.71 | -23.61% |

Empty 中位数为 `328886.79 → 244374.92 ns`，改善 `25.70%`；配对几何均值改善
`24.91%`。Button 中位数为 `411867.79 → 317656.96 ns`，改善 `22.87%`；
配对几何均值改善 `22.44%`。两组均为 B 胜 `5/5`。

## 资源与候选 perf

| 场景 | allocations | allocated bytes | peak | final / retained |
|---|---:|---:|---:|---:|
| Unkeyed stable | 120 | 112,320 B | 4,480 B | final 0 |
| Unkeyed structural | 1,902 | 1,380,672 B | 42,144 B | final 1,024 B |
| Keyed Empty | 5/次 | 4,680 B/次 | 4,480 B | roots 1 |
| Keyed Button | 1,029/次 | 35,400 B/次 | 4,480 B | roots 1 |

A 与 B 的四组资源完全一致；两组 keyed 的 key-width 申请均为 0。

候选 perf 位于 `/tmp/uix-perf025-provider-ptr-cand.NwRSeZ/perf.data`，为
`581 samples / 0 lost`。top self 包括 `reconcile_existing 12.24%`、SipHash `write 6.87%`、
`WidgetTree::get_raw 5.85%`、`ViewNode::leaf 3.87%`、`HashMap::hash_one 3.34%`、
`RawVecInner::deallocate 3.12%`。未以独立符号采到 `ProviderContext::eq` 或深层
`WidgetConfig` 比较；它们可能已经内联，有限样本不能证明比较成本为零，最终裁决仍以
四场景配对耗时和资源契约为准。

## 验证

- `default_context_reuses_one_immutable_snapshot`：`1 passed`；确认默认快照共享身份与结构
  尺寸不变。
- `shared_non_reflexive_snapshot_uses_identity`：`1 passed`；确认同一含 NaN 快照按身份相等，
  不同含 NaN 快照仍不等。
- `independent_equal_snapshots_keep_value_semantics`：`1 passed`；确认不同身份的普通等值快照
  仍能深比较为相等。
- `nested_overrides_restore_outer_snapshot` 与 `panic_restores_previous_snapshot`：各
  `1 passed`；确认嵌套及 panic 展开后的快照生命周期。
- 20 个正式交错 release exact 进程全部 `1 passed`；四场景资源不变。
- `rustfmt --edition 2024 src/ui/widget_runtime/provider_context.rs` 与 `git diff --check`：通过。
- release 构建报告既有 `163` 条 warning，未观察到本轮新增 warning 或错误。

该热点是不可变共享对象的比较控制流，不是可向量化数值循环。Rust 层身份快返已直接
消除目标深比较，当前没有支持内联汇编的剖析或基准证据。
