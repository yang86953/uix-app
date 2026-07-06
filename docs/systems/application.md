# 应用系统

> 启动、主循环、桥接平台 / View / 渲染。编排者，不实现组件或绘制算法。

## 运行模式

GUI：窗口 + View 根 + 渲染循环。CLI：无窗口，共用 App 外壳。

## 启动 → 主循环

1. 初始化平台 → 创建引擎（GPU 优先，失败 CPU，#59）→ View 构建组件树 → 创建窗口  
2. 每帧：轮询事件 → 映射派发 → AnimationRegistry + Effect → Layout 脏则布局 → Paint/Composite 脏则渲染 → Idle 跳过 present → 重置脏标记  

Layout 失效**不** present。

## 桥接

| 项 | 说明 |
|----|------|
| 事件 | `UiEvent → SystemEvent → Semantic/Custom`（#70 逻辑像素） |
| 场景 | 组件树实现 ScenePaint |
| 主题 | 全局 Theme 快照传入绘制 |

## AppState · 多窗 · Settings

AppState 单例、仅主线程（#55、#88）。ComponentHandle 只读配置 + invalidate/emit（#61、#72）。多窗共享 AppState + Theme，每窗独立树（#93–#94）。Light/Dark 默认跟 OS，可覆盖并运行中监听（#74、#76）。Settings 可选注入，默认不自动存盘（#64）。

窗口持有 Platform 与光标，不参与 layout/render。Present 由主循环在渲染成功后触发。
