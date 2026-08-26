# UIX-PERF-003 跨编译场景热点再评估

## 结论

本轮不保留源码优化。三类代表场景的主要热点不同：稳定缓存命中场景由公开拥有型 `CheckOutput` 深拷贝主导；持续变化场景由 lowering 与 View 代码生成主导。唯一同时具备高占比和局部可改性的候选虽然显著降低变化场景的分配与峰值，但让 `change-single` 两轮耗时分别回退 2.99% 和 3.62%，不满足跨场景净收益标准，因此已完整撤销。最终只提交本文档作为证据化停止结论。

## 场景与 SMC 边界

从现有 `compiler_session_profile` 支持的模式选取三类 500 节点场景，均预热一次后测量 200 轮：

- `single`：根 overlay 不变，验证完整源码图与流水线缓存命中；
- `change-single`：根 overlay 在两份等长源码间切换，验证单文件解析、分析与 lowering 重算；
- `change-multi`：依赖 overlay 在两份等长源码间切换，验证 import 图、依赖合并、分析与 lowering 重算。

Compiler System 的公开 `CompilerSession` 与拥有型 `CheckOutput` 是 System 契约；解析、lowering 和 View codegen 是私有 Modules/Components。评估只允许调整私有 View codegen 的重复所有权，不把公开返回值改成借用或 `Arc`，也不改变缓存生命周期。

## 基线

标准 release 命令：

```bash
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile
rtk proxy ./target/release/examples/compiler_session_profile 200 500 single
rtk proxy ./target/release/examples/compiler_session_profile 200 500 change-single
rtk proxy ./target/release/examples/compiler_session_profile 200 500 change-multi
```

每个场景取 5 个独立进程样本。分配次数、分配字节和峰值在同一场景的全部样本中完全一致：

| 场景 | 耗时中位数 | 分配次数/轮 | 分配字节/轮 | 峰值存活增量 |
|---|---:|---:|---:|---:|
| `single` | 186,159 ns | 5,521 | 1,214,829 B | 1,214,745 B |
| `change-single` | 4,399,384 ns | 89,753 | 17,707,930 B | 4,449,248 B |
| `change-multi` | 4,690,971 ns | 96,005 | 19,527,520 B | 5,502,783 B |

耗时范围分别为 183,623–187,941 ns、4,358,807–4,425,222 ns 和 4,657,497–4,757,420 ns；同阶段重复稳定，但跨阶段机器负载仍可能变化，因此收益判定只采用后续交替 A/B。

## 剖析命令与归因

使用 jemalloc 累计分配档案让 200 次测量压过一次预热；`jeprof` 的比例是含调用下游的累计路径，路径之间会重叠，不能相加：

```bash
CARGO_TARGET_DIR=/tmp/uix-perf-003-prof-target \
CARGO_PROFILE_RELEASE_STRIP=none \
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
RUSTFLAGS='-C force-frame-pointers=yes' \
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile

MALLOC_CONF='prof:true,prof_active:true,prof_accum:true,prof_final:true,lg_prof_sample:12,prof_prefix=/tmp/uix-perf-003-accum/heap' \
LD_PRELOAD=/usr/lib/libjemalloc.so \
rtk proxy /tmp/uix-perf-003-prof-target/release/examples/compiler_session_profile 200 500 change-multi

rtk proxy jeprof --text --show_bytes --alloc_space --nodecount=500 \
  /tmp/uix-perf-003-prof-target/release/examples/compiler_session_profile \
  /tmp/uix-perf-003-accum/heap.<pid>.0.f.heap
```

三个场景分别写入独立目录并执行同一命令。剖析器自身的 `malloc_stats_print` 占比不用于候选判断。主要应用路径为：

| 场景 | 采样累计总量 | 主要路径 | 累计字节 / 总量占比 |
|---|---:|---|---:|
| `single` | 307,697,602 B | `materialize_check_output` | 286,332,447 B / 93.1% |
| `change-single` | 4,491,748,685 B | `lower_rust_plan` | 3,225,099,810 B / 71.8% |
|  |  | `generate_document_view_owned` | 2,185,831,151 B / 48.7% |
|  |  | `materialize_check_output` | 285,322,233 B / 6.4% |
| `change-multi` | 4,919,298,696 B | `lower_rust_plan` | 2,985,243,168 B / 60.7% |
|  |  | `generate_document_view_owned` | 2,680,778,466 B / 54.5% |
|  |  | `load_unit` | 1,504,443,834 B / 30.6% |
|  |  | `materialize_check_output` | 287,619,941 B / 5.8% |

`single` 每轮只命中缓存，但公开返回值必须拥有 `tracked_files`、`SourceGraph` 和 `TypedUiIr`，所以 `materialize_check_output` 会深拷贝分析结果。把这些公开字段改为共享句柄会改变 System API 与调用方所有权，超出本轮低风险边界。

变化场景的最大私有候选位于 `generate_view_for_source`：它为删除根上的内部 `SOURCE_ID_ATTRIBUTE` 深拷贝整棵 `Element`；而 `apply_common_attributes` 已显式跳过该内部属性。实验候选直接借用根元素，保持诊断、SourceId 与生成结果契约不变。

## 两轮交替 A/B 与否决结果

基线 A 使用提交 `43e24792` 的独立 worktree，候选 B 只移除上述根 `Element` 深拷贝。每个场景按 A1/B1/A2/B2 交替，每阶段取 3 个独立进程样本。下表耗时为全部 6 个样本的合并中位数：

| 场景 | 指标 | 基线 A | 候选 B | 变化 |
|---|---|---:|---:|---:|
| `single` | 耗时 | 191,220 ns | 185,543.5 ns | -2.97% |
|  | 分配次数 | 5,521 | 5,521 | 不变 |
|  | 分配字节 | 1,214,829 B | 1,214,829 B | 不变 |
|  | 峰值存活 | 1,214,745 B | 1,214,745 B | 不变 |
| `change-single` | 耗时 | 4,539,078.5 ns | 4,797,736.5 ns | **+5.70%** |
|  | 分配次数 | 89,753 | 81,245 | -9.48% |
|  | 分配字节 | 17,707,930 B | 16,163,683 B | -8.72% |
|  | 峰值存活 | 4,449,248 B | 3,452,629 B | -22.40% |
| `change-multi` | 耗时 | 4,993,156.5 ns | 4,994,510.5 ns | +0.03% |
|  | 分配次数 | 96,005 | 82,984 | -13.56% |
|  | 分配字节 | 19,527,520 B | 16,933,140 B | -13.29% |
|  | 峰值存活 | 5,502,783 B | 4,504,320 B | -18.14% |

各轮耗时中位数：

- `single`：192,290 → 185,505 ns（-3.53%）；190,150 → 185,582 ns（-2.40%）。候选不在稳定测量循环中执行且资源完全不变，该差异只能视为二进制布局或机器噪声，不能归为收益。
- `change-single`：4,412,889 → 4,544,624 ns（+2.99%）；4,662,558 → 4,831,261 ns（+3.62%）。两轮同向回退，否决候选。
- `change-multi`：4,992,093 → 4,990,129 ns（-0.04%）；4,994,220 → 5,034,585 ns（+0.81%）。方向不一致，属于噪声内且没有耗时净收益。

资源收益可重复，但不能抵消 `change-single` 的稳定耗时回退；候选已撤销，最终源码与 `43e24792` 一致。剩余主要热点要么是公开拥有型返回契约，要么分散在 parser、import 合并与令牌构造，当前没有同时满足高占比、低风险和跨场景净收益的下一候选，因此停止。

## 验证

```bash
rtk proxy cargo test -p uix-lang-compiler compiler_session::tests::check_and_compile_share_analysis_and_lowering_stages -- --exact
rtk proxy cargo test -p uix-lang-compiler compiler_session::tests::dependency_overlay_reparses_only_the_changed_file -- --exact
rtk proxy cargo test -p uix-lang-compiler source_map::tests::source_map_resolves_marker_to_attribute_semantic_node -- --exact
rtk proxy cargo test -p uix-lang-compiler tests::source_map_preserves_recursive_import_sources -- --exact
rtk cargo test -p uix-lang-compiler
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile
rtk git diff --check
```

四个定向测试各实际命中并通过 1 项；完整包测试命中并通过 527 项（2 个 suite）。最终 release 构建与差异检查通过，没有新增警告。
