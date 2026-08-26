# UIX-PERF-002 峰值存活内存优化证据

## 场景与边界

- 基线：先 `fetch`，再以普通 merge 把 `origin/main` 合入已推送分支 `codex/uix-perf-001`，合并提交为 `e0d0acd6`，没有改写历史或产生冲突。
- 场景：沿用 `CompilerSession change-multi`，入口文件导入一个包含 500 个 `Text` 节点的组件；预热后执行 200 次等长依赖 overlay 切换。
- 测量：计数分配器同时记录耗时、累计分配次数、累计分配字节和测量窗口内峰值存活字节增量。
- SMC 边界：Compiler System 的公开 `CompilerSession` 契约不变；只调整 Widget lowering Component 私有的 emission document 所有权交接。借用型生成入口仍保留原有按需复制语义。
- 停止边界：只处理内存剖析占主导的 AST 同时保留问题，不继续调整 parser、样式解析或令牌生成的其他分散样本。

## 可复核命令

更新与标准 release 基准：

```bash
rtk git fetch origin --prune
rtk git merge --no-ff origin/main -m '合并 origin/main 后续性能提交'
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile
rtk proxy ./target/release/examples/compiler_session_profile 200 500 change-multi
```

使用 jemalloc 高水位转储记录对象存活组成；保留行表并强制帧指针以归因 Rust 调用路径：

```bash
CARGO_TARGET_DIR=/tmp/uix-perf-002-prof-target \
CARGO_PROFILE_RELEASE_STRIP=none \
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
RUSTFLAGS='-C force-frame-pointers=yes' \
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile

MALLOC_CONF='prof:true,prof_active:true,prof_gdump:true,lg_prof_sample:12,prof_prefix:/tmp/uix-perf-002-heap-profile/heap' \
LD_PRELOAD=/usr/lib/libjemalloc.so \
rtk proxy /tmp/uix-perf-002-prof-target/release/examples/compiler_session_profile 1 500 change-multi

rtk proxy jeprof --text --show_bytes --nodecount=50 \
  /tmp/uix-perf-002-prof-target/release/examples/compiler_session_profile \
  /tmp/uix-perf-002-heap-profile/heap.802884.1451.u1451.heap
```

剖析运行仍报告与标准基线一致的 `100,516` 次分配、`20,578,427 B` 分配字节和 `6,552,922 B` 峰值增量，说明采样命中了同一 workload。

## 剖析归因

1731 份按高水位生成的转储中，序号 1451 是采样存活字节最高的一份。`jeprof` 估计总存活为 `10,237,267 B`；其中 `malloc_stats_print` 自身占 `1,339,355 B`（13.1%），归因时排除。应用侧分配路径占 `8,533,613 B`（83.4%），主要累计调用路径为：

- `lower_analyzed` / `lower_rust_plan`：`7,297,697 B`（总样本 71.3%）；
- `generate_document_view`：`5,905,488 B`（57.7%）；
- AST `clone` / `to_vec`：`6,763,656 B`（66.1%）；
- `analyze_file`：`2,710,192 B`（26.5%）；
- `load_unit`：`2,235,327 B`（21.8%）。

CodeGraph 与定向调用链复核表明：`lower_rust_plan` 已独占 `ir.emission_document()` 返回的 `Document`，但旧路径随后借用它构造 `WidgetExpander`，把全部 Widget、Record 和 Keyframes 声明深拷贝到注册表；原 emission document 与这些副本在展开期间同时存活。该保留路径属于可控的 lowering 私有所有权，而不是测试夹具、外部依赖或必须跨阶段保留的缓存对象。

## 变更

新增拥有型 `generate_document_view_owned` 入口。`CompilerSession` 的 View lowering 把独占 emission document 直接交给该入口；样式 resolver 完成读取后，通过 `std::mem::take` 把 Widget、Record 和 Keyframes 声明移动到 `WidgetExpander` 注册表，不再深拷贝声明。App codegen 和测试使用的借用型 `generate_document_view` 保留旧的按需复制实现，避免旁路性能回退。

## 两轮交替 A/B

基线使用合并提交 `e0d0acd6` 的独立临时 worktree，优化组使用最终工作树；按 A1/B1/A2/B2 交替运行，每阶段执行 3 个独立进程样本。表中耗时是全部 6 个样本的合并中位数，其他三项在各组全部样本内完全一致：

| 指标 | 合并后基线 | 优化后 | 变化 |
|---|---:|---:|---:|
| 合并耗时中位数 | 4,885,323.5 ns/轮 | 4,701,810 ns/轮 | -3.76% |
| 分配次数 | 100,516 次/轮 | 96,005 次/轮 | -4.49% |
| 分配字节 | 20,578,427 B/轮 | 19,527,520 B/轮 | -5.11% |
| 峰值存活字节增量 | 6,552,922 B | 5,502,783 B | -16.03% |

两轮各自的耗时中位数均改善：

- 交替轮 1：4,861,021 → 4,706,064 ns/轮（-3.19%）；
- 交替轮 2：4,932,584 → 4,697,556 ns/轮（-4.76%）。

## 功能验证

```bash
rtk proxy cargo test -p uix-lang-compiler -- --list
rtk proxy cargo test -p uix-lang-compiler compiler_session::tests::check_and_compile_share_analysis_and_lowering_stages -- --exact
rtk proxy cargo test -p uix-lang-compiler compiler_session::tests::dependency_overlay_reparses_only_the_changed_file -- --exact
rtk cargo test -p uix-lang-compiler
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile
rtk git diff --check
```

两个定向测试各实际命中并通过 1 项；完整包测试命中并通过 527 项（2 个 suite）。最终 release 构建和差异检查通过，没有新增警告。
