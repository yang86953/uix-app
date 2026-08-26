# UIX-PERF-013 协调器快照复用门禁

## 结论

PERF-012 之后的指令采样中，ViewAdapter::reconcile_existing 占
12.02% self。此前的稳定 Button 场景基线为 512 个 keyed Button、每轮
24 次协调、9 轮；独立基线二进制单跑的中位耗时为 524451.00 ns/次，
每次 2053 次申请、66120 B 累计申请、4480 B 峰值存活、0 次 key-width
申请，最终保留 1 个 Layout 根。

本轮最终实现复用协调开始时已经取得的新版 SnapshotFields，但只在
next_disabled=false 且两次快照观察之间没有 Pointer/Focus 用户事件清理
时借用。disabled 路径一律在事件清理完成后重新取得快照。候选在 Button
场景中稳定减少 512 次申请和 15360 B 累计申请；借用化 v2 五对配对的耗时没有
稳定收益，因此不宣称耗时改善。

## 审计与否决

最先审计了直接复用 BoxedWidget::interaction_enabled 的路线。该方法属于
公开可实现的 EventHandler 行为，公开自定义 EventHandler 可以返回与
SnapshotFields 中 disabled 语义不一致的结果；用它决定是否复用快照会把
协调时序绑定到组件实现细节，也可能错误地跳过 disabled 清理后的第二次
快照观察。

BoxedWidget 的通用回退还会为 DynamicLabel 额外调用 semantic_text()，而
协调器原有的 SnapshotFields::accessibility 路径不会产生这次观察。因此该
路线被否决，最终门禁只使用协调器已经计算出的 next_disabled，并保留
Pointer/Focus 清理的原有事件顺序。

## 最终实现

reconcile_existing 先取得一次新版 widget.snapshot_fields()，从中计算
无障碍 disabled 状态。只有 next_disabled=false 时，才把对这份快照的
借用传给 patch_widget：

1. enabled 路径不存在 PointerLeave、DragEnd 或 FocusOut 清理，借用在同一
   次同步调用内完成配置与布局分类；
2. disabled 路径先按原顺序清理 pointer hover、pointer gesture 和 focus，
   再让 patch_widget 重新调用 widget.snapshot_fields()；
3. patch_widget 的参数是 Option<&SnapshotFields>，避免把包含大量字符串、
   样式和组件字段的 SnapshotFields 枚举按值传递；
4. patch 完成后仍按原顺序执行位置、选择策略、焦点、无障碍、事件注册、
   render/system handler、State、动画源和失效处理。

这样借用只跨越当前协调调用，不跨越可变树借用、事件回调、递归协调或
生命周期边界。

## SMC 边界与契约

该优化位于 UI System 的 coordination Module。coordination Module 负责把
ViewNode 声明映射为 WidgetTree 更新，但不取得 WidgetTree 槽位表、节点
generation 或组件生命周期所有权。widget runtime Module 继续拥有节点和
运行时状态；SnapshotFields 是只读的 widget snapshot Component 契约；
Pointer、Focus 与 EventHandler 各自保留其事件和清理责任。

实现保持以下契约：

- enabled 与 disabled 的判定来自同一份新版无障碍快照；
- disabled 的 Pointer/Focus 清理先于最终 patch 快照，不能被快路吞掉；
- 公开自定义 EventHandler 不会被 snapshot disabled 结果替代；
- DynamicLabel 的依赖观察和布局后探测不改变；
- 组件 patch、布局/绘制分类、失效顺序、handler 替换和 State 绑定语义不变；
- SnapshotFields 借用不被缓存、存储或跨线程发送。

## 基线与配对方法

基线产品代码锚定提交
daa1765ae1834cdc1e5cbd64396e3a617177f162。基线使用独立 target 和冻结的
可执行文件：

~~~text
B target: /tmp/uix-perf013-button-gate-base.9V6jDh
B binary: /tmp/uix-perf013-button-gate-base.9V6jDh/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
~~~

借用化 v2 使用独立 target：

~~~text
A target: /tmp/uix-perf013-button-gate-borrowed.s2t9ny
A binary: /tmp/uix-perf013-button-gate-borrowed.s2t9ny/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
~~~

两侧均使用 release、strip=none、line-tables-only 和 frame pointers 构建
同一个 keyed integration benchmark：

~~~bash
rtk proxy env \
  CARGO_TARGET_DIR=/tmp/uix-perf013-button-gate-borrowed.s2t9ny \
  CARGO_PROFILE_RELEASE_STRIP=none \
  CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
  RUSTFLAGS=-Cforce-frame-pointers=yes \
  cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness --no-run
~~~

直接运行二进制的参数为
stable_keyed_reconcile_profile --exact --nocapture；每次设置唯一
UIX_RECONCILE_PROFILE_LABEL。配对顺序为
B,A,A,B,B,A,A,B,B,A，每侧五次。baseline binary 单跑的
stable_keyed_button_reconcile 中位为 524451.00 ns/次，
2053 alloc、66120 B、peak 4480、roots 1；这只是固定基线参考，不替代
交替配对统计。

## 借用化 v2 结果

下表是有效的五对交替结果，A 为借用化 v2，B 为原始 baseline。百分比
按 A 相对 B 计算；空场景是 ReconcileProbe 负面对照。

| 配对 | Button B → A（ns/次） | Button 变化 | 空 B → A（ns/次） | 空场景变化 |
|---|---:|---:|---:|---:|
| 1 | 515000.38 → 524511.12 | +1.847% | 411027.08 → 425945.42 | +3.630% |
| 2 | 520888.08 → 536002.25 | +2.902% | 416316.12 → 416208.42 | -0.026% |
| 3 | 516917.29 → 522886.83 | +1.155% | 397858.21 → 414077.75 | +4.077% |
| 4 | 520863.04 → 512587.92 | -1.589% | 418695.17 → 420405.88 | +0.409% |
| 5 | 523792.67 → 518699.79 | -0.972% | 415974.67 → 423865.75 | +1.897% |
| 中位 | 520863.04 → 522886.83 | +0.389% | 415974.67 → 420405.88 | +1.065% |

Button A 只有 2/5 对更快，成对 A/B 比值几何均值为 1.006541，即几何
上慢 0.654%。空对照中位慢 1.065%，几何均值为 1.019838，即慢 1.984%。
因此耗时不宣称收益；Button 的单个 +2.902% 配对没有重复出现，不能视为
可重复的超过 2% 回退，按约束不再扩展轮次。

## 资源与语义结果

Button 场景每次协调的资源在所有有效运行中固定为：

| 指标 | baseline B | 借用化 v2 A | 差值 |
|---|---:|---:|---:|
| allocations | 2053 | 1541 | -512（-24.94%） |
| allocated bytes | 66120 B | 50760 B | -15360 B（-23.23%） |
| peak live bytes | 4480 B | 4480 B | 不变 |
| key-width allocations | 0 | 0 | 不变 |
| retained layout roots | 1 | 1 | 不变 |

空 ReconcileProbe 负面对照在两侧都保持 5 alloc、4680 B、peak 4480、
key-width 0 和 roots 1。稳定 keyed profile 同时执行强制全碰撞前置断言，
并验证碰撞回退后的身份顺序、移除清理、Button 子节点身份顺序和唯一根
Layout。

协调器快照计数契约的两个精确测试均命中 1 项：

- ui::adapter::tests::enabled_reconcile_reuses_precomputed_snapshot
- ui::adapter::tests::disabled_reconcile_refreshes_snapshot_after_cleanup

stable_keyed_reconcile_profile exact 命中 1 项并通过；碰撞、身份顺序和
retained_layout_roots=1 断言均通过。本文档控制在 1500 行以内；本轮基线
已有 138 条编译警告，未观察到新增警告或错误。

## 指令证据与限制

本轮目标是快照所有权和调用次数，现有指令采样没有显示必须用手写算术、
SIMD 或内联汇编才能改善的热点。没有内联汇编依据。

Button 的申请次数和字节下降在五对中保持完全一致，但耗时方向受运行时
环境影响：候选在 2/5 Button 配对更快，整体中位仅变化 +0.389%，几何
均值为 +0.654% 慢。后续若要宣称耗时收益，应在固定 CPU/调度条件下继续
独立基准；当前证据只支持资源收益和语义安全，不支持耗时收益。
