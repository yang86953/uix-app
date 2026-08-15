# application 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 app 系统的应用组装、运行模式、共享服务和生命周期根。依赖：全部下层系统，以及 [event-loop](event-loop.md)、[window](window.md) 和可选 [agent](agent.md)。导出：`App`、`AppMode`、`AppHandle` 与应用级服务容器。
>
> **当前实现线索**：相关实现暂分布于 `src/app/application/`（app_handle.rs 等）、`src/app/session_runtime.rs`；重构后应用组装和进程级生命周期统一归本模块。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `App` | struct | 收集根 View、主题上下文、backend、设置、服务和运行选项 |
| `AppMode` | enum | 区分 GUI、CLI 及未来无窗口运行模式 |
| `AppHandle` | struct | 向目标窗口投递操作并访问允许共享的应用服务 |
| `AppRuntime` | internal struct | 持有进程级状态、窗口目录和关闭协调 |
| `Container` | struct | 按类型注册与解析应用级 singleton |
| `Cli` / `CliArgs` | structs | 解析并路由不依赖 GUI surface 的命令 |

## 组件：App

启动顺序为：验证配置与运行模式 → 建立应用服务 → 按需加载设置 → 解析 platform/graphics 能力 → 创建初始窗口 → 可选发布 Agent discovery → 进入事件循环。任一步失败都按所有权逆序清理，不能留下半初始化 runtime。

主窗、副窗与 `AppHandle::update_view` 的根 View 统一经过 application 根组装入口：仅当根节点没有显式背景时应用 ui/theme 拥有的 `NeutralRole::BgLayout` 语义令牌。调用方显式背景（包括透明色）保持最高优先级；该默认值不改变普通 `ViewNode` 的透明语义，也不进入 graphics 或 platform 生命周期。

## 组件：Container

容器只管理与应用同寿的线程安全 singleton，不自动构造服务、不驱动 timer，也不执行设置保存。`WidgetTree`、Renderer、窗口对象、字体/图像会话等线程亲和资源属于窗口或 graphics 生命周期，不得注册为进程共享 singleton。

## 组件：AppHandle

`AppHandle` 不暴露 `WidgetTree` 或平台内部对象。跨线程 UI 操作必须带目标窗口身份进入其队列；排队成功不等于回调完成或画面已呈现。

## 模块不变量

- application 只负责组装与进程生命周期，不复制 ui、graphics、platform 的内部机制。
- CLI 模式不隐式创建窗口、surface 或图形 backend。
- 应用关闭先停止接受新工作，再关闭窗口和可选控制面，最后释放共享服务。
