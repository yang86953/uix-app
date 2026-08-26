# UIX-PERF-004 CheckOutput 兼容所有权优化

## 结论

本轮在不改变公开 API 的前提下完成了 `CheckOutput` 物化热点优化。`CheckOutput`、`SourceGraph`、`TypedUiIr` 的公开类型、字段类型、方法、错误与拥有型生命周期均保持不变；只把后两者构建后不可变的私有大对象改为 `Arc` 共享。两轮三场景交替 A/B 均无回退：`single` 耗时、分配次数、分配字节和峰值分别下降 70.39%、63.56%、79.53% 和 79.54%，两个持续变化场景的耗时与累计分配也同步改善。

## CodeGraph 契约与调用者

CodeGraph 定位的 Compiler System 公开检查契约包括：

- `CompilerSystem::{check_inline, check_file, check_file_with_overlays}` 及 auto 变体；
- 长寿命 Adapter 使用的 `CompilerSession::{check_file, check_file_with_overlays}` 及 auto 变体；
- 所有入口继续返回 `Result<CheckOutput, CompilerDiagnostic>`；`CheckOutput` 的四个公开字段仍是 `Vec<PathBuf>`、`SourceGraph`、`TypedUiIr` 和 `CompilationKey`。

仓库内实际生产调用者是 CLI 与 LSP：CLI 只判断成功，LSP 只取得失败诊断；profile harness 消费完整成功结果。仓库外调用者仍可能读取所有公开字段，因此不能把旧入口改成窄结果或借用结果。

实例所有权链为：

```text
CompilerSession
  └─ CachedPipeline
      └─ Arc<AnalyzedUnit>
          ├─ Vec<PathBuf>
          ├─ SourceGraph
          └─ TypedUiIr
               └─ 私有 emission Document

每次公开检查返回 CheckOutput（独立拥有）
```

旧 `materialize_check_output` 从缓存的 `AnalyzedUnit` 深拷贝 `SourceGraph` 与 `TypedUiIr`。`SourceGraph` 的字段私有，公开方法只返回不可变切片；`TypedUiIr` 的字段同样私有，公开方法只提供不可变查询。两者在构建后均无原地修改入口，因此共享私有存储不会改变公开值语义，也不会泄漏缓存借用或要求 `CompilerSession` 比输出存活更久。

## 最小边界方案比较

| 优先级 | 方案 | 结论 |
|---|---|---|
| 1 | 私有内部快速路径 | CLI/LSP 可新增只返回成功与否的私有入口，但旧公开检查仍需物化完整 `CheckOutput`，不能解决 profile 与仓库外调用者热点。 |
| 2 | 保持旧 API 的共享物化 | 采用。公开类型不变；`SourceGraph` 私有文件/导入序列和 `TypedUiIr` 私有声明/Document 使用拥有型 `Arc`。 |
| 3 | 新增窄查询契约 | 可让调用者选择只取状态或 key，但会新增语义选择、迁移和长期兼容面；方案 2 已满足目标，无需引入。 |

`TypedUiIr::root()` 继续保持原有 `const fn`，公开 `TypedElement` 结构也不变，因此根元素本身仍按值克隆；这是为保留现有公开 const 与值语义而明确留下的兼容成本。

## 当前基线

```bash
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile
rtk proxy ./target/release/examples/compiler_session_profile 200 500 single
rtk proxy ./target/release/examples/compiler_session_profile 200 500 change-single
rtk proxy ./target/release/examples/compiler_session_profile 200 500 change-multi
```

每个场景取 5 个独立进程样本；资源指标在同一场景全部样本中完全一致：

| 场景 | 耗时中位数 | 分配次数/轮 | 分配字节/轮 | 峰值存活增量 |
|---|---:|---:|---:|---:|
| `single` | 188,449 ns | 5,521 | 1,214,829 B | 1,214,745 B |
| `change-single` | 4,786,107 ns | 89,753 | 17,707,930 B | 4,449,248 B |
| `change-multi` | 5,257,435 ns | 96,005 | 19,527,520 B | 5,502,783 B |

## 变更

- `SourceGraph` 的私有 `files` 与 `imports` 从 `Vec<T>` 改为 `Arc<[T]>`；构建器仍消费并移动原始分配，公开 `files()`、`imports()`、`file()` 与 hash 语义不变。
- `TypedUiIr` 的私有 `declarations` 与原始 `Document` 改为 `Arc`；公开 `declarations()`、`root()`、语义查询与 capability 查询不变。lowering 需要可变发射 AST 时仍显式深拷贝 `Document`，不会修改共享分析结果。
- 新增两个模块内测试，直接证明克隆后的公开值相等且私有不可变存储使用同一 `Arc`；没有向公开边界暴露 `Arc` 或借用。

SMC 边界保持为：Compiler System 继续拥有公开检查结果；Source Graph Module 与 Semantic IR Module 只改变各自私有不可变实例的存储策略，CompilerSession 缓存和调用者各自持有强所有权，任一方释放都不影响另一方。

## 两轮交替 A/B

基线 A 使用提交 `e58d7af6` 的独立 worktree，优化 B 使用最终实现。每个场景按 A1/B1/A2/B2 交替，每阶段取 3 个独立进程样本；表中耗时是全部 6 个样本的合并中位数，资源指标在各组全部样本内完全一致：

| 场景 | 指标 | 基线 A | 优化 B | 变化 |
|---|---|---:|---:|---:|
| `single` | 耗时 | 190,146.5 ns | 56,301 ns | -70.39% |
|  | 分配次数 | 5,521 | 2,012 | -63.56% |
|  | 分配字节 | 1,214,829 B | 248,665 B | -79.53% |
|  | 峰值存活 | 1,214,745 B | 248,581 B | -79.54% |
| `change-single` | 耗时 | 4,361,879.5 ns | 4,213,182 ns | -3.41% |
|  | 分配次数 | 89,753 | 86,248 | -3.91% |
|  | 分配字节 | 17,707,930 B | 16,741,878 B | -5.46% |
|  | 峰值存活 | 4,449,248 B | 4,449,248 B | 不变 |
| `change-multi` | 耗时 | 4,706,717.5 ns | 4,518,495 ns | -4.00% |
|  | 分配次数 | 96,005 | 90,483 | -5.75% |
|  | 分配字节 | 19,527,520 B | 18,311,589 B | -6.23% |
|  | 峰值存活 | 5,502,783 B | 5,502,783 B | 不变 |

各轮耗时中位数均同向改善：

- `single`：188,954 → 55,957 ns（-70.39%）；193,173 → 56,597 ns（-70.70%）；
- `change-single`：4,364,866 → 4,253,456 ns（-2.55%）；4,332,096 → 4,203,589 ns（-2.97%）；
- `change-multi`：4,703,126 → 4,486,512 ns（-4.61%）；4,710,309 → 4,544,228 ns（-3.53%）。

## 优化后剖析

沿用 PERF-003 的 jemalloc `prof_accum` / `alloc_space` 方法对优化后的 200 轮 `single` 复核：

```bash
MALLOC_CONF='prof:true,prof_active:true,prof_accum:true,prof_final:true,lg_prof_sample:12,prof_prefix:/tmp/uix-perf-004-accum-single/heap' \
LD_PRELOAD=/usr/lib/libjemalloc.so \
rtk proxy /tmp/uix-perf-004-prof-target/release/examples/compiler_session_profile 200 500 single

rtk proxy jeprof --text --show_bytes --alloc_space --nodecount=500 \
  /tmp/uix-perf-004-prof-target/release/examples/compiler_session_profile \
  /tmp/uix-perf-004-accum-single/heap.<pid>.0.f.heap
```

累计采样总量从 PERF-003 基线的 307,697,602 B 降到 81,412,069 B（-73.54%）；`materialize_check_output` 累计路径从 286,332,447 B 降到 60,102,761 B（-79.01%）。剩余绝对成本主要包含为兼容公开 `TypedElement`/`root() const` 而保留的根值克隆，本轮不继续扩大契约。

## 验证

```bash
rtk proxy cargo test -p uix-lang-compiler source_graph::tests::clone_shares_immutable_source_snapshot_storage -- --exact
rtk proxy cargo test -p uix-lang-compiler semantic_ir::tests::clone_shares_private_immutable_ir_storage -- --exact
rtk proxy cargo test -p uix-lang-compiler compiler_session::tests::unchanged_graph_reuses_parse_analysis_and_check_stages -- --exact
rtk proxy cargo test -p uix-lang-compiler compiler_session::tests::check_and_compile_share_analysis_and_lowering_stages -- --exact
rtk proxy cargo test -p uix-lang-compiler tests::source_map_preserves_recursive_import_sources -- --exact
rtk cargo test -p uix-lang-compiler
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile
rtk git diff --check
```

五个定向测试各实际命中并通过 1 项；完整包测试命中并通过 529 项（2 个 suite）。最终 release 构建与差异检查通过，没有新增警告。
