# UIX-PERF-015 复用类型前置消除专属 owner 查询

## 结论

PERF-014 后的稳定 keyed 协调仍会为 Calendar、Anchor、Transfer 与 Carousel 分别
查询 live 节点。基线 perf 共 1009 个用户态 cycles 样本、丢失 0 个，`get_raw`
占 3.04% self、31 个样本；四个 owner helper 均有独立或内联归因。

所有 `reconcile_existing` 入口都已由 `can_reuse_current` 或 `can_reuse` 验证 live 与
incoming widget 的具体 `TypeId` 相同，`apply_style` 也只原位修改样式而不替换具体类型。
最终实现因此直接用 incoming 类型识别纯类型 owner：Anchor、Transfer 与 Carousel；
Calendar 因为还拥有 `materialized_cells` 运行态，继续保留 live fallback，但仅在 incoming
本身是 Calendar 且新声明状态不足以判定时查询树。

在 512 个稳定 keyed 节点、每轮 24 次协调的五对 release 二进制交替测试中，空组件
场景 4/5 更快，中位改善 2.027%，成对比值几何均值改善 1.857%；Button 场景
5/5 更快，中位改善 5.737%，几何均值改善 5.624%。资源指标全部不变。

## 候选审计与否决

第一版把 `Option<&dyn Widget>` 缓存在四个判定之间，以一次 `tree.get` 替代四次查询。
它保持了各 OR 分支的 `as_any` 短路，却让 trait-object 胖指针跨多项判定存活并反复搬运。
五对结果中空组件 0/5 更快，中位回退 2.013%，几何回退 2.586%；Button 虽然几何
改善 1.746%，也不能抵消通用路径的一致回退。该版本还让 Calendar helper 失去调用点，
新增一条 `dead_code` warning。因此 v1 被完整否决，没有进入最终实现或提交。

最终 v2 不缓存 trait object，也不把运行时类型快照扩展为新契约。它复用协调入口已经
成立的类型相等事实，只保留确有运行态信息的 Calendar 查询。

## 最终实现与语义边界

`reconcile_existing` 在 `apply_style` 后执行以下判定：

1. incoming Calendar 自身已拥有 custom/materialized cell 时直接成立；否则才调用
   `tree.is_calendar_cell_widget(id)` 读取旧 live Calendar 的 materialized 状态；
2. navigation 开启时，Anchor 直接由 incoming 的具体类型判定；
3. Transfer 与 Carousel 同样直接由 incoming 具体类型判定；
4. Image、动态子树捕获、patch、事件、State、Effect 与动画所有权顺序保持不变。

Calendar 从 true 变为 false 时仍可通过 live fallback 进入专属协调，移除旧动态日期格。
Anchor、Transfer 与 Carousel 的 owner 条件只由具体类型定义，不存在额外运行态开关；
类型变化会在 `reconcile_existing` 之前进入节点替换路径，因此不需要再次查询旧类型。

## SMC 边界

该优化位于 UI System 的 coordination Module。coordination 继续消费运行时树公开的安全
节点查询，但只在 Calendar 运行态确有需要时查询；WidgetTree 仍唯一拥有节点槽位、
generation、动态子树和生命周期。具体类型相等由协调 Module 自己的复用前置建立，不把
节点实现、裸槽位或 owner 状态缓存到调用边界之外。

专属动态 Module 仍负责 Calendar cell、Anchor container、Transfer item 与 Carousel
arrows 的捕获、提交和清理；本轮只去掉进入这些 Module 之前的重复类型确认。

## 基线与方法

基线提交为 `0f0d1a094df806918591f373a2ad01de6976873a`：

```text
B target: /tmp/uix-perf015-owner-probe-base.PKX6lf
B binary: /tmp/uix-perf015-owner-probe-base.PKX6lf/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
B perf:   /tmp/uix-perf015-owner-probe-base.PKX6lf/perf.data
A target: /tmp/uix-perf015-owner-type-cand.23OI95
A binary: /tmp/uix-perf015-owner-type-cand.23OI95/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
A perf:   /tmp/uix-perf015-owner-type-cand.23OI95/perf.data
```

基线 exact 单跑命中 1 项。空场景为 370854.62 ns/次，资源为 5 alloc、4680 B、
peak 4480 B、key-width 0、roots 1；Button 为 458579.96 ns/次，资源为
1541 alloc、50760 B、peak 4480 B、key-width 0、roots 1。

配对顺序为 `B,A,A,B,B,A,A,B,B,A`，每次二进制内部取 9 轮中位：

| 配对 | 空 B → A（ns/次） | Button B → A（ns/次） |
|---|---:|---:|
| 1 | 372054.38 → 354665.50 | 489057.58 → 441441.17 |
| 2 | 362329.46 → 378593.71 | 465480.00 → 462837.12 |
| 3 | 362433.42 → 355833.96 | 478625.04 → 447993.46 |
| 4 | 378348.71 → 360750.29 | 467756.79 → 442379.62 |
| 5 | 363195.71 → 354671.38 | 469305.92 → 442256.88 |
| 中位 | 363195.71 → 355833.96（-2.027%） | 469305.92 → 442379.62（-5.737%） |

空场景成对 A/B 几何均值为 0.981432，即改善 1.857%；Button 为 0.943761，
即改善 5.624%。可比 release 构建 warning 为 163 → 163，没有新增 warning。

## 资源与指令证据

两侧所有有效运行都保持：

| 场景 | allocations | allocated bytes | peak live bytes | key-width | roots |
|---|---:|---:|---:|---:|---:|
| 空 ReconcileProbe | 5 | 4680 B | 4480 B | 0 | 1 |
| Button | 1541 | 50760 B | 4480 B | 0 | 1 |

候选 perf 共 976 个样本、丢失 0 个；`get_raw` 为 3.17% self、28 个样本。由于总样本
不同，不能把 3.04% → 3.17% 当成回退。静态检查确认 adapter 目标区域不再调用
Anchor、Transfer 与 Carousel 的 live owner helper，Calendar helper 只保留在条件分支；
perf 中残留的 Carousel/Anchor 符号来自其他建树或动态路径。保留依据是静态调用点消除与
五组成对计时共同成立，而不是采样占比变化。

## 验证

以下四个精确测试各命中 1 项并通过：

- `calendar_reusing_paths_match_owned_month_geometry`
- `carousel_custom_arrows_dynamic_capture_leave_reentry_and_shutdown`
- `anchor_container_dynamic_capture_leave_reentry_reuses_id_and_state`（navigation）
- `transfer_reusing_paths_match_owned_filtered_geometry`

keyed benchmark exact 也命中 1 项并通过，`rustfmt --edition 2024` 与
`git diff --check` 通过。修改后的 adapter 文件低于 1500 行。Transfer 当前没有专门的
动态协调测试，因此使用最近的 keyed/layout 复用测试，并由类型复用前置和 benchmark
共同覆盖通用路径。

本轮热点是重复身份查询与动态分派，不是算术、SIMD 或指令级内核，没有使用内联汇编的
证据。
