# UIX-PERF-005 PERF-004 后热点再评估

## 结论

本轮不保留源码优化。PERF-004 后，`single` 的真实成本已经降到每轮 248,665 B、2,012 次分配和约 59.4 μs；其剩余累计分配仍以公开拥有型 `CheckOutput` 的根值物化为主，但绝对成本较低，继续降低需要改变公开所有权契约。

两个变化场景的第一私有热点均为 lowering 内的 View 令牌生成。针对其中占比最高且边界最小的 `generate_child_statements`，实验了直接累积单一 `TokenStream`、避免末尾重放子流的实现。两轮交替 A/B 中，候选虽减少约 1.7% 分配次数，却让 `change-single` 耗时连续回退，并让两个变化场景峰值存活确定性增加，因此已经完整撤销。按“最多处理一个热点”的边界，本轮以证据化停止结论收口。

## 场景、契约与所有权边界

沿用既有 profile harness 的三个 500 节点真实模式，均预热一次后测量 200 轮：

- `single`：根 overlay 不变，测量完整源码图、分析与 lowering 缓存命中；
- `change-single`：根 overlay 在两份等长源码间切换，测量单文件重算；
- `change-multi`：依赖 overlay 在两份等长源码间切换，测量 import 图与多文件重算。

CodeGraph 在当前 `f791d22e` 索引上确认的责任链是：公开 Compiler System 的 `CompilerSession::check_file_with_snapshot` 调用私有 analysis/lowering Module；`lower_rust_plan` 调用私有 Widget codegen 的 `generate_document_view_owned`，再进入普通实现 `generate_children` / `generate_child_statements`。`CompilerSession` 缓存拥有 `Arc<AnalyzedUnit>`，公开 `CheckOutput` 仍按值拥有根 `TypedElement`；SourceGraph 与 IR 的私有不可变切片已由 PERF-004 共享。

因此本轮候选只改普通 View 令牌拼接实现，不改变公开 API、失败语义、SourceId 线程局部上下文、AST 生命周期或依赖方向，也不跨 System 共享私有实例。

## PERF-004 后新基线

标准 release 命令：

```bash
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile
rtk proxy ./target/release/examples/compiler_session_profile 200 500 single
rtk proxy ./target/release/examples/compiler_session_profile 200 500 change-single
rtk proxy ./target/release/examples/compiler_session_profile 200 500 change-multi
```

每个场景取 5 个独立进程样本。分配次数、分配字节和峰值存活在同一场景的全部样本中完全一致：

| 场景 | 耗时中位数（范围） | 分配次数/轮 | 分配字节/轮 | 峰值存活增量 |
|---|---:|---:|---:|---:|
| `single` | 59,449 ns（55,491–68,074） | 2,012 | 248,665 B | 248,581 B |
| `change-single` | 4,212,799 ns（4,144,231–4,226,181） | 86,248 | 16,741,878 B | 4,449,248 B |
| `change-multi` | 4,509,165 ns（4,464,934–4,552,248） | 90,483 | 18,311,589 B | 5,502,783 B |

`single` 的耗时绝对值很小，单个 68 μs 样本对范围影响明显；热点和候选收益不依赖该场景的耗时差，而以后续交替 A/B 与确定性资源指标判断。

## 新剖析与热点归因

重新构建带行表与帧指针的当前提交，并为三个场景写入独立的 jemalloc 累计分配档案：

```bash
CARGO_TARGET_DIR=/tmp/uix-perf-005-prof-target \
CARGO_PROFILE_RELEASE_STRIP=none \
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
RUSTFLAGS='-C force-frame-pointers=yes' \
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile

MALLOC_CONF='prof:true,prof_active:true,prof_accum:true,prof_final:true,lg_prof_sample:12,prof_prefix=/tmp/uix-perf-005-accum-<scenario>/heap' \
LD_PRELOAD=/usr/lib/libjemalloc.so \
rtk proxy /tmp/uix-perf-005-prof-target/release/examples/compiler_session_profile 200 500 <scenario>

rtk proxy /usr/bin/jeprof --text --show_bytes --alloc_space --nodecount=500 \
  /tmp/uix-perf-005-prof-target/release/examples/compiler_session_profile \
  /tmp/uix-perf-005-accum-<scenario>/heap.<pid>.0.f.heap
```

下表是含下游的累计路径，路径会重叠，不能相加；剖析器自身的 `malloc_stats_print` 不用于候选判断：

| 场景 | 采样累计总量 | 主要应用路径 | 累计字节 / 总量占比 |
|---|---:|---|---:|
| `single` | 80,402,149 B | `materialize_check_output` | 59,293,318 B / 73.7% |
|  |  | `lower_rust_plan` | 15,901,261 B / 19.8% |
|  |  | `generate_document_view_owned` | 10,868,390 B / 13.5% |
| `change-single` | 4,265,032,377 B | `lower_rust_plan` | 3,224,639,446 B / 75.6% |
|  |  | `generate_document_view_owned` | 2,186,231,027 B / 51.3% |
|  |  | `generate_child_statements` | 846,865,168 B / 19.9% |
|  |  | `load_unit` | 840,053,987 B / 19.7% |
| `change-multi` | 4,634,876,164 B | `lower_rust_plan` | 2,987,258,749 B / 64.5% |
|  |  | `generate_document_view_owned` | 2,683,632,845 B / 57.9% |
|  |  | `load_unit` | 1,504,613,447 B / 32.5% |
|  |  | `generate_child_statements` | 1,165,497,823 B / 25.1% |

`single` 的第一占比仍是公开结果物化，但每轮绝对分配只有约 0.25 MB；其中 SourceGraph 与 IR 私有大块存储已共享，剩余根值克隆是现有公开拥有型语义的必要成本。变化场景的绝对成本集中于 lowering 与 codegen，`generate_child_statements` 同时命中两场景且是可隔离的第一普通实现候选。

## 两轮交替 A/B 与否决结果

基线 A 是独立 worktree 中的 `f791d22e`；候选 B 只把 `generate_child_statements` 的 `Vec<TokenStream>` 加末尾 `quote!` 重放，替换为生成时直接移动进单一 `TokenStream`。每阶段每场景取 3 个独立进程样本，顺序为 A1/B1/A2/B2。

下表耗时是两轮共 6 个样本的合并中位数，资源指标在同一实现内完全一致：

| 场景 | 指标 | 基线 A | 候选 B | 变化 |
|---|---|---:|---:|---:|
| `single` | 耗时 | 56,986 ns | 53,944 ns | -5.34%（不可归因） |
|  | 分配次数 | 2,012 | 2,012 | 不变 |
|  | 分配字节 | 248,665 B | 248,665 B | 不变 |
|  | 峰值存活 | 248,581 B | 248,581 B | 不变 |
| `change-single` | 耗时 | 4,219,082 ns | 4,271,401.5 ns | **+1.24%** |
|  | 分配次数 | 86,248 | 84,741 | -1.75% |
|  | 分配字节 | 16,741,878 B | 16,685,646 B | -0.34% |
|  | 峰值存活 | 4,449,248 B | 4,496,152 B | **+1.05%** |
| `change-multi` | 耗时 | 4,557,704 ns | 4,489,061.5 ns | -1.51% |
|  | 分配次数 | 90,483 | 88,973 | -1.67% |
|  | 分配字节 | 18,311,589 B | 18,255,147 B | -0.31% |
|  | 峰值存活 | 5,502,783 B | 5,549,727 B | **+0.85%** |

各轮耗时中位数：

- `single`：56,930 → 54,008 ns（-5.13%）；57,042 → 53,880 ns（-5.54%）。候选路径不在稳定缓存命中的测量循环内，且全部资源指标不变，只能视为二进制布局或机器噪声；
- `change-single`：4,229,439 → 4,269,414 ns（+0.95%）；4,202,538 → 4,273,389 ns（+1.69%）。两轮同向回退；
- `change-multi`：4,548,199 → 4,509,257 ns（-0.86%）；4,606,895 → 4,452,752 ns（-3.35%）。

直接累积降低了临时分配次数，但单一流更早、也更久地保留扩容后的存储，造成可重复峰值回退；同时 `change-single` 耗时稳定受损。候选不满足跨场景与峰值门槛，源码已完整恢复到 `f791d22e`。

本轮最多只能处理一个新热点。其余累计成本分散在 proc-macro 令牌构造、parser 与 import 合并，或属于现有公开拥有型结果的必要成本；没有证据支持在当前范围内继续微调。

## 验证

```bash
rtk proxy cargo test -p uix-lang-compiler compiler_session::tests::check_and_compile_share_analysis_and_lowering_stages -- --exact
rtk proxy cargo test -p uix-lang-compiler tests::source_map_preserves_recursive_import_sources -- --exact
rtk cargo test -p uix-lang-compiler
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile
rtk git diff --check
```

两个定向测试各实际命中并通过 1 项；完整包测试命中并通过 529 项（2 个 suite）。最终 release 构建和差异检查通过，没有新增警告。最终仅新增本文档，不保留源码差异。
