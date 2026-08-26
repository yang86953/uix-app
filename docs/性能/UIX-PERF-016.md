# UIX-PERF-016 单次可变访问合并上下文查询

## 结论

稳定 keyed 协调在进入 `reconcile_existing` 后，先用 `WidgetTree::get` 比较
`provider_context`，随即又对同一节点调用 `get_mut` 更新元数据。两次访问使用相同的
执行状态、槽位与 generation 校验，中间没有回调、状态转换或用户代码。

本轮将比较和更新合并到一个局部 `get_mut`：先计算上下文是否变化，再按原顺序覆盖
provider context、离场动画、光标、UIX scope 和 captured effects。节点缺失或已失效时
仍返回 `context_changed = true`，旧 effects 仍在原 setter 位置释放，可变借用也在后续
State 绑定、事件、patch 和生命周期操作之前结束。

在 512 个稳定 keyed 节点、每轮 24 次协调的五对 release 二进制交替测试中，空组件
场景 4/5 更快，中位改善 1.489%，成对比值几何均值改善 1.006%；Button 场景
3/5 更快，中位改善 0.780%，几何均值改善 0.816%。全部资源指标不变。

## SMC 与语义边界

该优化只位于 UI System 的 coordination Module：它缩短同一节点、同一协调阶段内的
安全访问序列，不把节点引用、槽位、generation 或生命周期所有权带出局部表达式。
`WidgetTree` 仍唯一拥有节点实例与执行状态，State、Effect、事件及动态子树 Component
的契约均未变化。

`provider_context` 的比较必须发生在 `set_provider_context` 之前；
`replace_captured_effects` 的位置不能前移，因为覆盖它会立即释放旧租约。最终实现严格
保持五个 setter 的原顺序。现有测试没有直接暴露私有 `reconcile_existing` 的缺失或
stale 分支，但 `get` 与 `get_mut` 共享相同门控，新的 `else { true }` 保留了原先
`is_none_or` 的结果。

## 基线与方法

基线提交为 `8a96f43350f88877b240f0cb916c140134d815f1`：

```text
baseline target: /tmp/uix-perf016-context-base.hPGuKK
baseline binary: /tmp/uix-perf016-context-base.hPGuKK/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
baseline perf:   /tmp/uix-perf016-context-base.hPGuKK/perf.data
candidate target: /tmp/uix-perf016-context-cand.7YUjmH
candidate binary: /tmp/uix-perf016-context-cand.7YUjmH/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
candidate perf:   /tmp/uix-perf016-context-cand.7YUjmH/perf.data
```

两侧都使用 release、`--features test-harness`、frame pointers、line-table debug info 且
不 strip。`A` 为基线、`B` 为候选，执行顺序是 `B,A,A,B,B,A,A,B,B,A`；下表每对
均按基线到候选记录，每次二进制内部取 9 轮中位：

| 配对 | 空基线 → 候选（ns/次） | Button 基线 → 候选（ns/次） |
|---|---:|---:|
| 1 | 367465.79 → 362852.54（-1.255%） | 453169.83 → 456537.00（+0.743%） |
| 2 | 370328.21 → 352098.21（-4.923%） | 456564.96 → 453003.71（-0.780%） |
| 3 | 357630.25 → 370112.38（+3.490%） | 457245.79 → 447092.21（-2.221%） |
| 4 | 356961.50 → 352306.50（-1.304%） | 466062.79 → 450467.67（-3.346%） |
| 5 | 355192.38 → 352136.62（-0.860%） | 454325.75 → 461634.04（+1.609%） |
| 中位 | 357630.25 → 352306.50（-1.489%） | 456564.96 → 453003.71（-0.780%） |

空场景成对候选/基线几何均值改善 1.006%，候选胜 4/5；Button 改善 0.816%，
候选胜 3/5。收益幅度较小，保留依据是两场景中位数和成对几何均值同向、静态调用点
确实减少一次，以及资源契约完全不变，而不是每一对都战胜测量噪声。

## 资源与采样证据

两侧所有有效运行都保持：

| 场景 | allocations | allocated bytes | peak live bytes | key-width | roots |
|---|---:|---:|---:|---:|---:|
| 空 ReconcileProbe | 5 | 4680 B | 4480 B | 0 | 1 |
| Button | 1541 | 50760 B | 4480 B | 0 | 1 |

基线 perf 共 983 个用户态 cycles 样本、丢失 0 个；`reconcile_existing` 为 106 个
样本、11.02% self，`get_raw` 为 23 个样本、2.53% self。候选共 968 个样本、丢失
0 个；对应为 97 个样本、10.18% self 和 29 个样本、3.04% self。

不可变 `get` 会内联到 `reconcile_existing`，删除的一次调用不能靠独立符号样本精确
归因；两次采样总量不同，也不能把 `get_raw` 占比上升解释为回退。采样用于确认节点
查询确属当前协调热点，最终收益结论来自静态调用点消除与交错配对计时。

## 验证

以下精确测试各命中 1 项并通过：

- `enabled_reconcile_reuses_precomputed_snapshot`
- `anchor_container_dynamic_capture_parent_reconcile_reuses_id_and_state`（navigation）
- `alternating_effect_dependencies_release_stale_branch`

keyed benchmark exact 的基线、候选及全部配对运行也都各命中 1 项并通过。两侧可比
构建均为 163 条既有 warning，没有新增 warning；`rustfmt --edition 2024` 与
`git diff --check` 通过。

本轮热点是重复树槽位查询，不是算术、SIMD 或指令级内核，没有使用内联汇编的剖析
依据。
