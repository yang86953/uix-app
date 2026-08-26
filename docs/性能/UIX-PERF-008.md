# UIX-PERF-008 空渲染 sidecar 清理快路

## 结论

稳定 keyed 协调的下一个可隔离 CPU 热点位于 `RenderHandlerTable::clear_widget`：绝大多数普通节点没有 Select 选项或 VirtualScroll 行 renderer，但每轮替换组件和更新 render handler 时，仍会对两张全局空 `HashMap` 执行带哈希的删除。本轮在每张 sidecar 表为空时直接跳过该表的寻址删除；表非空时仍执行原有精确清理。

在 512 个稳定 keyed 同级节点、每轮 24 次完整协调的 release 场景中，五组交替配对 A/B 的总体中位耗时从 468,294.88 ns/次降至 444,512.75 ns/次，改善约 5.08%。五组候选均快于对应基线。每次协调仍为 5 次申请、4,680 B 累计申请、4,480 B 峰值存活和 0 次 key-width 申请，未以额外内存换取速度。

## 边界与契约

该路径属于 UI System 的协调 Module。`RenderHandlerTable` 是 `WidgetTree` 唯一拥有的节点 renderer sidecar，以 `WidgetId` 维护 Select、VirtualScroll 及可选 Table renderer 的生命周期。

实现保持以下契约：

- 某张表已空时，当前节点不可能是其 owner，可跳过哈希与删除；
- 表非空时仍以当前 `WidgetId` 执行原有 `remove`，不改变其他 owner 的 renderer；
- renderer 替换仍先清理旧 owner 登记，再按新声明注册；
- 节点移除、同型协调和后续 layout 都不能复活已清理的闭包或动态子树；
- 没有改变 renderer 实例所有权、动态捕获能力、错误语义或关闭顺序。

## 基线与配对方法

基线与候选分别用不同 `CARGO_TARGET_DIR` 构建，但使用完全相同的 release 参数：

```bash
CARGO_PROFILE_RELEASE_STRIP=none \
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
RUSTFLAGS='-C force-frame-pointers=yes' \
rtk cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness --no-run
```

两个独立二进制按 `B,A,A,B,B,A,A,B,B,A` 顺序交替运行，降低编译和跨时段环境漂移对结论的影响。每次命令均只命中一项：

```bash
<binary> --exact stable_keyed_reconcile_profile --nocapture
```

## 候选结果

| 配对 | 基线 B | 候选 A | A 相对 B |
|---|---:|---:|---:|
| 1 | 468,294.88 ns | 439,540.92 ns | -6.14% |
| 2 | 497,851.96 ns | 453,507.54 ns | -8.91% |
| 3 | 462,385.46 ns | 444,512.75 ns | -3.87% |
| 4 | 498,995.38 ns | 444,201.75 ns | -10.98% |
| 5 | 460,795.79 ns | 454,071.54 ns | -1.46% |
| 总体中位 | 468,294.88 ns | 444,512.75 ns | -5.08% |

候选后 `perf` 命中 1 项并获得 535 个 CPU 样本。`RandomState::hash_one<WidgetId>` 从基线的 3.47% 降至 1.51%；剩余主要调用链已转为独立的 `focus_handles` 清理路径。这与五组 A/B 的改善方向一致，支持收益归因；本阶段不继续修改焦点 sidecar。

## 否决的候选

本轮同时用实测排除了以下结构性猜测，避免后续在没有新证据时重复：

- 空结构 State 绑定快路的样本区间与撤回版重叠，未形成可归因收益，已撤回；
- 用自定义分块摘要整体替换 SipHash 的三进程中位慢 1.66%，已撤回；
- 唯一性位图的未扩散草图因低位聚集慢 57.41%；加终结扩散后，五组配对仍慢 2.25%，已撤回；
- 焦点空表移除候选虽显示 0.78% 小幅改善，但哈希热点从 3.47% 到 3.46% 基本不变，无法归因，已撤回。

## 验证

```bash
rtk proxy rustfmt --edition 2024 --check \
  src/ui/coordination/render_handler.rs \
  tests/render_handler_cleanup_contract.rs
rtk cargo test --test render_handler_cleanup_contract --features test-harness
rtk proxy cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness -- --exact stable_keyed_reconcile_profile --nocapture
rtk git diff --check
```

格式检查通过；VirtualScroll renderer 注册、替换、清理和 layout 后不复活契约实际命中并通过 1 项；配对 profile 的 10 次运行均实际命中并通过 1 项；差异检查通过，没有新增警告。
