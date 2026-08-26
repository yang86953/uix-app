# UIX-PERF-026 RenderHandler 空 sidecar 查询短路

## 结论

本轮在 `RenderHandlerTable` 自有查询边界为四张 renderer sidecar 增加空表快返。普通节点
协调仍会探测 Select、VirtualScroll 及启用 capability 后的 Table renderer；旧实现即使
对应 `HashMap` 全局为空，也会为每次 `contains_key` 计算完整 `WidgetId` SipHash。

候选不改变分配。四场景五对隔离 release 运行中，unkeyed stable、unkeyed structural、
keyed Empty、keyed Button 的配对几何均值分别改善 `2.667%`、`3.778%`、`1.310%`、
`1.527%`，均未回退，通过预设门槛。

## 剖析与候选选择

PERF-025 候选 perf 为 `581 samples / 0 lost`，top self 包括 SipHash `write 6.87%`、
`HashMap::hash_one 3.34%` 与 `refresh_virtual_scroll_widget 1.89%`。协调热路对每个普通节点
调用 `has_select_option_renderer`、`has_virtual_scroll_renderer`，并在末尾刷新虚拟滚动
子树；这些查询最终进入 `RenderHandlerTable::contains_*`。

`RenderHandlerTable` 已在删除路径避免对空表调用 `remove`，但查询路径仍无条件执行
`contains_key`。空表不可能持有任何 `WidgetId`，因此本轮选择在四个对应查询中先检查
`is_empty()`。没有把 `replace_widget` 的空输入纳入候选：全局其他表非空或当前节点曾持有
renderer 时，直接跳过替换会遗留旧 sidecar；现有证据不足以扩展该范围。

## 实现与 SMC 边界

`RenderHandlerTable` Component 由单个 `WidgetTree` 独占，唯一拥有四张动态 renderer
sidecar。候选只修改它自己的只读查询：

1. 对应表为空时直接返回 `false`；
2. 表非空时仍以原有 `contains_key` 校验完整 `WidgetId`；
3. stale、已移除或碰撞身份仍由 `HashMap` 原有规则处理；
4. 注册、替换、移除、shutdown 和用户闭包生命周期完全不变。

调用方始终把 `false` 解释为当前节点没有对应 renderer，因此空表快返与原返回值严格
等价。该修改没有新增字段、缓存、分配、跨 Module 依赖或生命周期状态。

## 基线与候选

```text
A target:  /tmp/uix-perf025-provider-ptr-cand.NwRSeZ
A unkeyed: /tmp/uix-perf025-provider-ptr-cand.NwRSeZ/release/deps/unkeyed_reconcile_allocation_contract-a86d430783ff922e
A Build ID: ea00c6eb9cec0baad4a36d2c75da7964f0228401
A keyed:   /tmp/uix-perf025-provider-ptr-cand.NwRSeZ/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
A Build ID: 0e46bf3b105ffe2c7698b079939472df9e1b7aba

B target:  /tmp/uix-perf026-render-empty-query-cand.CdY4JY
B unkeyed: /tmp/uix-perf026-render-empty-query-cand.CdY4JY/release/deps/unkeyed_reconcile_allocation_contract-0c8dd90538cdd64a
B Build ID: a0bbee25f64c3c6de2fafe52f61f47493635bf31
B mtime:   2026-08-27 07:43:59 +0800
B keyed:   /tmp/uix-perf026-render-empty-query-cand.CdY4JY/release/deps/keyed_reconcile_allocation_contract-7075994198236f54
B Build ID: 0d04eb05c8d26c05d51cde2fe28907461c2302be
B mtime:   2026-08-27 07:44:00 +0800
```

四场景均按 `A,B,B,A,A,B,B,A,A,B` 交替运行。每个进程以 exact 方式命中一项；
unkeyed profile 同时报告 stable 与 structural，keyed profile 同时报告 Empty 与 Button。

## 五对耗时

| 对 | Unkeyed stable A → B（ns/次） | 变化 | Unkeyed structural A → B（ns/次） | 变化 |
|---|---:|---:|---:|---:|
| 1 | 213001.96 → 207679.92 | -2.499% | 261067.08 → 251388.50 | -3.707% |
| 2 | 210250.96 → 206695.12 | -1.691% | 259969.62 → 250191.67 | -3.761% |
| 3 | 216483.08 → 208503.92 | -3.686% | 261043.71 → 251482.83 | -3.663% |
| 4 | 210594.54 → 209403.12 | -0.566% | 259816.00 → 250324.00 | -3.653% |
| 5 | 219080.00 → 208485.12 | -4.836% | 261602.71 → 250864.21 | -4.105% |

Stable 中位数为 `213001.96 → 208485.12 ns`，改善 `2.121%`，B 胜 `5/5`；配对
几何均值改善 `2.667%`。Structural 中位数为 `261043.71 → 250864.21 ns`，改善
`3.900%`，B 胜 `5/5`；配对几何均值改善 `3.778%`。

| 对 | Keyed Empty A → B（ns/次） | 变化 | Keyed Button A → B（ns/次） | 变化 |
|---|---:|---:|---:|---:|
| 1 | 249013.62 → 249862.75 | +0.341% | 323344.71 → 323173.17 | -0.053% |
| 2 | 240315.67 → 239880.25 | -0.181% | 315578.08 → 316541.50 | +0.305% |
| 3 | 242994.88 → 242435.46 | -0.230% | 339353.46 → 319321.75 | -5.903% |
| 4 | 242688.04 → 239492.46 | -1.317% | 316946.04 → 314637.12 | -0.728% |
| 5 | 253923.71 → 241070.42 | -5.062% | 317044.96 → 313489.12 | -1.122% |

Empty 中位数为 `242994.88 → 241070.42 ns`，改善 `0.792%`，B 胜 `4/5`；配对
几何均值改善 `1.310%`。Button 中位数为 `317044.96 → 316541.50 ns`，改善 `0.159%`，
B 胜 `4/5`；配对几何均值改善 `1.527%`。

## 资源与候选 perf

| 场景 | allocations | allocated bytes | peak | final / retained |
|---|---:|---:|---:|---:|
| Unkeyed stable | 120 | 112,320 B | 4,480 B | final 0 |
| Unkeyed structural | 1,902 | 1,380,672 B | 42,144 B | final 1,024 B |
| Keyed Empty | 5/次 | 4,680 B/次 | 4,480 B | roots 1 |
| Keyed Button | 1,029/次 | 35,400 B/次 | 4,480 B | roots 1 |

A 与 B 的四组资源完全一致；两组 keyed 的 key-width 申请均为 0。

候选 perf 位于 `/tmp/uix-perf026-render-empty-query-cand.CdY4JY/perf.data`，为
`583 samples / 0 lost`。top self 包括 `reconcile_existing 14.19%`、`get_raw 6.53%`、
SipHash `write 4.85%`、`HashMap::hash_one 1.77%`、`refresh_virtual_scroll_widget 1.19%`。
SipHash 与 `hash_one` 相对 PERF-025 的采样占比方向性下降，但两次运行和二进制不同，
不能只凭百分比作严格因果判断；最终裁决以配对耗时和资源契约为准。

## 验证

- `empty_render_handler_sidecars_report_every_renderer_absent`：`1 passed`；覆盖默认空 sidecar
  的全部查询。
- `virtual_scroll_shutdown_releases_renderer_captures`：`1 passed`；确认虚拟滚动 renderer
  生命周期与 shutdown 释放不变。
- `virtualized_true_materializes_viewport_only`：`1 passed`；确认 Table renderer 的真实存在
  查询与物化不变。
- `custom_option_layout_uses_current_tree_surface`：`1 passed`；确认 Select 自定义选项仍通过
  当前树 surface 工作。
- 20 个正式交错 release exact 进程全部 `1 passed`；四场景资源不变。
- `rustfmt --edition 2024 src/ui/coordination/render_handler.rs` 与 `git diff --check`：通过。
- 候选 release 构建报告既有 `163` 条 warning，与基线一致，无新增 warning 或错误。

该热点是空哈希表查询控制流，不是可向量化数值循环。Rust 层空表门禁已直接消除目标
工作，当前没有支持内联汇编的剖析或基准证据。
