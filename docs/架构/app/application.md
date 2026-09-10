# application 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 app System 的应用组装、运行模式、共享服务和生命周期根。基础依赖：全部下层 System 的公开契约；与 [event-loop](event-loop.md)、[window](window.md) 和可选 [agent](agent.md) 的协作由 app System 通过私有契约编排，application 不直接持有兄弟 Module 实例。导出：`App`、`AppMode`、`AppHandle` 与应用级服务容器。
>
> **当前实现线索**：相关实现暂分布于 `src/app/application/`（app_handle.rs 等）、`src/app/session_runtime.rs`；重构后应用组装和进程级生命周期统一归本模块。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `App` | struct | 收集根 View、主题上下文、backend、设置、服务和运行选项 |
| `AppMode` | enum | 区分 GUI、CLI 及未来无窗口运行模式 |
| `AppHandle` | struct | 向目标窗口投递操作并访问允许共享的应用服务 |
| `Container` | struct | 按类型注册与解析应用级 singleton |
| `Cli` / `CliArgs` | structs | 解析并路由不依赖 GUI surface 的命令 |

## 组件：App

启动顺序为：验证配置与运行模式 → 建立应用服务 → 按需加载设置 → 解析 platform/graphics 能力 → 创建初始窗口 → 可选发布 Agent discovery → 进入事件循环。任一步失败都按所有权逆序清理，不能留下半初始化 runtime。

主窗、副窗与 `AppHandle::update_view` 的根 View 统一经过 application 根组装入口：仅当根节点没有显式背景时应用 ui/theme 拥有的 `NeutralRole::BgLayout` 语义令牌。调用方显式背景（包括透明色）保持最高优先级；该默认值不改变普通 `ViewNode` 的透明语义，也不进入 graphics 或 platform 生命周期。

## 组件：Container

容器只管理与应用同寿的线程安全 singleton，不自动构造服务、不驱动 timer，也不执行设置保存。注册必须由 application 组合阶段显式完成；解析只允许出现在组合根、启动路由或已知应用服务入口。Module 与 Component 必须经构造参数或窄端口获得依赖，不得在运行路径中把 `Container` 当作全局服务定位器。

`WidgetTree`、Renderer、窗口对象、字体/图像会话等线程亲和资源属于窗口或 graphics 生命周期，不得注册为进程共享 singleton。服务释放顺序与注册顺序相反；仍有窗口或任务引用时不得提前替换或销毁 singleton。

## 组件：AppHandle

`AppHandle` 不暴露 `WidgetTree` 或平台内部对象。跨线程 UI 操作必须带目标窗口身份进入其队列；排队成功不等于回调完成或画面已呈现。

## app System 私有所有者：AppRuntime

`AppRuntime` 是 app System 的唯一进程级所有者，不是 application Module 可复用的 Component。它持有窗口目录、Diagnostics System、共享服务和关闭状态，并由 application Module 在启动与关闭阶段编排；兄弟 Module 只能取得明确的 System 私有契约或窄句柄，不能查找或持有 `AppRuntime`。

## 诊断接线

运行保障是框架级能力（[diagnostics](../platform/diagnostics.md)），由 application 组装层接线：

- `AppRuntime` 持有 Diagnostics System 实例，`AppHandle` 只取得可 clone 的公开句柄（`AppHandle::diagnostics`），不暴露平台内部对象。
- `App::run` 在配置加载前安装绑定 Diagnostics runtime 的 panic hook：配置 `crash_report_directory` 时原子写入有界 CrashReport，未配置时行为等同默认；始终转发 previous hook，不吞 panic。
- 注入点：`App::new().diagnostics(DiagnosticsConfig)` 以配置注入（报告容量、崩溃目录、backtrace 策略）；`App::new().diagnostics_runtime(Diagnostics)` 注入已构建的共享 runtime 实例。
- App owner-thread 任务边界调用 `drain_platform_pending_failures`（`src/app/application/lifecycle/runtime/mod.rs`）：先对每个取出的失败 `attempt_recovery`，注册 handler 返回 `Recovered` 时不再 report（仅 tracing 观察），`Unhandled` / `Failed` 才最终 report 一次——「不能恢复的才报告」。

公开用法见[运行保障](../../使用/框架设施/运行保障.md)。

## 模块不变量

目标中的[运行时动态扩展](extensions.md)由 app System 持有，application 负责显式组装、能力注入与关闭接线。它通过 app 私有契约协调 `extensions`，不直接查找兄弟 Module；扩展不使用 `Container` 访问任意 singleton。该入口尚未实现，现有 `AppHandle::update_view` 也不提供扩展提交或呈现回执。

- application 只负责组装与进程生命周期，不复制 ui、graphics、platform 的内部机制。
- app System 的兄弟 Module 不直接相互查找、调用或持有实例；application 只通过 System 私有契约完成编排与注入。
- CLI 模式不隐式创建窗口、surface 或图形 backend。
- 应用关闭先原子进入 closing、拒绝新窗口/任务/连接，再关闭可选控制面和逐窗会话，最后注销诊断接线并按逆序释放共享服务；重复关闭必须幂等。
- 队列接受、唤醒成功和关闭请求已登记都只表示工作被接纳；完成、呈现或资源释放必须由各自结果契约确认。
