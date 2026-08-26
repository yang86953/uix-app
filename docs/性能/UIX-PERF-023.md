# UIX-PERF-023 焦点空注册表删除短路

## 结论

本轮在 `FocusManager` 内消除空 `focusable_widgets` 上必然无效的哈希删除。稳定协调会为
每个节点重新同步 tab index，结构替换还会注销旧节点；非聚焦组件占多数时，原实现仍会
为每次 `remove` 计算完整 `WidgetId` SipHash，即使注册表从未持有任何条目。

候选不改变分配：unkeyed stable 仍为 `120 alloc / 112,320 B / peak 4,480 B`，
structural 仍为 `1,902 / 1,380,672 B / 42,144 B`。四场景五对隔离 release 运行中，
unkeyed stable、unkeyed structural、keyed Empty、keyed Button 的配对几何均值分别改善
`1.571%`、`3.076%`、`0.894%`、`4.113%`，均未回退。

## 剖析与候选选择

PERF-022 候选的混合 perf 同时包含稳定和结构轮次，不能区分 SipHash、`retain_mut` 与树
访问属于哪一场景。本轮先以同一二进制分别采集：

```text
target:     /tmp/uix-perf023-split-base.o314Hd
stable:     309111.33 ns；120 alloc；112320 B；peak 4480 B；114 samples / 0 lost
structural: 340481.04 ns；1902 alloc；1380672 B；peak 42144 B；114 samples / 0 lost
```

stable top self 包括 `reconcile_existing 23.62%`、SipHash `write 7.17%`、
`get_raw 4.94%`、`get_mut_raw 3.44%`；structural 包括 SipHash `write 10.21%`、
`reconcile_existing 7.62%`、`Vec<WidgetId>::retain_mut 5.23%`、`get_raw 4.75%`。

stable 父栈把 SipHash 分成两类：布局失效集合的 `contains`，以及
`HashMap<WidgetId, i32>::remove`。后者对应 `FocusManager::register_focusable`；structural
还会在 `unregister_widget` 删除同一空表。布局失效去重跨越传播与帧消费边界，本轮不动；
焦点注册表则由单一 Component 独占，空表删除可局部证明无效，因此选为 PERF-023。

曾审计以确定性专用 hasher 替换 AppState SipHash 的方向，但予以否决：64 位平台的
`WidgetId` 至少包含 160 位可预测输入，且 AppState 可跨 tree scope；弱或无随机种子的
hasher 会保留可构造碰撞和 UI 线程退化风险。当前证据也没有把 AppState 指认为首要父栈。

## 实现与 SMC 边界

`FocusManager` Component 唯一拥有 `focusable_widgets: HashMap<WidgetId, i32>`。候选只在
两个所有者方法内增加空表门禁：

1. 非正 tab index 且表为空时，`register_focusable` 不再调用 `remove`；
2. 注销节点且表为空时，`unregister_widget` 不再调用 `remove`；
3. `focused_widget` 的身份比较与清理仍独立执行，不受表是否为空影响。

正 tab index 的插入、已登记节点转为非正值的真实删除、焦点注销和排序输出完全沿用旧
路径。没有新增缓存、字段、分配或跨 Module 所有权，WidgetTree 生命周期也未改变。

## 基线与候选

```text
A unkeyed: /tmp/uix-perf022-direct-unkeyed-cand.kQizw4/release/deps/unkeyed_reconcile_allocation_contract-a86d430783ff922e
A Build ID: 0fda66eec73235aceb25f60c8e37ccddb4a73a4b
A keyed:   /tmp/uix-perf022-direct-unkeyed-cand.kQizw4/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
A Build ID: 3121d885dc201c4fa3b49248538ed26067dfa508

B target:  /tmp/uix-perf023-focus-empty-cand.vCa0Yi
B unkeyed: /tmp/uix-perf023-focus-empty-cand.vCa0Yi/release/deps/unkeyed_reconcile_allocation_contract-eaee7fa24b9ccf06
B Build ID: c3561ff6dafa34929c54935494a118bef7f6d26b
B mtime:   2026-08-27 06:44:18 +0800
B keyed:   /tmp/uix-perf023-focus-empty-cand.vCa0Yi/release/deps/keyed_reconcile_allocation_contract-89c6d97b970d9da1
B Build ID: 616358b5fad06b7502b94069a4e3f79b9b6408df
B mtime:   2026-08-27 06:44:18 +0800
```

四场景均按 `A,B,B,A,A,B,B,A,A,B` 交替运行。每个进程以 exact 方式命中一项；
unkeyed profile 同时报告 stable 与 structural，keyed profile 同时报告 Empty 与 Button。

## 五对耗时

| 对 | Unkeyed stable A → B（ns/次） | 变化 | Unkeyed structural A → B（ns/次） | 变化 |
|---|---:|---:|---:|---:|
| 1 | 299222.79 → 297123.83 | -0.701% | 328457.62 → 319980.08 | -2.581% |
| 2 | 312017.21 → 289298.79 | -7.281% | 330880.50 → 317737.08 | -3.972% |
| 3 | 300384.96 → 289859.83 | -3.504% | 332438.42 → 316472.21 | -4.803% |
| 4 | 302056.46 → 312021.38 | +3.299% | 326918.08 → 316493.96 | -3.189% |
| 5 | 301656.12 → 303676.12 | +0.670% | 331181.46 → 328577.00 | -0.786% |

Stable 中位数为 `301656.12 → 297123.83 ns`，B 胜 `3/5`，配对几何均值改善
`1.571%`。Structural 中位数为 `330880.50 → 317737.08 ns`，B 胜 `5/5`，
配对几何均值改善 `3.076%`。

| 对 | Keyed Empty A → B（ns/次） | 变化 | Keyed Button A → B（ns/次） | 变化 |
|---|---:|---:|---:|---:|
| 1 | 334626.29 → 342038.08 | +2.215% | 427112.21 → 400765.29 | -6.169% |
| 2 | 334564.50 → 324647.12 | -2.964% | 423487.08 → 395333.88 | -6.648% |
| 3 | 344332.00 → 323094.25 | -6.168% | 406125.33 → 402687.67 | -0.846% |
| 4 | 329611.46 → 339049.17 | +2.863% | 407256.25 → 403337.67 | -0.962% |
| 5 | 342561.17 → 342112.83 | -0.131% | 421424.04 → 397134.33 | -5.764% |

Empty 中位数为 `334626.29 → 339049.17 ns`，B 胜 `3/5`；两次噪声回退使独立中位数
略高，但预先采用的配对几何门槛仍改善 `0.894%`。Button 中位数为
`421424.04 → 400765.29 ns`，B 胜 `5/5`，配对几何均值改善 `4.113%`。

## 资源与候选 perf

| 场景 | allocations | allocated bytes | peak | final / retained |
|---|---:|---:|---:|---:|
| Unkeyed stable | 120 | 112,320 B | 4,480 B | final 0 |
| Unkeyed structural | 1,902 | 1,380,672 B | 42,144 B | final 1,024 B |
| Keyed Empty | 5/次 | 4,680 B/次 | 4,480 B | roots 1 |
| Keyed Button | 1,029/次 | 35,400 B/次 | 4,480 B | roots 1 |

A 与 B 的四组资源及 key-width 统计完全一致；两组 keyed 的 key-width 申请均为 0。

候选 perf 位于 `/tmp/uix-perf023-focus-empty-cand.vCa0Yi/perf.data`，为
`190 samples / 0 lost`。top self 包括 `reconcile_existing 14.08%`、`get_raw 6.15%`、
`ReconcileProbe::type_id 3.13%`、`Vec<WidgetId>::retain_mut 3.12%`、
`fill_visual_path 2.97%`、`RawVecInner::deallocate 2.67%`。候选中未再采到
`HashMap<WidgetId, i32>::remove/remove_entry`；低样本量与内联使 SipHash 百分比不适合
直接作因果比较，最终裁决以四场景配对耗时和资源契约为准。

## 验证

- `empty_registry_removal_preserves_focus_and_registration_semantics`：`1 passed`；覆盖空表
  非正注册、正值排序、真实删除、焦点清理和重复注销。
- unkeyed 与 keyed release exact profile：均命中 `1 passed`，四场景资源不变。
- `rustfmt --edition 2024 src/ui/widget_runtime/managers/focus_manager.rs`：通过。
- `rtk git diff --check`：通过。
- 精确 lib 测试只出现既有 `138` 条 warning，未发现本轮新增 warning 或错误。

该热点是哈希控制流而非可向量化计算循环；Rust 层空表门禁已经直接消除目标工作，没有
内联汇编依据。
