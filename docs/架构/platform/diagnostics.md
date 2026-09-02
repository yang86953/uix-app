# diagnostics 系统

[← 返回架构索引](../../架构.md)

> **接口**：声明独立 `diagnostics` System，权威持有 runtime-scoped 错误观察、恢复协调、callback 失败归队与崩溃记录。依赖：[core/error](../core/error.md)。导出：`uix::diagnostics`。

> **当前实现线索**：公开实现位于 `src/diagnostics/`；core 的历史诊断机制不定义目标公开边界。本文档暂留在 `platform/` 路径只是迁移路由，不表示 diagnostics 是 platform 私有 Module。

## 框架定位

diagnostics 是 UIX 框架稳定运行的基石——整个框架的运行保障子系统，不只是 platform 的局部工具：它是产品原则「容错可观测——出错有类型，恢复有路径，崩溃有报告」（[定位与原则](../../产品/定位与原则.md)）的架构载体，使用层公开入口见[运行保障](../../使用/框架设施/运行保障.md)。框架与应用宿主共用同一个 `uix::diagnostics` 公开面，承担错误收集、错误处理与稳定运行保障：

- **错误收集**：typed `Error` 保留错误类别、来源与责任边界；panic/崩溃由绑定 runtime 的 panic hook 捕获为有界 `CrashReport`；callback/worker 失败经 pending failure queue 投递到 owner-thread，错误发生点不执行 tracing、报告、恢复或用户代码。
- **错误处理**：精确 `Errc` 恢复登记与协调、有界脱敏报告、统一调试模式、结构化日志（tracing）、复现清单与按 `ReportId` 排序的时点快照，供宿主展示或持久化。
- **框架与宿主共用**：同一公开面服务框架自身（图形后端、窗口/输入/TSF 等失败分支）与应用宿主（业务 typed `Error`、日志 subscriber、报告配置与崩溃目录）。
- **稳定运行保障**：崩溃只做最小安全记录、不吞 panic、不在损坏状态下继续运行复杂 UI；失败隔离并继续，错误不在中间层被吞掉或转换成伪成功；错误风暴不能无限增长内存。

## 错误处置决策矩阵

任何框架错误处理点必须按下表显式选择处置类别；判断依据是错误的**终态性、频率与恢复所有者**，而不是位置直觉。类型化入口让选择成为编译期可见的调用事实：

| 判定问题 | 处置类别 | 类型化入口 | 可观测结果 |
|---|---|---|---|
| 错误是否到达最终责任边界且无恢复可能（启动终止、恢复放弃、清理失败）？ | **终态报告** | `Diagnostics::report_with_origin` + `ReportOrigin::framework`（crate 内） | 进入有界报告存储与 `snapshot()`，tracing 结构化事件同步发射 |
| 错误是否发生在跨线程回调/worker 边界？ | **回调投递** | `PendingFailureSource::enqueue`（crate 内） | owner-thread drain 边界先恢复后报告 |
| 错误是否有恢复所有者（RecoveryDriver FSM、注册的恢复 handler）？ | **恢复** | `attempt_recovery` / 恢复状态机 | 恢复成功不报告；放弃时转终态报告 |
| 错误是自愈重试、fallback 保持武装或高频平台噪声？ | **瞬态观察** | `Diagnostics::observe_transient(target, reason, detail)` | 冷却去重的结构化事件（30 秒窗口 + 抑制计数），不进报告存储 |
| 所在层按 SMC 边界不持有 Diagnostics 句柄（UI 树、adapter teardown）？ | **边界观察** | `diagnostics::observe_boundary_error(layer, error)`（crate 内） | 带层标识的结构化日志 |
| 是否为开发者契约或内部不变量破坏（文档声明的组装期/所有权契约）？ | **契约 panic** | `uix_contract_violation!` 宏（或带文档声明的 panic） | crash hook 捕获为 CrashReport + 复现清单，不吞 panic |

判定顺序自上而下：先问终态性，再问回调边界，再问恢复所有者，然后是瞬态性，最后才考虑边界与契约。新错误处理点不得使用裸 `tracing::error!`/`warn!` 表达上述任何一类——裸日志只在诊断系统自身（emit/crash fallback）与无关事实日志中出现。

矩阵由四层机制强制，强度从编译期到运行时递进：

1. **编译期（框架内）**：`Error` 标注 `#[must_use]` 且 crate 级 `deny(unused_must_use)`——框架代码把 `Error` 值作为表达式语句直接丢弃即编译失败；下游应用的同类丢弃得到 `must_use` warning（应用可自行 `deny` 升级为错误）。
2. **静态审计**：`scripts/diagnostics_audit.py`（接入 AGENTS.md 验证流程）扫描 `src/` 生产代码（剔除测试/验收 feature 门控块），检查错误值的裸 tracing 日志必须有同分支诊断通道、panic 家族必须带契约注释（`todo!`/`unimplemented!` 一律违规）、空体 `Err(_) =>` 吞没必须带理由注释；确属事实日志或设计豁免时在调用点上方加 `diagnostics-exempt: <理由>` 标记。新增违规使工具以非零退出码失败。
3. **落地检测（运行时全覆盖，含应用）**：每个 `Error` 携带处置标记；`report`/`observe_boundary_error`/`observe_transient_error`/pending 入队与恢复尝试在消费时递归标记全原因链（`with_source` 附加的源错误随父错误承担）。Drop 时仍未处置的错误进入全局有界去重观测——按 `(code, 消息前缀)` 每键一条 debug 事件、键数硬上限 256（超出折叠为溢出键），`uix::core::unhandled_error_summary()` 返回 `(累计次数, 去重键数)` 供宿主与测试检视。应用吞掉框架错误（`let _ =`、match 后丢弃）不再是静默行为。
4. **类型入口**：六个处置类别的类型化调用（见上表），处置意图从调用点直接判读。

「马上知道」的即时性由两条通道保证：终态报告在入库的同时经 `on_report` 订阅同步通知宿主（无需 tracing subscriber），全部类别（含瞬态与边界观察）的结构化事件以 `uix::diagnostics` target 即时发射；瞬态观察的量级（累计观察与被抑制次数）并入 `DiagnosticsSnapshot`，宿主可随时回看观察通道的活动而不只依赖事件窗口。

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
| `ReproSnapshot` | private struct | 固定 schema、无用户文本的有界运行现场（私有 repro Module） |
| `PendingFailureQueue` / `PendingFailureSource` | private queue/source | callback 到 owner-thread 的固定容量 typed failure 投递 |
| 瞬态观察（`observe_transient`） | method | 瞬态失败的冷却去重结构化观察，不进报告存储 |
| `observe_boundary_error` / `uix_contract_violation!` | fn / macro | 无句柄层边界观察与契约 panic 的显式处置入口 |

## SMC 落地边界

Diagnostics 的首个 Rust SMC 纵切已经把公开 System 契约与私有实现边界落到源码：

| SMC 角色 | 源码落点 | 所有权与职责 |
|---|---|---|
| System | `src/diagnostics/mod.rs` 的 `Diagnostics` | 对外提供报告、快照和恢复登记/尝试；持有 runtime id、配置，并编排三个私有 Module |
| 私有 Module | `src/diagnostics/reporting/mod.rs` 的 `ReportingModule` | 由一个 Diagnostics 实例拥有；编排报告构建、容量存储和结构化事件发射，不向外暴露实现 |
| 私有 Module | `src/diagnostics/recovery.rs` 的 `RecoveryModule` | 由同一 Diagnostics 实例拥有；持有精确 `Errc` handler 和 RAII 注册生命周期，不访问 reporting Module |
| 私有 Module | `src/diagnostics/crash.rs` 的 crash Module | 由同一 Diagnostics 实例拥有；持有有界 `CrashReport` 模型、原子写入与框架 panic hook，不访问 reporting / recovery Module |
| 私有 Module | `src/diagnostics/repro.rs` 的 `ReproModule` | 由同一 Diagnostics 实例拥有；持有 debug 事件环、窗口数值快照和错误码缓存，不保存任意文本，不访问 reporting / recovery Module |
| Component | `reporting/mod.rs` 的 `ReportDraftBuilder` | 单一负责 typed Error 的预算、脱敏、cause 截断和按策略捕获 backtrace |
| Component | `reporting/store.rs` 的 `ReportStore` | 单一负责 ReportId 顺序、固定容量保留和淘汰计数 |
| Component | `reporting/emit.rs` 的 emission guard/event emitter | 单一负责 tracing 结构化事件、递归抑制和 subscriber panic 隔离 |
| Component | `reporting/notify.rs` 的 `ReportNotifier` / `ReportSubscription` | 单一负责报告即时通知的注册、RAII 注销、递归抑制与处理器 panic 隔离；不访问 store 或发射 |
| Component | `recovery.rs` 的 `RecoveryGuard` | 单一负责同线程递归恢复保护；不拥有 handler 或 System 状态 |
| Component | `crash.rs` 的原子写入与 panic hook | 单一负责 tmp + fsync + rename 原子落盘、失败清理与 hook 递归 guard；不写敏感值，不吞 panic |
| Component | `repro.rs` 的事件环与渲染器 | 单一负责固定事件的容量淘汰、32 KiB 行式渲染和 panic 非阻塞快照；不执行文件写入 |
| Component | `pending.rs` 的 `PendingFailureQueue` / `PendingFailureSource` | 单一负责 callback 失败的固定容量入队、按 source 隔离、溢出信号和 teardown 关闭；不调用 subscriber、handler 或用户代码 |
| 私有 Module | `src/diagnostics/transient.rs` 的 `TransientObservationModule` | 由同一 Diagnostics 实例拥有；持有 `(target, reason)` 冷却窗口与抑制计数（键为静态字符串对，编译期有界），只做去重判定不发射事件；不访问 reporting / recovery Module |

System 只通过 `ReportingModule::{report,snapshot}`、`RecoveryModule::{register,attempt}` 与 `ReproModule` 的固定事实入口
协作；crash Module 由 `Diagnostics::install_panic_hook` 编排，System 将 repro 的纯渲染结果交给 crash 原子写入 Component。私有 Module 不互相引用、查找或持有实例。`report.rs` 的报告值和
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
| `observe_transient` | `observe_transient(&self, target, reason, detail) -> bool` | 瞬态失败的冷却去重观察（30 秒窗口、抑制计数、首条立即发射）；返回本次是否实际发射，不进报告存储 |
| `on_report` | `on_report(&self, handler) -> ReportSubscription` | 每份报告入库的同时在 report 调用线程同步通知（RAII 注销；嵌套上报不再分发；处理器 panic 被隔离为限流 emergency 输出） |
| `debug_mode` / `set_debug_mode` | 查询或动态切换 runtime-scoped 开关 | 全窗口共享；关闭时不采集帧与组件树调试事实 |
| `rebuild_tracing_interest_cache` | `(&self)` | 重新评估 tracing 进程级 callsite interest 缓存；subscriber 晚于首次报告安装导致事件不可见时，调用即可恢复诊断事件可见性 |
| `write_debug_repro_manifest` | `(&self, directory) -> Result<PathBuf, Error>` | 原子写出不含用户文本、上限 32 KiB 的固定 schema 复现清单 |

crate 内编排入口（不属公开 API，由 app 组装层调用）：

| 入口 | 位置 | 语义 |
|---|---|---|
| `Diagnostics::install_panic_hook` | `src/diagnostics/mod.rs` | 绑定 runtime 的 panic hook；`App::run` 在配置加载前安装 |
| `drain_platform_pending_failures` | `src/app/application/lifecycle/runtime/mod.rs` | App owner-thread 任务边界取出 callback/worker 失败，先恢复后报告 |
| `Diagnostics::report_with_origin` + `ReportOrigin::framework` | 框架侧失败边界 | 窗口操作失败、图形恢复终态失败与字体初始化失败经 crate 内入口以 framework 来源上报；图形瞬态失败由 RecoveryDriver 收敛不进入报告 |

## Callback → owner-thread 边界

`Diagnostics` 为一个 runtime 持有共享的固定容量 pending failure queue；每个拥有异步 callback 的资源上下文取得独立 `PendingFailureSource`。callback 只构造并入队 typed `Error`，不执行 tracing、报告、恢复 handler 或用户代码。owner-thread 在资源自己的安全边界取出对应 source 的错误，再交给资源所属 System 的恢复状态机；队列溢出必须折叠为有界的 `InsufficientResources` 信号，source 关闭必须拒绝或清理晚到 callback，旧 generation 不得污染替换资源。

App 的 owner-thread 安全点（`drain_platform_pending_failures`）先对每个取出的失败调用 `attempt_recovery`：注册 handler 返回 `Recovered` 时不再 report（仅 tracing 观察），`Unhandled` / `Failed` 才最终 `report` 一次，符合「不能恢复的才报告」。`Errc::GraphicsDeviceLost` 的恢复接线只能通过共享 `RebuildRequest` 请求窗口引擎在下个帧边界执行 graphics 拥有的有界恢复序列；请求一次性且不覆盖首个未处理失败。`report` 只建立观察事实，不得隐式执行恢复 handler；实现验证必须锁定这两个入口的分离。

所有跨 FFI、系统 callback 与 worker 边界必须满足同一内部实现义务：unwind 不跨 ABI，失败只转换并投递一次，晚到事件受 generation / teardown 状态约束，owner-thread 保持 source 内顺序，并且不会在 callback 中执行 subscriber、恢复 handler 或应用用户代码。这些私有 callback、队列与 generation 细节不建立项目测试；只从公开 `Diagnostics` 契约测试使用方可见结果。

## 组件：ErrorReport

报告限制总字节、分段、cause 数和 metadata，清理控制字符并按策略捕获 backtrace，是否捕获可经 `ErrorReport::backtrace` 观察；回溯策略默认 `FatalOnly`（仅为致命报告捕获），`ErrorsAndFatal` 覆盖错误与致命报告，`Disabled` 全部禁止。达到容量淘汰旧报告，错误风暴不能无限增长内存。

## 组件：CrashReport 与 panic hook

`CrashReport` 字段有界（线程 256B、消息 2048B、位置 512B），渲染为行式 key=value 并替换控制字符，非字符串 panic payload 以固定标记代替，不写敏感值。`App::run` 在配置加载前安装绑定 runtime 的 panic hook：配置 `crash_report_directory` 时原子写入 CrashReport 与复现清单（唯一 tmp + fsync + rename，冲突 replace，失败清理），未配置时行为等同默认；hook 始终转发 previous hook 保持默认输出与 unwind/abort 语义（不吞 panic），`PanicHookGuard` 防止 hook 内递归。

## 组件：统一调试与 ReproModule

debug 开关由 Diagnostics System 的原子状态唯一持有，应用组合根、环境变量和快捷键只委托该入口。事件循环为同批输入分配关联身份，窗口帧驱动消费同一身份并记录布局、渲染、GPU 提交、呈现、脏区、动画、失效来源和协调跨度；窗口驱动只缓存上一已完成帧的无文本数值快照，renderer 在普通内容、`AfterChildren` 与全部 overlay 完成后执行唯一最终 Debug Pass。组件检查器通过既有 WidgetTree → ScenePaint 桥生成只读快照，并在根画布坐标读取 layout/viewport/visible、裁切、绘制和交互事实；调试图元不进入 Picture 或 backdrop 快照，也不改变命中、布局或应用绘制语义。

`ReproModule` 的正常事件入口先检查 debug 原子位，关闭时不取锁。开启时最多保留最近 128 个固定事件、8 个窗口和 16 个错误码事实；错误文本、窗口标题、组件内容、资源标识、环境变量和路径不进入其模型。panic hook 使用 `try_lock`，当前线程若已持有复现锁则写出 `capture_busy=true` 的最小清单，不能因采集现场再次等待或死锁。

## 数据、隐私与生命周期

- 报告、日志和崩溃记录默认视为敏感运行数据；路径、窗口标题、用户输入、凭据、Agent token 与原始业务载荷不得未经脱敏进入记录。
- 内存报告受容量约束，落盘目录、保留期限、访问权限和删除策略由应用宿主显式配置；diagnostics 不擅自远程上传。
- `AppRuntime` 唯一拥有 diagnostics System 实例；公开 clone 句柄只共享该 runtime 状态，不创建第二套报告或恢复登记。
- 关闭顺序为停止新来源、关闭 pending source、在 owner-thread 有界 drain、注销恢复订阅、恢复/转发 panic hook；关闭完成后的 late callback 只能被拒绝或丢弃，不能重启 runtime。

## System 不变量

不内建通用 retry、GPU 重建、业务补偿、远程上传或第二套日志系统；结构化事件经 tracing，具体恢复归资源所属模块。
