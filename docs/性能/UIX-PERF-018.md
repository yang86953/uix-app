# UIX-PERF-018 按禁用状态快路跳过无障碍名称副本

## 结论

本轮只在 `ViewAdapter::reconcile_existing` 的 disabled 判定处增加窄快路：
先读取 `AccessibilityOverride::disabled_override()`，没有显式覆盖时，Button
直接读取 `disabled || loading`；其他快照仍调用原有
`next_fields.accessibility().state.disabled`。公开 `SnapshotFields`、`Button` 与
`AccessibilitySnapshot` 均未修改，disabled cleanup、patch、覆盖保存顺序不变，
最终语义快照仍完整。

Button 资源差异可由现有 keyed fixture 直接归因：
`1541 - 5 = 1536 = 512 × 3`。三项是每个 Button 的新版快照文本副本、
无障碍名称副本和当前快照文本副本；本轮仅消除其中的无障碍名称路径，故
`1541 → 1029 = 512 × 2 + 5`，而不是改变公开快照存储。

## 实现与证据边界

实现保持以下优先级：

1. `accessibility_override.as_ref()` 的显式 disabled（state 优先于 replacement）；
2. `SnapshotFields::Button` 的 `disabled || loading`；
3. 其他快照的完整 AccessibilitySnapshot fallback。

这保留了覆盖优先级和 Button 的无障碍语义，同时避免 enabled Button 为了读取
disabled 而创建 `AccessibilitySnapshot::named` 及其临时名称字符串。没有改变
`next_fields` 本身，也没有改变后续 `patch_widget`、事件清理或覆盖保存时序。

allocator 调用栈观察到 `next snapshot` 14 block、`accessibility` 5、`current
snapshot` 4；相关 cycles children 约为 `1.60 / 0.58 / 0.47`。uprobe 验证受
tracefs 权限与 allocator DIE 解析失败限制，因此这些栈只作为结构证据，不能替代
完整的指令级归因。

## SMC 边界

改动属于 UI System 的 coordination Module，仅消费 Widget Snapshot 与无障碍覆盖
的窄读取；Widget、SnapshotFields 和 AccessibilitySnapshot 仍是原有契约，
WidgetTree 继续唯一拥有 live widget、事件清理和生命周期状态。没有新增公开 API、
跨 Module 所有权或缓存引用。

## 基线、候选与方法

基线提交：`707775140af21c53168d40cf6752fb8c62c0f845`。A 为已保留的 PERF-017
TypeId 候选，B 为本轮无障碍 disabled 窄快路候选；两侧均为 release keyed
integration profile，结果按 A → B 记录。

```text
A target:  /tmp/uix-perf017-owner-typeid-cand.XOB7Iu
A binary:  /tmp/uix-perf017-owner-typeid-cand.XOB7Iu/release/deps/keyed_reconcile_allocation_contract-7075994198236f54
A Build ID: 2c3266d05392e52744530d72ca37c0e31eed0881
A mtime:   2026-08-27 02:56:36 +0800

B target:  /tmp/uix-perf018-accessibility-cand.tKHO8j
B binary:  /tmp/uix-perf018-accessibility-cand.tKHO8j/release/deps/keyed_reconcile_allocation_contract-7075994198236f54
B Build ID: ed538e4747e7dc8ffc6012e278021c96ad6d8bd2
B mtime:   2026-08-27 03:33:15 +0800
```

五对 keyed profile 的中位耗时如下；负数表示 B 更快。

| 对 | Empty A → B（ns/次） | 变化 | Button A → B（ns/次） | 变化 |
|---|---:|---:|---:|---:|
| 1 | 350195.88 → 345667.42 | -1.293% | 450453.50 → 439155.25 | -2.508% |
| 2 | 348822.50 → 354872.17 | +1.734% | 454001.42 → 422043.12 | -7.039% |
| 3 | 349814.38 → 343075.46 | -1.926% | 461837.33 → 420943.17 | -8.855% |
| 4 | 354853.79 → 343204.88 | -3.283% | 446293.17 → 435024.96 | -2.525% |
| 5 | 349008.67 → 342251.83 | -1.936% | 448838.79 → 425700.83 | -5.155% |

Empty 中位数为 `349814.38 → 343204.88 ns`，下降 `1.889%`，B 胜 `4/5`，
配对几何均值下降 `1.355%`。Button 中位数为 `450453.50 → 425700.83 ns`，
下降 `5.495%`，B 胜 `5/5`，配对几何均值下降 `5.249%`。

## 资源与 perf

| 场景 | A allocations/次 | B allocations/次 | A → B bytes/次 | peak | key-width | roots |
|---|---:|---:|---:|---:|---:|---:|
| Empty | 5 | 5 | 4680 → 4680 B | 4480 → 4480 B | 0 → 0 | 1 → 1 |
| Button | 1541 | 1029 | 50760 → 35400 B | 4480 → 4480 B | 0 → 0 | 1 → 1 |

B 的 perf 数据为
`/tmp/uix-perf018-accessibility-cand.tKHO8j/perf.data`，`919 samples / 0 lost`。
主要样本为：`reconcile_existing` 73（8.12%）、
`RawVecInner::deallocate` 36（4.17%）、`WidgetTree::get_raw` 28（3.20%）、
`__rust_alloc` 19（2.19%）、`SnapshotFields::accessibility` 2（0.23%）。
候选采样中未出现基线的 Button accessibility-name clone 栈，但仍有两个通用
accessibility fallback 样本；因此只能谨慎说明目标路径样本消失，不能宣称该路径
绝对执行次数为零或占比精确为零。

## 验证

- `button_interaction_gate_preserves_accessibility_override_precedence`：1 passed，
  3 filtered；覆盖 Button disabled/loading 与 AccessibilityOverride state/replacement。
- keyed exact `stable_keyed_reconcile_profile`：1 passed、0 failed；Empty/Button
  资源断言均命中。
- `rustfmt --edition 2024`（adapter）与 `rtk git diff --check`：通过。
- `ui::adapter::tests::enabled_reconcile_reuses_precomputed_snapshot` 与
  `ui::adapter::tests::disabled_reconcile_refreshes_snapshot_after_cleanup`：因仓库
  既有 `tests/unit/ui/widgets/feedback/modal/builder__tests.rs:87,103` 调用不存在的
  `TestApp::tree()`（E0599）而阻塞，命中数均为 0，不能计为通过。

候选 release 构建记录 `163 warnings emitted`，未发现本轮新增 warning。该热点没有
内联汇编依据。
