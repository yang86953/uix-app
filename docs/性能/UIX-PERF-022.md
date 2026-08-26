# UIX-PERF-022 无 key 同序声明按位置直接复用

## 结论

本轮为同长度、全 unkeyed、逐位置类型可复用的声明增加直接协调路径。候选只读取一次
父级子序列并分类 keyed/unkeyed；keyed 继续执行唯一性门禁，unkeyed 则按位置验证
`key=None` 与 `can_reuse_current`。任一长度、key、类型或身份不匹配都会回退原通用路径。

512 个稳定无 key 同级节点每次协调从 `16 alloc / 80,500 B / peak 63,392 B`
降到 `5 alloc / 4,680 B / peak 4,480 B`。五对隔离 release 运行中，unkeyed
stable 配对几何均值改善 `15.003%`；长度交替变化的 structural 负面对照改善
`4.641%`；既有 keyed Empty 与 Button 分别改善 `2.673%`、`3.667%`，四场景
均未回退。

## 实现与语义边界

稳定 unkeyed 位置语义下，原通用路径仍会建立旧子顺序、已使用身份集合与新子顺序，
再逐 index 命中同一节点。候选把以下充分条件收敛为直接路径：

1. 父级当前子节点数与新声明数相同；
2. 全部声明与当前节点都没有 key；
3. 每个位置的当前节点存在且 `can_reuse_current` 为真。

命中后继续调用原有 `cancel_pending_removal` 与 `reconcile_existing`，节点 patch、状态绑定、
动画、事件清理、失效传播与 panic 边界均未改动。混合 key、类型替换、长度变化和 stale
身份继续进入通用路径。新增集成测试先证明三枚 unkeyed 身份在稳定协调后完全保留，
再把中间 Input 改为 Button，验证只替换该位置且旧身份失效。

父节点直接子序列的身份唯一性仍由 `WidgetTree` 的唯一所有权和既有构建、移除、
`new_order` 发布路径维护；候选没有建立第二份身份所有者或公开缓存。

## SMC 边界

改动属于 UI System 的 coordination Module，`ViewAdapter` Component 只决定声明协调策略。
`WidgetTree` 继续唯一拥有 live widget、父子身份、pending leave 与生命周期；快路不接管
节点实例，也不跨 Module 保存借用。稳定位置复用与 keyed 身份复用共享同一发布循环，
而通用结构变更路径保持原样。

## 基线与候选

unkeyed 基线与候选：

```text
A target:  /tmp/uix-perf022-unkeyed-base.5CW53O
A binary:  /tmp/uix-perf022-unkeyed-base.5CW53O/release/deps/unkeyed_reconcile_allocation_contract-a86d430783ff922e
A Build ID: 45ffde826d535c63cf91abdf3c5584bfc8022180
A mtime:   2026-08-27 06:01:42 +0800

B target:  /tmp/uix-perf022-direct-unkeyed-cand.kQizw4
B binary:  /tmp/uix-perf022-direct-unkeyed-cand.kQizw4/release/deps/unkeyed_reconcile_allocation_contract-a86d430783ff922e
B Build ID: 0fda66eec73235aceb25f60c8e37ccddb4a73a4b
B mtime:   2026-08-27 06:17:49 +0800
```

keyed 负面对照使用 PERF-018 作为 A，候选 target 内的 keyed binary 作为 B：

```text
A binary:  /tmp/uix-perf018-accessibility-cand.tKHO8j/release/deps/keyed_reconcile_allocation_contract-7075994198236f54
A Build ID: ed538e4747e7dc8ffc6012e278021c96ad6d8bd2

B binary:  /tmp/uix-perf022-direct-unkeyed-cand.kQizw4/release/deps/keyed_reconcile_allocation_contract-105221358a1af8a9
B Build ID: 3121d885dc201c4fa3b49248538ed26067dfa508
B mtime:   2026-08-27 06:17:49 +0800
```

两组都按 `A,B,B,A,A,B,B,A,A,B` 交替运行，所有进程均以 exact 方式实际命中
`1 passed`。

## 五对耗时

| 对 | Unkeyed stable A → B（ns/次） | 变化 | Unkeyed structural A → B（ns/次） | 变化 |
|---|---:|---:|---:|---:|
| 1 | 356270.21 → 312675.50 | -12.236% | 347274.58 → 348880.92 | +0.463% |
| 2 | 355955.46 → 309513.75 | -13.047% | 348014.29 → 329602.71 | -5.290% |
| 3 | 356226.38 → 313883.21 | -11.887% | 361017.83 → 339120.96 | -6.065% |
| 4 | 378769.92 → 301612.71 | -20.370% | 354715.21 → 331322.17 | -6.595% |
| 5 | 361282.08 → 299335.92 | -17.146% | 348665.96 → 329323.42 | -5.548% |

Stable 中位数为 `356270.21 → 309513.75 ns`，B 胜 `5/5`，配对几何均值
改善 `15.003%`。Structural 中位数为 `348665.96 → 331322.17 ns`，B 胜
`4/5`，配对几何均值改善 `4.641%`。

| 对 | Keyed Empty A → B（ns/次） | 变化 | Keyed Button A → B（ns/次） | 变化 |
|---|---:|---:|---:|---:|
| 1 | 365500.00 → 331530.92 | -9.294% | 440269.38 → 415325.96 | -5.665% |
| 2 | 340970.29 → 346964.42 | +1.758% | 426746.12 → 416670.12 | -2.361% |
| 3 | 341917.04 → 330339.50 | -3.386% | 436562.46 → 410625.04 | -5.941% |
| 4 | 345280.46 → 332665.50 | -3.654% | 423978.42 → 416816.25 | -1.689% |
| 5 | 341343.04 → 346949.42 | +1.642% | 425336.42 → 414293.17 | -2.596% |

Keyed Empty 中位数为 `341917.04 → 332665.50 ns`，B 胜 `3/5`，配对几何
均值改善 `2.673%`。Button 中位数为 `426746.12 → 415325.96 ns`，B 胜
`5/5`，配对几何均值改善 `3.667%`。

## 资源与 perf

unkeyed 指标按每轮 24 次协调记录；peak/final 是整轮同时存活量：

| 场景 | A allocations | B allocations | A → B allocated bytes | A → B peak | final |
|---|---:|---:|---:|---:|---:|
| Stable | 384 | 120 | 1,932,000 → 112,320 B | 63,392 → 4,480 B | 0 → 0 |
| Structural | 1,902 | 1,902 | 1,380,672 → 1,380,672 B | 42,144 → 42,144 B | 1,024 → 1,024 |

Stable 候选的申请尺寸只剩 `192×72、8×24、4096×24`；基线还包含
`12288×48` 以及从 `116` 到 `25616 B` 的集合增长序列。Structural 尺寸分布
完全不变，证明长度变化仍走原通用路径。

Keyed 资源负面对照也完全不变：Empty 为 `5 alloc / 4,680 B / peak 4,480 B`，
Button 为 `1,029 alloc / 35,400 B / peak 4,480 B`，两者 key-width 为 `0`、
retained roots 为 `1`。

基线 perf 为 `/tmp/uix-perf022-unkeyed-base.5CW53O/perf.data`，`215 samples / 0 lost`；
候选 perf 为 `/tmp/uix-perf022-direct-unkeyed-cand.kQizw4/perf.data`，
`214 samples / 0 lost`。候选 top self 包括 `reconcile_existing 13.92%`、SipHash
`write 8.27%`、`RawVecInner::deallocate 6.21%`、`hash_one 4.09%`、
`WidgetTree::get_raw 3.47%`。样本总量不足以把每种申请精确映射到唯一调用点，
因此分配结论以固定容量 allocator 直方图和源码控制流为准。

## 验证

- `unkeyed_stable_positions_reuse_and_type_changes_fall_back`：`1 passed`；覆盖稳定
  位置身份复用和类型变化回退。
- `unchanged_effective_policy_keeps_the_new_declaration_for_later_inheritance`：
  `1 passed`；覆盖继承声明事实。
- `reconcile_keeps_matching_positions_and_normalizes_empty_items`：`1 passed`。
- `empty_data_replaces_list_and_drops_declared_slots`：`1 passed`。
- unkeyed 与 keyed release exact profile：均 `1 passed`，资源契约如上。
- 两个定向 Rust 文件 `rustfmt --edition 2024` 与 `rtk git diff --check`：通过。
- release 构建只出现既有 `163` 条 warning，未发现新增 warning 或错误。

该热点由声明分类和临时集合分配主导，Rust 层窄快路已消除目标成本；没有内联汇编依据。
