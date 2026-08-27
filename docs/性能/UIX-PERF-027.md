# UIX-PERF-027 Layout 根条目提示跳过重复哈希

## 结论

本轮以新的用户态调用链基线确认 `reconcile_existing` 下的 Layout 失效上报会反复进入
`push_layout_invalidation -> push_layout_until_root -> HashSet::contains -> SipHash`。最终候选在
`InvalidationQueue` 自有的 `items` 与 `layout_ids` 之间保存一个经过完整 `WidgetId` 复核的根
条目位置；同一轮后续请求直接读取稳定 Vec 槽位，不再为同一根重复计算 SipHash。

四场景五对隔离 release 运行中，unkeyed stable、unkeyed structural、keyed Empty、keyed
Button 的配对几何均值分别改善 `10.503%`、`5.165%`、`11.433%`、`2.440%`，均为 B 胜
`5/5`。热段分配、申请字节、峰值与保留根不变；提示使用 `u32`，同编译器等价字段布局
检查为 `InvalidationQueue 136 B -> 136 B`。条目数超出 `u32` 可编码范围时提示保持为零，
安全回退原哈希查询，不改变正确性。

## 剖析与候选选择

旧 flat profile 只能说明 SipHash 自耗时存在，不能证明它属于哪条父栈。本轮重新使用
`cycles:u`、`999 Hz`、frame-pointer 调用链，在同一次 record 中串行执行精确 unkeyed
测试十次：

```text
data:    /tmp/uix-perf027-callchain-base.data
header:  /tmp/uix-perf027-callchain-base.header.txt
script:  /tmp/uix-perf027-callchain-base.script.txt
samples: 1540
lost:    0
exact:   10/10 passed
```

header 不含 `exclude_callchain_user=1`，脚本可见真实用户态父栈。确认的关键路径包括：

```text
reconcile_existing
  -> WidgetTree::push_layout_invalidation
  -> InvalidationQueue::push_layout_until_root
  -> HashMap/HashSet::contains
  -> SipHasher13::write
```

另有 `patch_builtin_widget`、`invalidate_paint -> path_has_visual_transform`、`replace_widget`
和 `register_focusable -> tab_index` 父栈。候选只处理其中已经具备局部等价变换且没有碰撞
风险的 Layout 根查询；没有更换随机哈希器，也没有采用弱摘要。

## 实现与 SMC 边界

`InvalidationQueue` Component 唯一拥有失效条目、Layout 身份集合及聚合标志，
`WidgetTree` 只通过队列端口上报并依据返回值决定是否扫描祖先。最终实现保持该所有权：

1. 提示保存一基 `u32` 条目位置，零表示未建立；每次快返前仍比较完整的
   `Invalidation::Layout(root_id)`，因此树作用域、槽位和 generation 任一不同都不会误命中；
2. `root_id == id` 时，以一次 `HashSet::insert` 同时完成存在性判断和条目登记；
3. `root_id != id` 时仍保留根查询，根不存在且当前节点重复时仍返回 `true`，不会漏掉祖先
   传播；
4. 通用 `push(Layout(root))` 先写入根时，首次专用查询只扫描一次条目建立提示；后续同根
   请求走槽位快返；
5. `clear` 与 `clear_layout` 同步复位提示；生产路径对条目只有追加、完整清理或消费全部
   Layout，提示下标在有效期内稳定；
6. 超过 `u32` 的极端条目位置不缓存，继续执行原有 HashSet 路径。

没有新增分配、跨 Module 缓存、弱身份摘要或生命周期所有者。

## 基线与候选

```text
A target:  /tmp/uix-perf026-render-empty-query-cand.CdY4JY
A unkeyed: /tmp/uix-perf026-render-empty-query-cand.CdY4JY/release/deps/unkeyed_reconcile_allocation_contract-0c8dd90538cdd64a
A Build ID: a0bbee25f64c3c6de2fafe52f61f47493635bf31
A keyed:   /tmp/uix-perf026-render-empty-query-cand.CdY4JY/release/deps/keyed_reconcile_allocation_contract-7075994198236f54
A Build ID: 0d04eb05c8d26c05d51cde2fe28907461c2302be

B target:  /tmp/uix-perf027-layout-root-hint-u32-cand.bCu0Ge
B unkeyed: /tmp/uix-perf027-layout-root-hint-u32-cand.bCu0Ge/release/deps/unkeyed_reconcile_allocation_contract-e4f5b6fcfdba1451
B Build ID: 45a79a32c82c49d32e2bbffb6b22a5630289c8db
B mtime:   2026-08-27 09:38:43 +0800
B keyed:   /tmp/uix-perf027-layout-root-hint-u32-cand.bCu0Ge/release/deps/keyed_reconcile_allocation_contract-3ebe7dd194900296
B Build ID: 19ac20824415e45232b022d27a3176919eee2e47
B mtime:   2026-08-27 09:38:43 +0800
```

两组均按 `A,B,B,A,A,B,B,A,A,B` 运行；unkeyed 的同一进程同时报告 stable 与
structural，keyed 的同一进程同时报告 Empty 与 Button。20 个正式进程都以 exact 方式
命中一项并通过。原始日志为候选 target 下的 `unkeyed-abba.log` 与 `keyed-abba.log`。

## 五对耗时

| 对 | Unkeyed stable A -> B（ns/次） | 变化 | Unkeyed structural A -> B（ns/次） | 变化 |
|---|---:|---:|---:|---:|
| 1 | 208754.83 -> 193866.88 | -7.132% | 251110.92 -> 239966.29 | -4.438% |
| 2 | 213470.75 -> 187434.83 | -12.197% | 252295.21 -> 238339.46 | -5.532% |
| 3 | 211203.17 -> 186448.42 | -11.721% | 249422.33 -> 239020.33 | -4.170% |
| 4 | 208520.62 -> 185954.96 | -10.822% | 259610.62 -> 239327.54 | -7.813% |
| 5 | 210104.83 -> 187919.50 | -10.559% | 250058.92 -> 240511.46 | -3.818% |

Stable 中位数为 `210104.83 -> 187434.83 ns`，改善 `10.790%`，配对几何均值改善
`10.503%`。Structural 中位数为 `251110.92 -> 239327.54 ns`，改善 `4.693%`，配对
几何均值改善 `5.165%`。两组均为 B 胜 `5/5`。

| 对 | Keyed Empty A -> B（ns/次） | 变化 | Keyed Button A -> B（ns/次） | 变化 |
|---|---:|---:|---:|---:|
| 1 | 242013.04 -> 214498.50 | -11.369% | 333175.25 -> 316223.83 | -5.088% |
| 2 | 244844.58 -> 216584.50 | -11.542% | 329998.42 -> 324795.75 | -1.577% |
| 3 | 248427.12 -> 215753.33 | -13.152% | 325449.88 -> 311856.50 | -4.177% |
| 4 | 244859.58 -> 227826.00 | -6.957% | 327630.62 -> 324494.38 | -0.957% |
| 5 | 249395.21 -> 214526.92 | -13.981% | 329328.04 -> 328306.50 | -0.310% |

Empty 中位数为 `244859.58 -> 215753.33 ns`，改善 `11.887%`，配对几何均值改善
`11.433%`。Button 中位数为 `329328.04 -> 324494.38 ns`，改善 `1.468%`，配对几何
均值改善 `2.440%`。两组均为 B 胜 `5/5`。

## 资源与空间

| 场景 | allocations | allocated bytes | peak | final / retained |
|---|---:|---:|---:|---:|
| Unkeyed stable | 120 | 112,320 B | 4,480 B | final 0 |
| Unkeyed structural | 1,902 | 1,380,672 B | 42,144 B | final 1,024 B |
| Keyed Empty | 5/次 | 4,680 B/次 | 4,480 B | roots 1 |
| Keyed Button | 1,029/次 | 35,400 B/次 | 4,480 B | roots 1 |

A 与 B 的四组资源逐字段一致，两组 keyed 的 key-width 申请仍为零。第一版 `usize`
提示虽有相同改善方向，却使等价布局从 `136 B` 增到 `144 B`，因此未作为最终候选；
`u32` 版本利用原结构尾部填充，等价布局为 `136 B -> 136 B`，不增加每树常驻队列大小。

## 未通过与未实施候选

- RenderHandler 全局空表替换快返：资源不变，但 structural `+1.1110%`、keyed Empty
  `+1.7454%`，已回滚。
- 空节点动画源快返：资源不变，但 structural `+0.1095%`，已回滚。
- Tab-index 单次查询：资源不变，但 stable `+0.770%`、structural `+3.025%`，已回滚。
- VirtualScroll `contains` 后 `get` 重排存在“renderer 已登记但节点尚 pending”的反向工作量，
  当前四场景没有该负载，未实施。

这些结果表明 flat 热点或局部指令减少不能替代四场景配对裁决。

## 候选 perf

为保留符号，另以相同源码和 release 优化、`strip=none`、`debug=1` 构建 unkeyed
候选并重采十次精确测试：

```text
target:   /tmp/uix-perf027-layout-root-hint-u32-perf.Vx9xO7
binary:   /tmp/uix-perf027-layout-root-hint-u32-perf.Vx9xO7/release/deps/unkeyed_reconcile_allocation_contract-4a9418f9c718e45f
Build ID: 414cf1d4dd463e1680407800dbc0f70525d08b7c
data:     /tmp/uix-perf027-layout-root-hint-u32-perf.Vx9xO7/perf.data
samples:  1757
lost:     0
exact:    10/10 passed
```

候选 header 同样不含 `exclude_callchain_user=1`，`push_layout_until_root` 及新的 Vec 槽位
校验均可符号化。top self 中 SipHash `write` 为 `2.88%`、`hash_one` 为 `1.56%`；基线
分别为 `5.20%`、`2.70%`。脚本文本中的 `push_layout_until_root`、SipHash `write` 与
`contains_key<WidgetId>` 符号帧也从基线的 `52/73/93` 个方向性降为 `18/37/10` 个。
两次采样数和二进制不同，这些百分比与帧计数不能单独证明因果，只用于确认目标哈希父栈
按预期收缩；正式裁决仍以同机交错 A/B 与资源契约为准。

## 验证

- `draw::renderer::invalidation::tests::`：`11 passed`；覆盖通用根入队、直接根重复、Paint
  先入队、一基位置、完整/仅 Layout 清理、generation 变化与根缺失重试。
- `propagated_root_layout_suppresses_followup_request`：`1 passed`；覆盖真实 WidgetTree 子请求
  传播至根后建立提示并抑制后续重复。
- 既有 `non_batch_root_layout_suppresses_child_request_and_advances_revision`：`1 passed`。
- 三个修改代码/测试文件以 `rustfmt --edition 2024` 定向格式化，均少于 1500 行；
  `git diff --check` 通过。
- 候选 release 构建报告既有 `163` 条 warning，与 PERF-026 基线一致；无新增 warning 或错误。
- 20 个正式交错进程全部 `1 passed`，四场景资源不变。

该热点是身份哈希与控制流，不是可向量化数值循环；Rust 层提示直接移除了目标工作，
当前没有支持内联汇编的剖析或基准证据。
