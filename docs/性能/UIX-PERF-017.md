# UIX-PERF-017 复用 incoming TypeId 减少 owner 动态分派

## 结论

本轮最终保留的候选只在 `ViewAdapter::reconcile_existing` 的 `apply_style` 之后
读取一次 incoming widget 的具体 `TypeId`，并用常量 `TypeId` 比较替代
Calendar、Anchor、Transfer、Carousel 和 Image 的重复 `as_any().is::<T>()`。
Calendar 仍保留 guarded downcast 以及 live runtime fallback；Anchor 的
`navigation` feature gate、Transfer/Carousel/Image 的后续协调顺序均不变。

在 512 个 keyed 节点、每轮 24 次协调的五对交替 release 运行中，Empty 与 Button
均为 4/5 对更快，且中位数和成对几何均值同向改善；所有分配、字节、峰值和保留根
指标完全不变。该证据支持保留这一窄候选，但不把不同采样轮次的 perf 占比当作独立
收益归因。

## 被否决的前置候选

此前的双空 handler 候选在 Empty 场景 4/5 更快，中位改善 3.200%，成对几何均值
改善 1.606%；Button 仅 1/5 更快，中位反而退化 0.927%，成对几何均值退化
0.866%。两场景资源均不变，因此该候选已否决并回退，其 candidate target 已删除；
本文件只记录它作为本轮筛选证据，不把它计入最终实现。

## 最终实现与语义

实现紧接 `let widget = Self::apply_style(...)` 增加一次：

```rust
// 复用前置已确认类型稳定，只读取一次 incoming 的具体 TypeId。
let widget_type_id = widget.as_any().type_id();
```

随后：

- Calendar 先比较 `TypeId::of::<Calendar>()`，命中后才执行原有
  `downcast_ref::<Calendar>()`，并保留
  `owns_custom_cell_children() || tree.is_calendar_cell_widget(id)` 的 live fallback；
- Anchor 只在 `navigation` 启用的原 cfg 分支中使用 `TypeId::of::<Anchor>()`，关闭
  navigation 时仍固定为 `false`；
- Transfer、Carousel 和 Image 分别直接比较其 `TypeId` 常量；
- `apply_style` 仍只更新样式，不替换具体 widget 类型，所有 owner 动态子树捕获、
  `sync_from`、patch 和后续操作顺序保持不变。

这等价于在 incoming 为 non-Calendar（例如 Button）时，把 owner 区原先五次动态
类型判定收敛为一次 `as_any`/`type_id`；Calendar 的专用 downcast 与 live 查询仍
按需执行，不能把整个函数宣称为只有一个 TypeId 指令。

## SMC 与所有权边界

改动位于 UI System 的 coordination Module，只保存当前调用栈中的 `TypeId` 值，
不缓存 `&dyn Any`、widget 引用或树槽位，也不跨函数、生命周期或线程传递 TypeId。
WidgetTree 仍唯一拥有 live widget 及其动态子树状态；Calendar、Anchor、Transfer、
Carousel、Image 的运行时 Component 契约没有改变。

该快路依赖公开 `Widget::as_any()` 契约稳定返回组件自身的 `Any` 视图。若外部自定义
Widget 违反这一约定，让 `as_any()` 具有副作用、返回非自身类型或依赖调用次数，则减少
调用次数可能改变其可观察行为；这属于不满足现有类型识别契约的实现限制，不是本候选
支持的语义路径。

## 基线、候选与方法

基线提交为
`20cd4e6e652ad78b5ea069736f2f72f44f4b2428`。两侧均为 release、
`--features test-harness` 的 keyed integration benchmark，使用 frame-pointer
call graph、未 strip 的独立 target，并按 `B,A,A,B,B,A,A,B,B,A` 交替运行；A 为
baseline，B 为 candidate。每次 exact 命中 `stable_keyed_reconcile_profile` 一项。

```text
baseline target: /tmp/uix-perf017-empty-handler-base.3hSAmr
baseline binary: /tmp/uix-perf017-empty-handler-base.3hSAmr/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
baseline perf:   /tmp/uix-perf017-empty-handler-base.3hSAmr/perf-typeid.data

candidate target: /tmp/uix-perf017-owner-typeid-cand.XOB7Iu
candidate binary: /tmp/uix-perf017-owner-typeid-cand.XOB7Iu/release/deps/keyed_reconcile_allocation_contract-7075994198236f54
candidate perf:   /tmp/uix-perf017-owner-typeid-cand.XOB7Iu/perf.data
```

五对完整结果如下；百分比按 candidate 相对 baseline 计算，正数表示候选更慢，负数
表示候选更快。

| 配对 | Empty baseline → candidate（ns/次） | 变化 | 胜者 | Button baseline → candidate（ns/次） | 变化 | 胜者 |
|---|---:|---:|:---:|---:|---:|:---:|
| 1 | 353291.29 → 365841.46 | +3.552% | A | 465144.38 → 450880.54 | -3.067% | B |
| 2 | 375280.42 → 351757.17 | -6.268% | B | 451824.00 → 450029.38 | -0.397% | B |
| 3 | 359919.12 → 351628.54 | -2.303% | B | 462787.46 → 457244.12 | -1.198% | B |
| 4 | 357355.58 → 350639.21 | -1.879% | B | 469202.00 → 448547.83 | -4.402% | B |
| 5 | 354675.96 → 349400.21 | -1.487% | B | 449614.00 → 454140.83 | +1.007% | A |
| 中位 | 357355.58 → 351628.54 | -1.603% | — | 462787.46 → 450880.54 | -2.573% | — |

Empty 为 4/5 对更快，成对几何均值改善 1.727%；Button 也为 4/5 对更快，成对
几何均值改善 1.630%。这些幅度较小，收益结论基于中位数、配对几何均值和静态调用
点同时同向，而不是要求每一对都胜出。

## 资源与 perf 证据

五对运行的两侧资源均固定如下：

| 场景 | allocations/次 | allocated bytes/次 | peak live bytes | key-width allocations | retained layout roots |
|---|---:|---:|---:|---:|---:|
| Empty ReconcileProbe | 5 | 4680 B | 4480 B | 0 | 1 |
| Button | 1541 | 50760 B | 4480 B | 0 | 1 |

baseline 的 `perf-typeid.data` 为 931 samples、0 lost；candidate 的 `perf.data` 为
969 samples、0 lost。候选报告中 `reconcile_existing` 为 84 samples/9.00%，
`WidgetTree::get_raw` 为 29/3.03%，`Button::type_id` 为 5/0.58%；baseline
fresh Button `TypeId` 为 6/0.65%。两侧样本数、调度和运行状态不同，且 Button
TypeId 样本混合了 `can_reuse`、patch 及其他运行时路径，故这些占比不可直接比较，
也不能据此独立归因。

候选反汇编确认 non-Calendar owner 路径由原五次动态类型识别降为一次；Calendar
的 guarded downcast 和 live fallback 仍保留。反汇编只作为调用数/保留语义的证据，
没有使用内联汇编，也没有内联汇编必要性依据。

## 验证

以下五个 lib 测试均使用 `--release --exact --nocapture`，各命中 1 项并通过：

- `ui::widgets::display::calendar::tests::calendar_reusing_paths_match_owned_month_geometry`
- `ui::widgets::navigation::anchor::anchor_dynamic_capture_tests::anchor_container_dynamic_capture_parent_reconcile_reuses_id_and_state`（navigation）
- `ui::widgets::other::misc::transfer::layout_tests::transfer_reusing_paths_match_owned_filtered_geometry`
- `ui::widgets::display::carousel::carousel_dynamic_capture_tests::carousel_custom_arrows_dynamic_capture_reuses_identity_and_complete_outputs`
- `ui::widgets::display::image::image_dynamic_capture_tests::image_error_dynamic_capture_survives_same_source_parent_reconcile`

keyed benchmark exact `stable_keyed_reconcile_profile` 命中 1 项并通过；碰撞、身份
顺序和 `retained_layout_roots=1` 断言均通过。目标文件执行
`rustfmt --edition 2024`/`--check` 与 `rtk git diff --check` 均通过。baseline 与
candidate 的 `output-lib-uix` warning fingerprint 一致，未发现新增或变化 warning。

本轮热点是重复动态类型分派，不是算术、SIMD 或指令级内核；没有内联汇编依据。
