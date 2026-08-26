# UIX-PERF-009 空焦点句柄 sidecar 清理快路

## 结论

稳定 keyed 协调在完成空渲染 sidecar 快路后，剩余可隔离哈希调用指向 `WidgetTree::set_focus_handle`：普通节点没有焦点句柄声明，但每轮协调仍会先查询、再删除全局空的 `focus_handles`。本轮在输入为 `None` 且整张表为空时直接返回；表非空或输入存在句柄时仍执行原有绑定、复用、替换与解绑流程。

在 512 个稳定 keyed 同级节点、每轮 24 次完整协调的 release 场景中，五组交替配对 A/B 的总体中位耗时从 451,929.17 ns/次降至 435,404.42 ns/次，改善约 3.66%。三组候选更快、一组基本持平、一组慢 1.97%，因此结论采用总体中位与因果采样共同约束，不把单组波动解释为收益。每次协调仍为 5 次申请、4,680 B 累计申请、4,480 B 峰值存活和 0 次 key-width 申请，未以额外内存换取速度。

## 边界与契约

该路径属于 UI System 的协调 Module。`WidgetTree` 唯一拥有 `focus_handles` sidecar，`WidgetId` 是节点登记键，`FocusHandle` 负责把节点绑定到当前 `AppState`，并在替换或清除时解除旧绑定。

实现保持以下契约：

- 输入为 `None` 且整张表为空时，当前节点不可能拥有登记，可跳过两次带哈希寻址；
- 表非空时仍查询当前 `WidgetId`，不会因其他节点存在句柄而误跳过清理；
- 相同句柄继续保持原绑定，不重复解绑与绑定；
- 替换句柄时仍先解绑旧实例，再绑定并登记新实例；
- 清除句柄时仍移除登记并使旧实例返回 `Unbound`；
- 没有改变 `WidgetTree` 的实例所有权、`AppState` 生命周期或焦点请求语义。

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
| 1 | 451,929.17 ns | 429,738.38 ns | -4.91% |
| 2 | 438,261.46 ns | 446,884.29 ns | +1.97% |
| 3 | 464,633.83 ns | 424,943.17 ns | -8.54% |
| 4 | 474,852.58 ns | 474,871.38 ns | +0.00% |
| 5 | 444,014.33 ns | 435,404.42 ns | -1.94% |
| 总体中位 | 451,929.17 ns | 435,404.42 ns | -3.66% |

基线和候选分别采集到 535 与 514 个 CPU 样本，均无丢失样本。逐栈检查中，`set_focus_handle` 从基线 13 次降至候选 0 次，关联 `RandomState::hash_one` 栈从 25 次降至 13 次；候选中剩余哈希来自其他映射。热点调用链的消失与配对中位改善方向一致，支持收益归因。

上一阶段曾在渲染 sidecar 尚未清理的基线上试探焦点空表候选，当时仅改善 0.78%，且哈希热点不变，因无法归因而撤回。本轮是在渲染热点移除后由新的调用链证据重新立项，并用独立构建、五组配对和逐栈计数重新验收，不复用旧结论。

## 验证

```bash
rtk proxy rustfmt --edition 2024 --check \
  src/ui/widget_runtime/widget/tree_core/focus.rs \
  tests/focus_handle_cleanup_contract.rs
rtk cargo test --test focus_handle_cleanup_contract --features test-harness
rtk proxy cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness -- --exact stable_keyed_reconcile_profile --nocapture
rtk git diff --check
```

焦点句柄的初始绑定、相同实例复用、替换和清除契约均由公开 API 覆盖。该热点是安全 Rust 的冗余控制流与哈希寻址，不是指令级瓶颈；没有证据支持内联汇编，本轮不引入汇编。
