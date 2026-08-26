# UIX-PERF-001 首轮性能与内存优化证据

## 场景与边界

- 场景：`CompilerSession` 多文件 overlay 持续变更，入口文件导入一个包含 500 个 `Text` 节点的组件，依赖源码在两份等长内容间切换。
- 测量：预热一次后执行 200 次 `check_file_with_overlays`；全局计数分配器记录耗时、分配次数、分配字节和测量窗口内峰值存活字节增量。
- SMC 边界：Compiler System 的公开 `CompilerSession` 契约不变；只调整 Import Resolver Module 私有的不可变 `ResolvedUnit` 缓存所有权。
- 停止边界：只处理运行时采样直接命中的解析单元深拷贝，不继续扫描或调整解析器、codegen 与令牌生成的其他分散样本。

## 可复核命令

标准 release 场景：

```bash
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile
rtk proxy ./target/release/examples/compiler_session_profile 200 500 change-multi
```

保留行表与帧指针后用 GDB 运行时栈采样：

```bash
CARGO_PROFILE_RELEASE_STRIP=none CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
RUSTFLAGS='-C force-frame-pointers=yes' \
rtk cargo build --release -p uix-lang-compiler --example compiler_session_profile
rtk proxy gdb --quiet ./target/release/examples/compiler_session_profile
```

GDB 内执行 `set pagination off`、`set debuginfod enabled off`、`set backtrace limit 30`、`run 5000 500 change-multi`，运行期间重复中断、`bt`、`continue`，共取得 12 个运行时栈样本。

## 剖析归因

12 个样本中至少 7 个落在完整 AST 的克隆或析构路径：

- `ImportResolver::load_unit` 把 `ResolvedUnit` 深拷贝进 resolver 私有缓存；
- `resolve_import` 返回及 `DeclarationMerge::push` 期间出现 `SourcedDeclaration`/`Node` 深拷贝；
- `WidgetCodegen` 与 `lower_rust_plan` 消费后出现对应 AST 析构。

其余样本分散在 parser 和 `proc_macro2` 令牌构造，没有形成更集中的第二热点。CodeGraph 确认 `load_unit` 只有根解析和递归 import 两个调用点，缓存不会越过单次 resolver 生命周期。

## 变更

`ImportResolver::cache` 从拥有 `ResolvedUnit` 副本改为拥有 `Arc<ResolvedUnit>`：缓存命中及写入只复制共享句柄；根单元从私有缓存移除后通过 `Arc::into_inner` 恢复唯一所有权，再按原契约消费成 `ResolvedDocument`。没有跨 System 共享实例，也没有改变公开类型、错误或生命周期语义。

## 前后数据

首轮独立基线的分配数据在 5 次运行中完全一致，但耗时受当时机器负载影响落在 23.79–29.02 ms/轮，因此不用于耗时收益结论。随后在同一机器、同一标准 release 配置下，用基线提交 `f0d7a646` 的临时 worktree 与优化分支进行两轮交替 A/B；每个阶段取 3 次独立进程样本：

| 指标 | 基线 | 优化后 | 变化 |
|---|---:|---:|---:|
| 合并耗时中位数 | 5,165,708 ns/轮 | 4,829,312 ns/轮 | -6.51% |
| 分配次数 | 107,540 次/轮 | 100,516 次/轮 | -6.53% |
| 分配字节 | 22,466,289 B/轮 | 20,578,427 B/轮 | -8.40% |
| 峰值存活字节增量 | 6,552,922 B | 6,552,922 B | 不变 |

两轮各自的耗时中位数分别为：

- A/B 轮 1：5,134,440 → 4,828,077 ns/轮（-5.97%）；
- A/B 轮 2：5,196,655 → 4,830,547 ns/轮（-7.04%）。

分配次数、分配字节和峰值在全部基线样本及全部优化样本内各自完全一致。

## 功能验证

```bash
rtk cargo test -p uix-lang-compiler uix_import_tests::
rtk cargo test -p uix-lang-compiler compiler_session::tests::dependency_overlay_reparses_only_the_changed_file
rtk cargo test -p uix-lang-compiler compiler_session::tests::unchanged_multi_file_overlay_skips_resolver_rebuild
rtk cargo test -p uix-lang-compiler
```

前三项分别命中并通过 8、1、1 项测试；完整包测试命中并通过 527 项（2 个 suite），没有新增警告。
