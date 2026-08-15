# diagnostics 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 platform 系统的目标 `diagnostics` 模块，权威持有 runtime-scoped 错误观察与恢复协调。依赖：[core/error](../core/error.md)。导出：`uix::diagnostics`。

> **当前实现线索**：公开实现位于 `src/diagnostics/`；core 的历史诊断机制不定义目标公开边界。

## 框架定位

diagnostics 是 UIX 框架稳定运行的基石——整个框架的运行保障子系统，不只是 platform 的局部工具：它是产品原则「容错可观测——出错有类型，恢复有路径，崩溃有报告」（[定位与原则](../../产品/定位与原则.md)）的架构载体，使用层公开入口见[运行保障](../../使用/框架设施/运行保障.md)。框架与应用宿主共用同一个 `uix::diagnostics` 公开面，承担错误收集、错误处理与稳定运行保障：

- **错误收集**：typed `Error` 保留错误类别、来源与责任边界；panic/崩溃由绑定 runtime 的 panic hook 捕获为有界 `CrashReport`；callback/worker 失败经 pending failure queue 投递到 owner-thread，错误发生点不执行 tracing、报告、恢复或用户代码。
- **错误处理**：精确 `Errc` 恢复登记与协调、有界脱敏报告、结构化日志（tracing）与按 `ReportId` 排序的时点快照，供宿主展示或持久化。
- **框架与宿主共用**：同一公开面服务框架自身（图形后端、窗口/输入/TSF 等失败分支）与应用宿主（业务 typed `Error`、日志 subscriber、报告配置与崩溃目录）。
- **稳定运行保障**：崩溃只做最小安全记录、不吞 panic、不在损坏状态下继续运行复杂 UI；失败隔离并继续，错误不在中间层被吞掉或转换成伪成功；错误风暴不能无限增长内存。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `Diagnostics` | cloneable struct | 单 runtime 报告存储与恢复登记 |
| `DiagnosticsConfig` / `BacktracePolicy` | struct/enum | 容量、紧急目录和 backtrace 策略 |
| `ErrorReport` / `ReportId` | struct/value | 有界、脱敏的不可变观察 |
| `DiagnosticsSnapshot` | struct | 按 ReportId 排序的时点快照 |
| `RecoveryAction` / `RecoveryOutcome` | enum | 恢复处理器动作与结果 |
| `RecoverySubscription` | RAII struct | 精确 Errc 处理器登记 |
| `CrashReport` | private struct | 有界、脱敏的进程 panic 快照（私有 crash Module） |
| `PendingFailureQueue` / `PendingFailureSource` | private queue/source | callback 到 owner-thread 的固定容量 typed failure 投递 |

## SMC 落地边界

Diagnostics 的首个 Rust SMC 纵切已经把公开 System 契约与私有实现边界落到源码：

| SMC 角色 | 源码落点 | 所有权与职责 |
|---|---|---|
| System | `src/diagnostics/mod.rs` 的 `Diagnostics` | 对外提供报告、快照和恢复登记/尝试；持有 runtime id、配置，并编排三个私有 Module |
| 私有 Module | `src/diagnostics/reporting.rs` 的 `ReportingModule` | 由一个 Diagnostics 实例拥有；编排报告构建、容量存储和结构化事件发射，不向外暴露实现 |
| 私有 Module | `src/diagnostics/recovery.rs` 的 `RecoveryModule` | 由同一 Diagnostics 实例拥有；持有精确 `Errc` handler 和 RAII 注册生命周期，不访问 reporting Module |
| 私有 Module | `src/diagnostics/crash.rs` 的 crash Module | 由同一 Diagnostics 实例拥有；持有有界 `CrashReport` 模型、原子写入与框架 panic hook，不访问 reporting / recovery Module |
| Component | `reporting.rs` 的 `ReportDraftBuilder` | 单一负责 typed Error 的预算、脱敏、cause 截断和按策略捕获 backtrace |
| Component | `reporting/store.rs` 的 `ReportStore` | 单一负责 ReportId 顺序、固定容量保留和淘汰计数 |
| Component | `reporting/emit.rs` 的 emission guard/event emitter | 单一负责 tracing 结构化事件、递归抑制和 subscriber panic 隔离 |
| Component | `recovery.rs` 的 `RecoveryGuard` | 单一负责同线程递归恢复保护；不拥有 handler 或 System 状态 |
| Component | `crash.rs` 的原子写入与 panic hook | 单一负责 tmp + fsync + rename 原子落盘、失败清理与 hook 递归 guard；不写敏感值，不吞 panic |
| Component | `pending.rs` 的 `PendingFailureQueue` / `PendingFailureSource` | 单一负责 callback 失败的固定容量入队、按 source 隔离、溢出信号和 teardown 关闭；不调用 subscriber、handler 或用户代码 |

System 只通过 `ReportingModule::{report,snapshot}` 与 `RecoveryModule::{register,attempt}`
协作（crash Module 由 `Diagnostics::install_panic_hook` 编排，不参与报告/恢复流程）；三个 Module 不互相引用、查找或持有实例。`report.rs` 的报告值和
`config.rs` 的配置属于 Diagnostics System 契约，不是额外的 Module。最终应用的
`AppRuntime` 持有 Diagnostics System 实例，`AppHandle` 只取得可 clone 的公开句柄。

## 组件：Diagnostics

`report` 只在最终责任边界把 typed Error 转成报告；`on_error` 按精确 Errc 登记，`attempt_recovery` 在调用方选定的安全 owner thread 同步尝试。未处理结果保留原 Error。

## 公开契约

`uix::diagnostics` 的公开契约（用法示例见[运行保障](../../使用/框架设施/运行保障.md)）：

| 入口 | 签名要点 | 语义 |
|---|---|---|
| `Diagnostics::new` | `new(config: DiagnosticsConfig) -> Diagnostics` | 创建单 runtime 报告存储与恢复登记 |
| `report` | `report(&self, error: Error) -> ReportId` | 在最终责任边界把 typed Error 转成有界、脱敏报告 |
| `snapshot` | `snapshot(&self) -> DiagnosticsSnapshot` | 按 ReportId 排序的时点快照 |
| `on_error` | `on_error(&self, code: Errc, handler: F) -> RecoverySubscription` | 按精确 Errc 登记恢复 handler；RAII 句柄释放即注销 |
| `attempt_recovery` | `attempt_recovery(&self, error: Error) -> RecoveryOutcome` | 在调用方选定的安全 owner thread 同步尝试；未处理结果保留原 Error |

crate 内编排入口（不属公开 API，由 app 组装层调用）：

| 入口 | 位置 | 语义 |
|---|---|---|
| `Diagnostics::install_panic_hook` | `src/diagnostics/mod.rs` | 绑定 runtime 的 panic hook；`App::run` 在配置加载前安装 |
| `drain_platform_pending_failures` | `src/app/application/application/runtime/mod.rs` | App owner-thread 任务边界取出 callback/worker 失败，先恢复后报告 |

## Callback → owner-thread 边界

`Diagnostics` 为一个 runtime 持有共享的固定容量 pending failure queue；每个拥有异步 callback 的 graphics context 取得独立 `PendingFailureSource`。callback 只构造并入队 typed `Error`，不执行 tracing、报告、恢复 handler 或用户代码。owner-thread 在资源自己的 `ensure_active` 边界取出该 source 的错误，再交给既有图形恢复状态机；队列溢出在 owner-thread 转换为一次 `InsufficientResources`，source 关闭会清理晚到 callback，避免污染替换 context。S1-06 的回归证明四个并发 producer 在 128 次尝试下只保留固定 64 个真实失败并产生一次 overflow 信号；关闭旧 generation 后晚到 callback 被拒绝，replacement source 可以重新取得完整容量，且 owner-thread drain 保持 FIFO。

App 的 owner-thread 安全点（`drain_platform_pending_failures`）先对每个取出的失败调用 `attempt_recovery`：注册 handler 返回 `Recovered` 时不再 report（仅 tracing 观察），`Unhandled` / `Failed` 才最终 `report` 一次，符合「不能恢复的才报告」。App 组合根已对 `Errc::GraphicsDeviceLost` 注册真实恢复 handler：handler 只通过共享 `RebuildRequest` 请求窗口引擎在下个帧边界执行既有有界恢复序列（`RecoveryDriver::with_rebuild_request`，请求一次性且不覆盖首个未处理失败），领域恢复算法仍归 graphics；`report` 不自动执行恢复 handler 有契约测试锁定。

当前已落地覆盖包括历史 GPU 后端的 uncaptured error / device-lost / offscreen destroy 与 checked teardown、Windows `wnd_proc` 的 DPI / 窗口尺寸移动 / IMM 失败分支、Windows DWM frame pacer worker 的 `DwmFlush` / `PostMessageW` 失败、Windows custom chrome 的 `WM_NCCALCSIZE` / `WM_NCHITTEST` / `WM_SIZE` Win32/DWM 失败，以及 Windows TSF composition / text-store callback 的事件唤醒、锁回调和借用冲突失败。Windows `wnd_proc` 外层 `catch_unwind` 返回 `DefWindowProcW`；windows-rs `#[implement]` 生成的 TSF COM thunk 由 trait guard 返回 `E_FAIL` 并把 `Errc::PlatformError` 投递到 owner source，因此 unwind 不跨 Win32/COM ABI。自动化测试分别穿过 `ITextStoreACP` 与 `ITfContextOwnerCompositionSink` 的真实生成 vtable，锁定两组接口的 panic 转换与单次 owner failure 投递。Windows platform 通过 `Platform::take_pending_failure` 在 App owner-thread 任务边界把 callback/worker failure 交给 Diagnostics；custom chrome 与 TSF 不直接执行 tracing、report、subscriber、recovery 或应用用户代码，同一 `WM_SIZE` 最多保留一个 chrome failure。TSF composition callback 只做状态转换并写入共享队列，`PostMessageW` 失败保留 typed cause；真实 Wayland compositor、macOS AppKit 与其他图形后端 teardown 仍属于后续平台队列批次。

## 组件：ErrorReport

报告限制总字节、分段、cause 数和 metadata，清理控制字符并按策略捕获 backtrace；达到容量淘汰旧报告，错误风暴不能无限增长内存。

## 组件：CrashReport 与 panic hook

`CrashReport` 字段有界（线程 256B、消息 2048B、位置 512B），渲染为行式 key=value 并替换控制字符，非字符串 panic payload 以固定标记代替，不写敏感值。`App::run` 在配置加载前安装绑定 runtime 的 panic hook：配置 `crash_report_directory` 时原子写入（唯一 tmp + fsync + rename，冲突 replace，失败清理），未配置时行为等同默认；hook 始终转发 previous hook 保持默认输出与 unwind/abort 语义（不吞 panic），`PanicHookGuard` 防止 hook 内递归。

## 模块不变量

不内建通用 retry、GPU 重建、业务补偿、远程上传或第二套日志系统；结构化事件经 tracing，具体恢复归资源所属模块。
