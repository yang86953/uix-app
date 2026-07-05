# 应用系统

> 系统职责：应用如何**启动、调度、桥接**各子系统完成一帧。

---

## 1. 定位

| 原则 | 说明 |
|------|------|
| 编排者 | 不实现组件或绘制算法 |
| 统一主循环 | GUI 由单一主循环驱动 |
| 桥接 | 连接平台、界面、渲染三端 |

本文定义应用系统边界；源码若与本文不一致，按本文重构。

---

## 2. 运行模式

| 模式 | 说明 |
|------|------|
| GUI | 窗口 + View 根 + 渲染循环 |
| CLI | 命令行，无窗口 |

---

## 3. 启动（GUI）

1. 初始化平台  
2. 创建渲染引擎：**GPU 优先，失败回退 CPU**（#59）  
3. 从 View 构建组件树  
4. 创建窗口  
5. 进入主循环  

可配置：标题、尺寸、主题。

---

## 4. 主循环（单帧）

1. 等待 / 轮询平台事件  
2. 映射为 UI 事件并派发到组件树  
3. 更新动画注册表；Effect 帧末回调  
4. Layout 脏 → 布局  
5. Paint / Composite 脏 → 帧渲染  
6. Idle → 跳过 present；否则上屏  
7. 重置脏标记  

Layout 失效**不**触发 present。

---

## 5. 桥接职责

| 桥接 | 说明 |
|------|------|
| 事件 | `UiEvent → SystemEvent → SemanticEvent / CustomEvent`（#70 逻辑像素） |
| 场景 | 组件树实现 ScenePaint；渲染系统不依赖具体树类型 |
| 主题 | 全局 Theme 快照传入绘制 |

---

## 6. AppState 与多窗

| 能力 | 设计 |
|------|------|
| AppState | App 单例；**仅主线程**（#55、#88） |
| ComponentHandle | 只读配置 + invalidate / emit（#61、#72） |
| 多窗口 | v1 支持；**共享** AppState + Theme；每窗独立组件树（#93–#94） |
| Light/Dark | 默认跟 OS；可显式覆盖；**运行中监听 OS**（#74、#76） |
| Settings | 可选注入；**默认不**自动存盘（#64） |

---

## 7. 窗口职责

创建平台窗口、持有 Platform、光标与调试状态；**不参与** layout 与 render。Present 由主循环在渲染成功后触发。

---

## 8. CLI 与 DI

CLI 与 GUI 共用 App 外壳。简易 DI 供 demo 注入，非核心路径。

---

## 9. 落地要求

| 项 | 要求 |
|----|------|
| 主循环 | GUI 由单一主循环驱动；Idle 帧跳过 present |
| 场景桥接 | `WidgetTree` 通过 app bridge 暴露为 `ScenePaint` |
| 事件桥接 | app 边界统一完成 `UiEvent → SystemEvent`；组件派发语义事件或自定义事件 |
| AppState | App 单例主线程持有，ComponentHandle 只读配置 |
| 多窗口 | v1 多窗共享 AppState + Theme，每窗独立组件树 |

---

## 10. 相关决策

#55、#59、#64、#70、#74、#76、#88、#93–#94 — [decisions.md](../decisions.md)
