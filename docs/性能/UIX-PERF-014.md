# UIX-PERF-014 空结构 State 绑定快路

## 结论

PERF-013 后重新采集的 1079 个用户态 cycles 样本中，
`WidgetTree::replace_node_captured_state_binds` 占 1.38% self，局部约 14 个样本；
其中空 `Vec<ReconcileBindLease>` 的迭代与收集约占 1.19%。稳定声明没有捕获结构性
State 时，原实现仍为每个非根节点克隆树级 reconcile 回调、读取端口键并从空迭代器
收集租约。

本轮为真正的空输入增加运行时树内部快路：先释放输入 `Vec`，随后仍通过
`WidgetTree::get_mut` 定位节点，并用原 `replace_reconcile_state_binds(Vec::new())`
释放旧租约。非空节点绑定和根绑定路径完全不变。

在 512 个稳定 keyed 节点、每轮 24 次协调的五对独立 release 二进制交替测试中，
空组件场景 5/5 更快，合并中位改善 5.483%，成对比值几何均值改善 4.683%；
真实 Button 场景 3/5 更快，中位改善 0.915%，几何均值改善 0.653%。两类场景的
申请次数、累计申请字节、峰值存活、key-width 申请与保留 Layout 根全部不变。

## 实现与语义边界

`replace_node_captured_state_binds` 的空输入分支保持以下顺序：

1. 显式 `drop(state_binds)`，让可能带容量的空输入缓冲区仍在旧租约替换前释放；
2. 使用原 `get_mut(id)`，保留 tree scope、generation 与停止树门控；
3. 仅在节点仍有效时调用原 `replace_reconcile_state_binds(Vec::new())`；
4. 由旧 `ReconcileBindLease` 的 `Drop` 精确执行 `unbind_reconcile_site`。

快路没有改用 `clear_reconcile_state_binds`，因为字段替换是原路径已经验证的租约交接
语义；保留同一入口可避免改变自定义解绑实现发生 panic 时的字段替换边界。节点缺失或
代际失效时仍不触碰任何节点，非空输入仍按原顺序创建请求端口租约。

## SMC 边界

该优化位于 UI System 的 widget runtime Module。`WidgetTree` 继续唯一拥有 reconcile
请求端口、节点身份和运行状态，`BoxedWidget` 继续唯一拥有节点租约集合；coordination
Module 仍只交接本轮捕获的 `StatePaintBind` 源，不判断旧租约状态，也不取得解绑责任。

空输入判断留在拥有替换生命周期的运行时树内，因此建树与原位协调两个调用方共享同一
契约。没有把节点所有权、停止状态或 `ReconcileBindLease` 的 Drop 语义泄漏给上层。

## 基线与方法

基线产品提交为 `a9ee1e320ec3548bf811cf249581638530c15271`。两侧均以
`strip=none`、`line-tables-only`、frame pointers 和 `test-harness` 构建独立 release
二进制：

```text
B target: /tmp/uix-perf014-state-bind-base.xHIJRt
B binary: /tmp/uix-perf014-state-bind-base.xHIJRt/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
B perf:   /tmp/uix-perf014-state-bind-base.xHIJRt/perf.data
A target: /tmp/uix-perf014-state-bind-cand.W943Tr
A binary: /tmp/uix-perf014-state-bind-cand.W943Tr/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
A perf:   /tmp/uix-perf014-state-bind-cand.W943Tr/perf.data
```

基线 exact 单跑命中 1 项。空场景中位为 416837.54 ns/次，资源为 5 alloc、
4680 B、peak 4480 B、key-width 0、roots 1；Button 场景为 520802.92 ns/次，
1541 alloc、50760 B、peak 4480 B、key-width 0、roots 1。

成对顺序为 `B,A,A,B,B,A,A,B,B,A`，每次二进制内部取 9 轮中位：

| 配对 | 空 B → A（ns/次） | Button B → A（ns/次） |
|---|---:|---:|
| 1 | 428612.12 → 412524.46 | 518472.71 → 506581.25 |
| 2 | 403690.79 → 395391.50 | 517077.17 → 500713.17 |
| 3 | 420187.58 → 392549.08 | 498284.00 → 514188.83 |
| 4 | 421826.88 → 398698.54 | 511257.08 → 505395.29 |
| 5 | 422904.75 → 399730.04 | 510008.92 → 511389.00 |
| 中位 | 421826.88 → 398698.54（-5.483%） | 511257.08 → 506581.25（-0.915%） |

空场景成对 A/B 几何均值为 0.953169，即改善 4.683%；Button 为 0.993467，
即改善 0.653%。Button 的绝对工作还包含快照、样式和字符串申请，因此快路占比被稀释，
但两种场景均没有观察到总体回退。

## 资源与指令证据

全部有效运行的资源指标固定不变：

| 场景 | allocations | allocated bytes | peak live bytes | key-width | roots |
|---|---:|---:|---:|---:|---:|
| 空 ReconcileProbe | 5 | 4680 B | 4480 B | 0 | 1 |
| Button | 1541 | 50760 B | 4480 B | 0 | 1 |

基线 perf 共 1079 个样本、丢失 0 个；目标函数占 1.38% self，annotate 的局部样本
集中在空租约 Vec/迭代器搬运，`collect` 归因约 1.19%。候选 perf 共 1073 个样本、
丢失 0 个；`perf report` 中目标符号没有直接样本，`perf annotate` 也没有该符号样本。
这与空分支被内联或消除一致，但采样占比受总样本与内联归因影响，不能据此宣称目标成本
精确降为零；保留依据是 5/5 的空场景成对计时与代码结构共同成立。

## 验证

以下两个精确测试各命中 1 项并通过：

- `ui::widget_runtime::widget::tree_dirty::tests::empty_node_state_binds_unbinds_existing_lease`
- `ui::widget_runtime::widget::tree_dirty::tests::empty_node_state_binds_ignores_removed_node`

第一个测试用 `Vec::with_capacity(8)` 的空输入确认已有 State 租约被精确解绑；第二个测试
确认正式移除后的 stale `WidgetId` 不会重新绑定或触碰其他节点。benchmark exact 也命中
1 项并通过；既有 138 条编译警告，没有新增警告或错误。两个代码文件分别为 640 行和
133 行，均低于 1500 行限制。

本轮热点是空集合控制流、原子引用计数与迭代器状态搬运，没有算术、SIMD 或指令级内核，
不存在使用内联汇编的证据。
