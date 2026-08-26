# UIX-PERF-011 声明样式原位借用评估

## 结论

稳定 keyed 协调的指令采样显示，`ViewAdapter::reconcile_existing` 仍是主要本地热点。编译器布局证实，当前 `ViewNode` 为 856 B、对齐 8 B，其中内联 `Style` 为 320 B、对齐 8 B；函数反汇编同时出现约 5.2 KiB 栈帧和一段 0x140 B 样式搬运。因此本轮验证了一个严格局部候选：解构 `ViewNode` 时不再把 `style` 绑定到独立局部值，而是让它留在传入节点的存储位置，并从该字段借用样式完成可见性判断和 `apply_style`。

候选没有形成可复现收益。五组交替配对中有四组候选更慢；总体中位数虽从 380,485.21 ns/次降至 377,400.67 ns/次，表面改善 0.81%，但配对方向不一致，无法排除调度与频率噪声。候选已精确回退，主线不保留代码变化。

这也否决了“只改变 Rust 源码中的绑定位置即可消除机器码搬运”的假设。`ViewNode` 的传值 ABI、字段生命周期和后续大量协调状态共同决定栈布局，不能依据内联调试栈或单段源码形态继续微调。

## 边界与契约

该路径属于 UI System 的协调 Module。候选仅改变 `ViewAdapter` 对声明节点样式字段的局部所有权表达，不改变 `ViewNode`、`Style`、运行时 Widget、可见性切换或样式应用契约；也没有引入新的堆分配。

本轮明确不采用把默认样式改为 `Option<Box<Style>>`、`Arc<Style>` 或全局默认对象的方案。此类方案会让带样式声明在首次修改时新增堆分配，并改变声明层与运行时样式之间的所有权成本；当前基准只覆盖默认样式节点，无法证明它们对真实带样式界面有净收益。

## 基线与配对方法

基线固定为提交 `067c0d8660a8dac165d7c790e9d9827f7d9c1c13`，候选使用独立 `CARGO_TARGET_DIR`，构建参数与上一阶段一致：

```bash
CARGO_PROFILE_RELEASE_STRIP=none \
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
RUSTFLAGS='-C force-frame-pointers=yes' \
rtk cargo test --release --test keyed_reconcile_allocation_contract \
  --features test-harness --no-run
```

两个二进制按 `B,A,A,B,B,A,A,B,B,A` 顺序交替运行，每次只命中 `stable_keyed_reconcile_profile` 一项。

## 候选结果

| 配对 | 基线 B | 候选 A | A 相对 B |
|---|---:|---:|---:|
| 1 | 389,840.25 ns | 405,636.96 ns | +4.05% |
| 2 | 368,990.75 ns | 374,348.67 ns | +1.45% |
| 3 | 384,468.92 ns | 373,039.17 ns | -2.97% |
| 4 | 369,295.46 ns | 377,400.67 ns | +2.20% |
| 5 | 380,485.21 ns | 381,495.42 ns | +0.27% |
| 总体中位 | 380,485.21 ns | 377,400.67 ns | -0.81% |

基线与候选每次协调都保持 5 次申请、4,680 B 累计申请、4,480 B 峰值存活、0 次 key-width 申请和 1 个保留 Layout 根；资源契约没有变化。

## 验证

编译器类型布局通过以下定向构建获取，工具链为 `rustc 1.97.1 (LLVM 22.1.6)`：

```bash
RUSTC_BOOTSTRAP=1 RUSTFLAGS='-Zprint-type-sizes' \
rtk cargo test --test keyed_reconcile_allocation_contract \
  --features test-harness --no-run
```

候选 release 性能二进制构建通过；五组资源断言和性能契约全部各命中 1 项并通过。回退后 `git diff --check` 通过，工作区恢复到仅包含本评估文档。

本轮没有证据表明热点受限于可由手写指令改善的算术或 SIMD 内核，因此不引入内联汇编。
