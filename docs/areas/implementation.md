# 实现落地计划

← [architecture](architecture.md) · 决策 [#154](../decisions.md#d154) [#157](../decisions.md#d157)

> **P0–P5 主体已落地**。本文保留 **阶段摘要**、**实现进度**、**后续 backlog** 与 **源码目录详表**；刻意保留或未决项边界 → [demand-driven · 剩余差距](systems/demand-driven.md#剩余差距)。

状态证据统一分级：`designed` → `coded` → `automated` → `compiled:<platform>` → `hardware:<platform>` → `production`。`✅` 仅用于局部代码/测试已闭合的分项，不代表跨平台或生产验收；平台交付以 [证据矩阵](#平台与后端证据矩阵) 为准。

## 索引

| 章节 | 说明 |
|------|------|
| [已落地阶段（P0–P5）](#已落地阶段p0p5) | 分阶段目标与验收摘要 |
| [实现进度总览](#实现进度总览) | 已落地能力按域汇总 |
| [当前验证基线](#当前验证基线-2026-07-10) | 最近一次全量测试结果与剩余证据缺口 |
| [后续工作](#后续工作) | 未实现 backlog（权威清单） |
| [P6 生产级框架](#p6-生产级框架) | 生产级优先项、图形后端里程碑、P6.8 可组合渲染轴 |
| [源码目录详表](#源码目录详表) | `src/` 路径与系统文档映射 |
| [维护](#维护) | 文档同步约定 |

**关联**：[demand-driven](systems/demand-driven.md) · [application](systems/application.md) · [view-reactive](systems/view-reactive.md) · [decisions](../decisions.md)

---

<a id="分阶段路线图"></a>
<a id="已落地阶段p0p5"></a>

## 已落地阶段（P0–P5）

设计（#154）— 下列阶段 **主体接线已完成**；按域能力清单见 [实现进度总览 · 已实现](#已实现按域)。

```text
P0 基础零闲置          P1 热更新           P2 App 运行时 API      P3 渲染窄路径
├─ 三态主循环 #106     ├─ reconcile 接入   ├─ Timer #132          ├─ PicturePolicy #122
├─ Registry #115       ├─ 帧内合并 #118    ├─ post_to_ui #133     ├─ Composite #107
├─ wait_until #127     ├─ reconcile合并#153├─ MainThreadQueue #137  └─ PointerMove #109
└─ Effect DeepIdle     ├─ handler重绑#123  ├─ on_start #140
                       └─ StateSlotId #143 └─ TestClock #139

P4 多窗                P5 组件 Handle 体系
├─ WindowSession #116  ├─ AppState #145
├─ open_window #148    ├─ Snapshot #146 #151 #152
├─ update_view #149    ├─ Handle emit #147
└─ State fan-out #150  └─ ComponentId #101

P6 图形后端            ← #162 #163，详见下文
├─ GraphicsBackend 枚举 + registry probe
├─ Windows D3D11/12
├─ Linux Vulkan
└─ macOS Metal + AppKit SoftwareEngine bootstrap
```

| 阶段 | 目标 | 关键决策 | 验收（已达成） |
|------|------|----------|----------------|
| **P0** | DeepIdle 真休眠 | #106 #115 #127 #105 | 无事件无 present；DeepIdle 不 `tick_effects` |
| **P1** | 热更新不上全量 build | #118 #153 #123 #143 | State 批次单次 reconcile；handler 智能重绑；显式 capture API 与 `semantic_handler!` 宏可生成 fingerprint，普通闭包仍保守重绑（#159/#160） |
| **P2** | App 公开定时/投递 | #132–#137 #140 #139 | Timer cancel 回 DeepIdle；TestClock 测 drain / wait_until |
| **P3** | L2 最小脏区 | #122 #129 #107 #109 | 滚动 Composite；Picture 自适应；PointerMove 窄路径 |
| **P4** | 多窗编排 | #116 #148 #149 #150 | 副窗独立 tree/session；State fan-out 仅 wake 有关窗 |
| **P5** | Handle 零维护 | #145–#152 #147 | mount 自动 snapshot；emit 走 dispatch_semantic |

**原则**（仍适用于后续工作）：

- **不**为兼容保留固定 interval 探活或每轮 `tick_effects`；新 API / 主循环路径继续受 #105 零闲置约束。
- 新增能力须能回答 [demand-driven · 新 API 审查清单](systems/demand-driven.md#新-api-审查清单)。

---

## 实现进度总览

**能力清单**（按域）；未实现 → [后续工作](#后续工作)；细节 → 各系统 `> **实现注记**`。阶段摘要 → [已落地阶段（P0–P5）](#已落地阶段p0p5)。

### 已实现（按域）

#### 基础设施 · `core`

| 能力 | 要点 | 文档 |
|------|------|------|
| 几何与标识 | `Point` / `Rect` / `EdgeInsets`；`ComponentId` / `WindowId`（Generational，跨窗 tree scope） | [foundation](systems/foundation.md) · [#101](../decisions.md#d101) |
| Damage | `DirtyRegion` → `DamageRegion` → `PresentDamage` | [foundation · Damage](systems/foundation.md#damage-区域) |
| 错误 / 日志 / 诊断 | `Errc` / `Error`；Logger；Fatal crash log；**默认 log**（#89）；App 默认错误 Toast overlay | [foundation](systems/foundation.md) |

#### 平台 · `native`

| 能力 | 要点 | 文档 |
|------|------|------|
| 生产后端 | **Windows、Linux Wayland、macOS**（AppKit + Metal CpuUpload）；`create_platform()` 三平台 backend 均已编码；**交付优先级 Windows 优先**（[#167](../decisions.md#d167)） | [platform](systems/platform.md) |
| 测试后端 | `FakePlatform` 完整 trait 实现 | [platform · 测试](systems/platform.md#测试平台) · [testing](systems/testing.md) |
| 事件循环 | `IEventLoop`；`EventLoopWaker` 外部线程 wake | [platform · 事件模型](systems/platform.md#ieventloop) |
| 窗口可选能力 | `WindowOps` 返回 `Result`；未支持记 `NotImplemented` | [platform · 窗口可选能力](systems/platform.md#窗口可选能力) |
| GPU 上下文 | Windows D3D11/WGL；Linux Vulkan/EGL；`create_gpu_context` | [platform · 工厂与后端](systems/platform.md#工厂与后端) · [rendering · 多图形 API](systems/rendering.md#多图形-api) |

#### 绘制 · `draw`

| 能力 | 要点 | 文档 |
|------|------|------|
| GPU 引擎 | OpenGL ES / D3D11 `GpuNative × Swapchain` → `GpuEngine`；Vulkan/Metal `Cpu × PixelUpload` → `PresentUploadEngine`；失败回退 SoftwareEngine | [rendering · 引擎](systems/rendering.md#引擎) · [#59](../decisions.md#d59) · [#172](../decisions.md#d172) |
| 失效管线 | `Invalidation::Paint` / `Layout` / `Composite`；脏区合并 | [rendering · 管线](systems/rendering.md#管线与失效) |
| Composite 滚动 | Wheel / 键盘 / 拖拽 → `scroll_region` memmove | [rendering](systems/rendering.md) · [#107](../decisions.md#d107) |
| PicturePolicy | 元数据 + 运行时信号；`node_count≥8 && est_pixels≥65536`；部分静态 widget `Eligible` | [rendering](systems/rendering.md) · [component · PicturePolicy](systems/component.md#picturepolicy-元数据122) |
| 动画帧 | `AnimationRegistry` deadline；`tree.update(dt)` 窄标脏 | [rendering · 动画帧](systems/rendering.md#动画帧) · [component · 动画](systems/component.md#动画) |

#### 应用 · `app`

| 能力 | 要点 | 文档 |
|------|------|------|
| 三态主循环 | DeepIdle / RegisteredActive / Active；`wait_until`；Effect 仅 Active 且 pending 时 tick | [demand-driven · 主循环](systems/demand-driven.md#主循环状态机) · [application](systems/application.md#主循环) |
| WindowSession | 每窗独立 tree / Registry / 三态 / MainThreadQueue | [application](systems/application.md) |
| 多窗单 loop | `open_window`；`window_id` 路由；共享 AppState + Theme | [application · 多窗](systems/application.md#appstate--多窗--settings) |
| App 运行时 API | `run_after` / `run_interval`；`post_to_ui`；`update_view` / `set_root`；全局 `set_theme` | [application](systems/application.md) · [demand-driven](systems/demand-driven.md) · [#175](../decisions.md#d175) |
| AppHandle 生命周期 | cloneable；close 时 cancel Timer、清空队列 | [application · AppHandle](systems/application.md#apphandle-生命周期) |
| reconcile 帧内合并 | `pending_root` + State 批次；layout 前至多一次 | [view-reactive · reconcile](systems/view-reactive.md#reconcile-合并) |
| on_start 回调 | `.on_start` / `.on_window_start` 首帧前注入 handle | [application · on_start](systems/application.md#on_start) |
| DI / Settings | `AppHandle::resolve`；`App::settings(path)` opt-in load | [application · DI](systems/application.md#cli-与-di) · [data](systems/data.md) |
| TestClock | `AppClock` / `TestClock` 注入 deadline 与 drain 测试 | [testing](systems/testing.md) |

#### 界面 · `ui`

| 能力 | 要点 | 文档 |
|------|------|------|
| View / Reconciler | keyed diff；同类型 patch；`view_factory` + `pending_root` | [view-reactive](systems/view-reactive.md) |
| 响应式 | `State` / `Computed` / `Effect`；窄 paint；跨窗 paint site fan-out | [view-reactive · 响应式](systems/view-reactive.md#响应式) |
| StateSlotId | `State::new` 单调 slot；capture 指纹 `TypeId + slot_id` | [view-reactive · StateSlotId](systems/view-reactive.md#stateslotid) · [#143](../decisions.md#d143) |
| Handler 智能重绑 | 显式 capture fingerprint / `semantic_handler!` → generation；无指纹保守重绑（#160） | [view-reactive · Handler](systems/view-reactive.md#handler-变更判定-135) · [event](systems/event.md) |
| ComponentHandle | mount 自动 snapshot；getter / 窄 invalidate / emit | [component · ComponentHandle](systems/component.md#componenthandle6172145) |
| component! | 推荐 authoring；`SnapshotSource` 自动提取；`#[snapshot(skip)]` | [component · Authoring](systems/component.md#authoring) |
| 无障碍快照元数据 | `ComponentConfigSnapshot::accessibility()` / `ComponentHandle::accessibility()` 派生 role、name、state，并提供静态 ARIA role/attribute 导出 | [component · ComponentConfigSnapshot](systems/component.md#componentconfigsnapshot) · [#99](../decisions.md#d99) |
| Manager 横切 | Focus / Interaction / Drag per-tree | [component · Manager](systems/component.md#manager-横切) |
| 内置 Widget | 86 个 Big Bang；OverlayStack（Modal / Tooltip / 菜单等） | [component · 内置 Widget](systems/component.md#内置-widget-目录) · [overlay](systems/overlay.md) |
| 事件 | HandlerTable（`ComponentId` key）；PointerMove 窄路径 | [event](systems/event.md) |
| 主题样式 | Theme / StyleSet 五态；`follow_system_theme` opt-in | [theme-style](systems/theme-style.md) |

### 关键机制（跨域）

| 机制 | 一句话 | 细则 |
|------|--------|------|
| **按需零闲置**（#105） | 有触发才工作；无 pending 零开销 | [demand-driven](systems/demand-driven.md) |
| ActiveWorkRegistry | 内置动画 / Tooltip / AppTimer / IME 自动 register | [demand-driven · Registry](systems/demand-driven.md#activeworkregistry) |
| 帧内合并（#118） | UiEvent → due work → post_to_ui → reconcile → layout → render | [demand-driven · 帧内合并](systems/demand-driven.md#帧内合并) |
| 开发者零维护（#130） | 调用方不维护 Picture 名单、Registry、标脏范围 | [demand-driven · 开发者契约](systems/demand-driven.md#开发者契约零维护) |

<a id="组件-snapshot-覆盖"></a>

### 组件 Snapshot 覆盖

全部 86 个内置 widget 均已手写 `SnapshotSource` 静态配置提取，排除 hover / pressed / focused / scroll / animation phase 等运行态。

| 分类 | 已覆盖 Widget |
|------|---------------|
| general | Button, Icon, Typography, Label, Divider, Space, FloatButton |
| containers | Container, Grid, Layout, Header, Sider, Content, Footer, Splitter, Affix, BackTop |
| navigation | Menu, MenuItem, Tabs, Breadcrumb, BreadcrumbItem, Pagination, Steps, Anchor, AnchorItem, Dropdown |
| input | Input, InputNumber, Select, Checkbox, Radio, Switch, Slider, Rate, Form, FormItem, TreeSelect, DatePicker, TimePicker, ColorPicker, Cascader, AutoComplete, Mentions, Segmented |
| display | Card, List, Tree, Carousel, Collapse, Descriptions, Avatar, Badge, Tag, Image, Empty, Result, Skeleton, Timeline, Calendar, Table, SelectableList |
| feedback | Modal, Drawer, Tooltip, Popover, Popconfirm, Alert, Message, Notification, ProgressBar, Spin |
| other | ScrollView, BarChart, LineChart, PieChart, QRCode, RichText, ThemeToggle, Transfer, Upload, Watermark |

`component!` 自定义组件自动提取 pub 字段，`#[snapshot(skip)]` 可排除。

---

<a id="当前验证基线-2026-07-10"></a>

## 当前验证基线（2026-07-10）

基于当前工作树完成复验：

| 门禁 | 结果 | 证据范围 |
|------|------|----------|
| `cargo test --all-targets` | **通过** | lib **1070/1070**；demo **19/19** |
| `cargo test --doc` | **通过** | 3 passed；20 ignored |
| Windows compile | **通过** | default、`--no-default-features`、`--all-features` |
| Linux cross-check | **通过** | `x86_64-unknown-linux-gnu` `--all-targets` default + all-features；Vulkan 测试断言不再隐式要求 target 依赖提供 `Debug` |
| macOS cross-check | **通过** | `x86_64-apple-darwin` `--all-targets` default + all-features |
| `cargo clippy --all-targets` | **通过，有既存 warnings** | 无 hard error；warning-free 不作为已完成事实 |
| 文档门禁 | **通过** | `check_project_docs.py --strict-design --json`：0 errors / 0 warnings；`git diff --check` 通过 |

此前两项历史失败已闭合：架构扫描现忽略注释/Rustdoc/字符串并收紧 graphics cfg 路径；ScrollView content expand 按最近 viewport 的 X/Y 滚动轴分别处理，Vertical/Horizontal/Both 与 Collapse 动态展开测试均通过；Horizontal 多直接子项现沿 X 轴流式排列并封口非滚动 Y 轴。Space 不再复用 stale 子 frame，Form/FormItem 的固定最小尺寸、label/padding/status 区均受窄父 frame 上限约束；Card body 现排除 actions 固定区，极小尺寸下以零 frame 清除旧布局，actions 也不会越出卡片。Container/Grid/ScrollView/Space/Form/FormItem/Card 已统一为 pass-local `LayoutChild` preparation → arrange，Arrange 不再递归 measure，通用 helper 也不再以旧 frame 污染零高度测量（[#174](../decisions.md#d174)）。FrameRenderer 现以“版本变化或 LayerTree 尚未构建”触发 build，合法初始版本 `0` 不再跳过首帧；Picture 在 backend 无 offscreen 能力时同帧直绘子树，不再吞掉完整静态 UI。D3D11 复杂填充现原生覆盖单一严格嵌套孔洞的 `EvenOdd` / `NonZero` winding 语义，并由真实 HWND 路径链像素回读守卫；多孔/深层嵌套、相交/接触轮廓和自交仍由拓扑守卫转端到端 soft fallback；demo 覆盖清单现以语义测试区分未实现、已实现但未单独展示与待真机验证。graphics probe 现保留候选、失败阶段、选中 API 与完整错误；窗口关闭已编码为 session/GL 资源/context/native window 顺序，session 与 native window 幂等性有分层测试，real-window factory 测试覆盖 context 创建/关闭。Linux/macOS 仅完成 cross-check，未做真机运行；完整 GL/context/window 组合顺序、Windows GUI 视觉/GPU 驱动矩阵、IME/无障碍真实设备与打包流程仍未验证。

Windows D3D11 real-window smoke 曾暴露 demo 页面内 `State::new` 在根 reconcile 后重置、以及 `ButtonBuilder::widget()` 丢弃 HandlerTable 注册两项真实交互缺陷；首页与应用能力页计数 State 已提升到应用生命周期，交互改用保留 handler 的 View DSL，并由根 reconcile 回归守卫覆盖。随后 D3D11 与无 GPU feature 的 SoftwareEngine 均完成首帧、颜色、按钮点击即时更新、最大化/恢复和标题栏关闭 smoke；Software 路径额外暴露并修复 `Color` 未按 `AARRGGBB` 编码、GDI 因 1px damage padding 越界而跳过局部拷贝两项缺陷。D3D11 真实窗口自动化现通过 staging texture 执行 row-pitch aware BGRA readback，并断言首帧像素内容；同一真实 HWND 测试依次验证 Hardware 与显式 WARP context、adapter identity、clear/readback 和 present，WARP 为 Microsoft Basic Render Driver（vendor `0x1414`、device `0x008C`）。context 初始化会记录 Hardware/WARP、adapter 名称、vendor/device ID 与专用显存，Hardware 失败会明确记录原因再尝试 WARP，两者均失败时保留双错误。2026-07-10 当前主机验证为 `hardware`：NVIDIA GeForce RTX 4070 Ti SUPER（驱动 `32.0.16.1062`、vendor `0x10DE`、device `0x2705`、D3D11 报告 16061 MiB VRAM）；真实 GUI 操作完成副窗创建、副窗触发 light→dark、主窗触发 dark→light及分别关闭，1200×800 主窗与 520×320 副窗的四组 swapchain backbuffer 均完成视觉验收。当前自动截图链路不能读取 GPU 前台组合内容，因此该项只计为单 GPU 的 hardware + backbuffer GUI 证据，前台 capture 与多驱动矩阵仍待补。`DXGI_SWAP_EFFECT_DISCARD` 不保证 present 后的 backbuffer，因此 backend 不再错误声明 partial redraw，而是在既有 Active 帧内提升为全帧。WGL 现以隐藏 bootstrap HWND/context 加载 `wglChoosePixelFormatARB`，再为真实 HWND 选择并校验可绘制、支持 OpenGL 的双缓冲格式。OpenGL soft fallback 不再依赖 GLES 可选 BGRA 上传，而是使用 core RGBA 上传并在 blit shader 交换 R/B。完整 real-window 测试现同时覆盖 ES 3 engine 初始化、GPU 原生红色背景、CPU 回退蓝色图元的精确 readback、GL 零错误与 present 调用；这些属于 `automated` 证据，OpenGL ES 屏幕呈现与 GPU/驱动矩阵仍待验证。

Windows native IME 现按 HWND 隔离 composition 与 UTF-16 decoder 状态，处理 `WM_IME_STARTCOMPOSITION` / `WM_IME_COMPOSITION` / `WM_IME_ENDCOMPOSITION`，并从 IMM32 读取预编辑/结果串；`WM_CHAR` 会聚合代理对，不再丢失 emoji。`ITextInput` 的 start/stop/cursor rect 已统一为 Result-only；`WidgetTextInput` capability 让 event loop 在文本组件聚焦期间自动持有无 deadline IME session，并在失焦、禁用、移除时停止。Input 现区分 preedit 与 committed value，支持 FocusIn、内联预编辑、primary underline、尾随 caret 与候选窗 rect；SoftwareEngine 自动化覆盖偏移裁剪和 DisplayList replay，真实 GUI 覆盖 placeholder、`abc` 提交与 Microsoft Pinyin 候选窗跟随 caret。当前 Microsoft Pinyin 的 IMM32 `GCS_COMPSTR` 只暴露空白占位，读音由系统候选 UI 持有；应用内 phonetic preedit 仍需 TSF/UI-less text store，故 IME 不能标记 production。

DisplayList 现完整记录 Input 使用的 clip stack 与预布局 glyph；Canvas offset 在 CPU/OpenGL/D3D11 三条路径均同步作用于 clip。RenderObject 脏帧边直绘边录制，列表只供后续 clean frame 重放，不再同帧二次覆盖或重复 alpha；遇到底层直绘绕过 PaintOp 时主动判定录制不完整并回退 live paint。自动化覆盖偏移 clip、placeholder/value/preedit 缓存重放与单帧半透明绘制一次。

运行时全局主题已由 `AppHandle::set_theme(Theme) -> Result<()>` 落地：App 级 pending 命令合并连续请求并仅 wake 一次，主循环在帧门控前替换主题、向主窗和全部副窗广播 `ThemeChanged`，新窗继承当前主题；关闭 handle 明确返回 `InvalidState`。自动化覆盖最终值合并、主副窗广播及 palette-only Paint / 零 Layout；demo 应用能力页现提供真实副窗入口，主副窗各自的 `ThemeToggle` 均可反向驱动同一全局主题。SoftwareEngine 真实双窗口已完成 light↔dark 双向同步、暗色语义面、选中态、文字对比和裁剪验收。暗色 token 的语义背景与次级边框已改为向暗色基底混合；demo shell、首页和共享提示/卡片改用 `ColorValue` 语义色。D3D11 主题 GUI 仍属于后续硬件矩阵。

`embed()` 现会在进入 View Reconciler 前递归物化 `WidgetComponent::build()` 的组件持有子树；`Space::child` 等 legacy interop 子节点不再只在冷启动展开、随后因 `ViewNode.children` 为空而被 reconcile 删除。自动化覆盖同类型 `Space` 从 Label 子树切换为 Input 子树，真实 SoftwareEngine demo 的“输入”页控件缺失问题由此闭合。

Windows 原生窗口过程现为每个 HWND 持有独立 `WindowState` 绑定，事件按该绑定写入对应 `WindowId`；从队列取事件时再选择对应窗口的输入/系统服务句柄。真实双 HWND 回归已覆盖独立 resize 状态与事件路由，销毁副窗也不再发送线程级 `WM_QUIT`。App/demo 已通过 SoftwareEngine 完成副窗创建、主副窗双向主题交互与分别关闭的真实 GUI smoke；D3D11 联合 GUI 与多 GPU/驱动矩阵仍待验收，不能据此标记 production。

---

<a id="平台与后端证据矩阵"></a>

### 平台与后端证据矩阵

矩阵只记录证据，不从“代码存在”推导“可交付”。`—` 表示该级别不适用，`待验证` 表示当前文档没有可复核证据；每次验证必须同步提交、命令与日期。

| 平台 / 路径 | coded | automated | compiled | hardware / GUI | production |
|-------------|:-----:|:---------:|:--------:|:--------------:|:----------:|
| Windows D3D11 `GpuNative × Swapchain` | 是 | 单元/集成与 real-window Hardware/WARP factory + adapter identity + BGRA staging readback 测试通过 | default/no-default/all-features | RTX 4070 Ti SUPER hardware context；主副窗 light↔dark backbuffer GUI 通过；待前台 capture、输入/IME 与多 GPU/驱动矩阵 | 否 |
| Windows OpenGL ES / Software fallback | 是 | 单元/集成与 real-window ES 3 engine 原生绘制 + soft fallback 精确 readback / GL 零错误测试通过 | default/no-default/all-features | Software 基础 GUI smoke 通过；OpenGL ES 屏幕呈现与完整交互矩阵待验 | 否 |
| Linux Vulkan / EGL | 是 | Windows 主机未运行目标测试 | `--all-targets` cross-check default/all-features | 待 Wayland/GPU 真机 | 否 |
| macOS Metal / Software fallback | 是 | Windows 主机未运行目标测试 | `--all-targets` cross-check default/all-features | 待 AppKit/Metal 真机 | 否 |
| iOS / Android | 否 | — | — | — | 否 |

证据记录与当前完整命令见 [当前验证基线](#当前验证基线-2026-07-10)；平台矩阵未达到 `hardware` 前，文档只能写“backend 已编码”，不得写“平台 parity 已完成”。

---

<a id="p6-生产级框架"></a>

## P6：生产级框架

**当前优先级**（[#167](../decisions.md#d167)）：**Windows 优先** — 先把 Windows 端做到 **生产可用**；Linux / macOS parity 与 macOS 原生验证 **随后**；native raster 仍为性能增强 backlog。开发策略摘要 → [project · 开发策略](../project.md#开发策略)。

**图形后端目标**（#162）：在保持 `IGraphicsContext` / `GraphicsEngine` 契约不变的前提下，扩展多种 GPU API 与 factory 选型；选型仅在初始化完成，符合 #105。架构原则 → [graphics-backend-pluggable · 图形 API 架构原则](systems/graphics-backend-pluggable.md#图形-api-架构原则)（非 P6 任务不必通读全文）。

### 生产级优先项

| 优先 | 项 | 状态 | 说明 |
|------|-----|------|------|
| P0 | **Windows 生产可用** | 进行中 | 稳定性、阻塞项、demo/docs 同步；**当前主验证与交付环境**（[#167](../decisions.md#d167)） |
| P0 | 全量测试恢复零失败 | `automated` 已完成 | lib 1070/1070、demo 19/19；历史布局回归、扫描假阳性、D3D11 复杂拓扑、颜色通道、GDI damage 越界、首帧 LayerTree/Picture 回退、DisplayList glyph/clip、WGL core loader/caps、Win32 多窗状态串用、embedded build 子树删除与 UTF-16 代理对错误均闭合 |
| P0 | probe 诊断与资源关闭 | `coded + partial automated` | 候选/阶段/selected/完整错误进入有序报告并有自动化守卫；WindowSession/native window 幂等性分层测试通过；GL 资源/context/window 组合顺序待 GUI/驱动 smoke |
| P0 | 平台层抹平差异 | `coded + automated` | Window 与 ITextInput Result-only API、直接依赖与 cfg 守卫已收敛；上层无 OS cfg |
| P1 | Linux / macOS parity | `coded + compiled` | 两目标 `--all-targets` default/all-features cross-check 通过；运行/硬件 parity 待验证 |
| P1 | macOS 原生运行验证 | 待验证 | AppKit backend、Metal CpuUpload、IME 已接；需真机验收 |
| P1 | demo / docs 与实现同步 | `automated` 基线已完成 | demo 19/19；覆盖状态语义守卫、局部 State reconcile 守卫、strict-design 与链接/锚点检查通过，后续持续维护 |
| P2 | 无障碍 v1 基线 | 部分 | role/name/state 快照、键盘导航已落地；屏幕阅读器桥待后续 |
| P1 | D3D11 GPU native raster | 部分 ✅ | **Windows 优先**（[#167](../decisions.md#d167) [#169](../decisions.md#d169)）；fill/stroke rect·circle + 轴对齐 line + identity solid glyph atlas + identity linear/radial gradient + identity 简单 path及单一严格嵌套孔洞（CPU tessellate → GPU mesh）+ identity box/ambient shadow（SDF）；多孔/深层嵌套、相交/接触轮廓、自交与非 identity 文本等仍走既有 soft 路径 — 见 [P6.8](#p68-可组合渲染轴) |
| P2 | native raster（非 Win） | backlog | Metal / D3D12 GPU 光栅 — **增强**，D3D11 之后 |
| P2 | WebGPU 评估 | backlog | P6.6 远期 |

### 图形后端里程碑

| 里程碑 | 内容 | 状态 |
|--------|------|------|
| P6.0 设计 | 抽象分层、平台矩阵、回退链、术语 | ✅ |
| P6.1 基线 | `GraphicsBackend` 枚举；registry 单条目创建 | ✅ |
| P6.2 Windows | D3D11 `GpuNative` × `Swapchain` + WGL `GpuNative` × `Swapchain` | ✅ |
| P6.3 Linux | Vulkan `Cpu` × `PixelUpload` + EGL `GpuNative` × `Swapchain` | ✅ |
| P6.4 macOS | AppKit + Metal `Cpu` × `PixelUpload` context；SoftwareEngine 回退 | 部分 — 原生验证待完成 |
| P6.5 配置 | App builder / env / Settings opt-in | ✅ |
| P6.6 WebGPU | 远期评估 | backlog |
| P6.7 可插拔 registry | registry、probe、统一 present 契约 | ✅ |
| P6.8 可组合渲染轴 | `RasterMode` × `PresentMode` 类型与表驱动装配；`BackendKind` 统一；D3D11 GpuNative 原生 fill/stroke/glyph/gradient/path/shadow | 部分 ✅ — 轴类型/分派/D3D11 原生 fill+stroke+glyph atlas+gradient+简单 path+单一严格嵌套孔洞+box/ambient shadow ✅；其余复杂 path / Metal/D3D12 GpuNative backlog — [#169](../decisions.md#d169) |

<a id="p68-可组合渲染轴"></a>

### P6.8 可组合渲染轴（#169）

**设计**（已确认）：以 [#168](../decisions.md#d168) 正交三轴为 **唯一** mental model；engine 分派 **仅**按 caps + registry 裁决的 `RasterMode` × `PresentMode`（× `GraphicsBackend` identity）合法组合；“×”不表示完整笛卡尔积；`BackendKind` 与上述轴对齐；factory / engine **表驱动正交装配**。非法组合返回诊断并继续 fallback（[#172](../decisions.md#d172)）。旧 bundled 管线枚举已拒绝，不作为设计面。

**实现状态**：正交轴类型、caps、`create_graphics_engine` 轴分派、registry 行 `raster`/`present`、删除 `RenderPipelineProfile` ✅。D3D11 `GpuNative` × `Swapchain`：`D3d11Backend` + VS/PS solid/rounded fill + SDF stroke + identity solid glyph atlas（CPU coverage → R8 atlas → textured quads）+ identity linear/radial gradient + identity 简单 path（flatten + ear-clip）及单一严格嵌套孔洞（fill-rule/winding 分类 + hole-aware tessellate → `GpuSolidMesh`）+ identity box/ambient shadow（SDF outer glow → `GpuBoxShadow`）+ soft alpha blit + swapchain present ✅。单孔切片覆盖 `EvenOdd` 双方向、`NonZero` 反向 winding 孔洞/同向 winding 外环化简与轮廓顺序无关性；多孔/深层嵌套、相交/接触轮廓和自交仍由拓扑守卫转 soft。真实 HWND 自动化已走 `Path → D3d11Backend → GpuSolidMesh` 并回读环区/孔中心/外部像素后 present；其余原生 mesh 扩展与 Metal / D3D12 `GpuNative` 仍 backlog。

| 优先 | 项 | 状态 | 说明 |
|------|-----|------|------|
| P0 | D3D11 GPU native raster | 部分 ✅ | **Windows 优先**；caps/registry → `GpuNative` × `Swapchain`；fill/stroke rect·circle + 轴对齐 line + identity solid glyph atlas + identity linear/radial gradient + identity 简单 path / 单一严格嵌套孔洞 mesh + identity box/ambient shadow；其余 Canvas2D soft blit |
| P0 | 正交轴类型与 caps（breaking） | ✅ | `RasterMode` / `PresentMode`；`GraphicsContextCaps` 用 `raster` + `present`；已删 `RenderPipelineProfile`；`create_graphics_engine` 按轴 match |
| P1 | 表驱动 factory / engine 装配 | ✅ | `GraphicsBackendEntry` 含 `raster` + `present`；engine 按 caps 轴组合装配 |
| P1 | `BackendKind` 统一 | ✅ | 文档与注释对齐正交轴；`Cpu`/`Gpu`/`Auto`/`Null` 为引擎级光栅偏好 |
| P2 | Metal / D3D12 GPU 光栅 | backlog | D3D11 之后；同 registry + caps 模式 |

<a id="p67-图形后端架构"></a>
<a id="p67-迁移验收-g1g11--m2m7"></a>

### P6.7 图形后端架构

可插拔 registry、probe 与统一 present 契约（`IGraphicsContext::present(PresentFrame)`）已落地。**设计**为 `RasterMode` × `PresentMode` × `GraphicsBackend` — 见 [#168](../decisions.md#d168) · [#169](../decisions.md#d169) · [P6.8](#p68-可组合渲染轴) · [graphics-backend-pluggable · 可组合渲染轴](systems/graphics-backend-pluggable.md#可组合渲染轴)。`draw::bootstrap_graphics_engine` 为唯一 probe 入口；诊断报告保留 candidate/stage/selected/完整错误并由 app 消费；`native/graphics/<api>/` 为 API 对等实现根目录。正交轴类型与表驱动 `create_graphics_engine` → P6.8 **已落地**；D3D11 `GpuNative` × `Swapchain` 垂直切片 **已落地**。

**Backlog**（权威分项与优先级见 [P6.8](#p68-可组合渲染轴)；另含 D3D12 context `Planned`、WebGPU 远期评估）：

| 项 | 说明 |
|----|------|
| **D3D11 原生路径扩展** | 剩余复杂 `fill_path`（多孔/深层嵌套、相交/接触轮廓、自交仍有 soft fallback 拓扑守卫）与精确 `stroke_path` cap/join；简单 fill_path/stroke_path、单一严格嵌套孔洞、fill+stroke+identity glyph atlas+linear/radial gradient+box/ambient shadow 已落地 |
| Metal / D3D12 GPU 光栅 | 扩展 `RenderBackendRegistry` |

实现细节 → [rendering · 多图形 API](systems/rendering.md#多图形-api) · [platform · 多图形 API 与 factory](systems/platform.md#多图形-api-与-factory) · [graphics-backend-pluggable · 图形 API 架构原则](systems/graphics-backend-pluggable.md#图形-api-架构原则)。

---

## 后续工作

**权威 backlog 清单**（下列表为唯一完整枚举；其他文档仅链接至此）。明细与边界见 [demand-driven · 剩余差距](systems/demand-driven.md#剩余差距)。P6 分项见 [P6 生产级框架](#p6-生产级框架)；可组合渲染轴见 [P6.8](#p68-可组合渲染轴)；其余推进前须人类决策或新决策 #176+。

| 项 | 说明 | 文档 |
|----|------|------|
| **生产级框架（P6 优先）** | **Windows 优先**生产可用；Linux/macOS parity 与 macOS 验证随后。子项含可组合渲染轴（[P6.8](#p68-可组合渲染轴)）、D3D11 GPU raster、macOS 真机验证 | [P6 生产级框架](#p6-生产级框架) |
| **移动端（P7+ backlog）** | **未实现**。目标 iOS / Android 原生 backend；复用 `ui`/`app`/`draw` trait 与零闲置主循环；须 [#166](../decisions.md#d166) 后分阶段切片 | [project · 当前阶段 vs 目标](../project.md#当前阶段-vs-目标愿景) · [plan · P7+](../plan.md#里程碑与工作域) |
| **全栈扩展（backlog）** | **部分**：`data` 仅 Settings KV（opt-in）。**未实现**：网络层、HTTP 客户端、数据同步、服务端集成 | [data](systems/data.md) · [#166](../decisions.md#d166) |
| 无障碍（屏幕阅读器桥） | #99：v1 基线（role/name/state 快照、静态 ARIA 映射、键盘导航）已落地；屏幕阅读器平台桥待后续 | [component](systems/component.md#componentconfigsnapshot) |

---

## 源码目录详表

`src/` 子路径与系统文档的细粒度映射；功能域总览见 [architecture · 功能域 ↔ 系统](architecture.md#功能域--系统)。

| 路径 | 系统文档 | 说明 |
|------|----------|------|
| `src/core/*` | [foundation](systems/foundation.md) | geometry, damage, error, log, diagnostic, **component_id**, **window_id** |
| `src/native/traits/*` | [platform](systems/platform.md) | public OS API；含 `EventLoopWaker` |
| `src/native/shared/*` | [platform](systems/platform.md) | `window_lifecycle`、`ime_events` 等跨后端 helper |
| `src/native/backends/*` | [platform](systems/platform.md) | OS 壳：窗口、事件、CPU presenter（`gdi_presenter` 等）；**不含** GPU API 对等实现 |
| `src/native/graphics/*` | [platform](systems/platform.md) · [graphics-backend-pluggable](systems/graphics-backend-pluggable.md) | **#164** IGraphicsContext 对等 API 实现（vulkan / opengl / d3d11 / metal / d3d12 stub） |
| `src/native/factory/*` | [platform](systems/platform.md) · [graphics-backend-pluggable](systems/graphics-backend-pluggable.md) | **#163** 唯一对外 factory 分派：`mod.rs` + `registry.rs` + `registry_<os>.rs`；`GraphicsBackendEntry` 表驱动 |
| `src/native/test_harness/*` | [platform](systems/platform.md), [testing](systems/testing.md) | FakePlatform |
| `src/draw/pipeline/*` | [rendering](systems/rendering.md) | invalidation, FrameRenderer, AnimationRegistry |
| `src/draw/compositor/*` | [rendering](systems/rendering.md) | ScenePaint, LayerTree |
| `src/draw/engine/*` | [rendering](systems/rendering.md) | `bootstrap.rs`（GPU probe）、`factory.rs`（正交轴分派）、`SoftwareEngine`（cpu）、`PresentUploadEngine`（`present_upload.rs`） |
| `src/draw/gpu_engine/*` | [rendering](systems/rendering.md) | GpuEngine |
| `src/draw/backend/*` | [rendering](systems/rendering.md) | RenderBackend 抽象、`registry.rs`（backend 配对） |
| `src/draw/font/*` | [rendering](systems/rendering.md) | FontService, text backends |
| `src/draw/spatial/*` | [rendering](systems/rendering.md) | PhysicalBox, Mat4, 3D 命中 |
| `src/app/shell/*` | [application](systems/application.md) | App builder, CLI, DI |
| `src/app/window_session.rs` | [application](systems/application.md) | WindowSession、三态、Registry |
| `src/app/window/*` | [application](systems/application.md) | Window — PlatformWindow 轻量包装 |
| `src/app/session_runtime.rs` | [application](systems/application.md) | AppRuntime：window_id 路由、EventLoopWaker |
| `src/app/app_handle.rs` | [application](systems/application.md) | AppHandle |
| `src/app/app_timer.rs` | [application](systems/application.md), [demand-driven](systems/demand-driven.md) | Timer API |
| `src/app/main_thread_queue.rs` | [application](systems/application.md) | post_to_ui FIFO |
| `src/app/active_work_registry.rs` | [demand-driven](systems/demand-driven.md) | ActiveWorkRegistry（内部） |
| `src/app/test_clock.rs` | [testing](systems/testing.md) | AppClock / TestClock |
| `src/app/window_config.rs` | [application](systems/application.md) | WindowConfig、open_window |
| `src/app/event_loop/*` | [application](systems/application.md) | run_widget_loop（设计名 run_app_loop） |
| `src/app/bridge/*` | [application](systems/application.md), [rendering](systems/rendering.md) | ScenePaint impl |
| `src/ui/app_state.rs` | [application](systems/application.md), [component](systems/component.md) | AppState snapshot registry |
| `src/ui/component_handle.rs` | [component](systems/component.md) | ComponentHandle |
| `src/ui/component_snapshot.rs` | [component](systems/component.md) | SnapshotSource |
| `src/ui/view/*` | [view-reactive](systems/view-reactive.md) | View DSL, adapter |
| `src/ui/core/widget/*` | [component](systems/component.md), [event](systems/event.md) | WidgetTree；`tree_core` / `tree_events` / `tree_dirty` |
| `src/ui/event.rs` | [event](systems/event.md) | (single file) |
| `src/ui/layout/*` | [layout](systems/layout.md) | |
| `src/ui/theme/*` | [theme-style](systems/theme-style.md) | |
| `src/ui/foundation/state.rs` | [view-reactive](systems/view-reactive.md) | State / Computed / Effect（经 `ui::state` 重导出） |
| `src/ui/foundation/virtual_scroll.rs` | [layout · VirtualScroll](systems/layout.md#virtual-scroll) | 大列表虚拟滚动 helper；Table/Tree/SelectableList/Select/TreeSelect 已接 VirtualListScroll |
| `src/ui/foundation/*` | [theme-style](systems/theme-style.md), [view-reactive](systems/view-reactive.md), [event](systems/event.md) | style, config, locale, focus_trap, clipboard |
| `src/ui/foundation/style/*` | [theme-style](systems/theme-style.md) | Style, StyleSet |
| `src/ui/animation/*` | [component](systems/component.md) | WidgetAnimation, easing, transition |
| `src/ui/widgets/*` | [component](systems/component.md) | built-in widgets |
| `src/ui/managers/*` | [component](systems/component.md) | per-tree manager container |
| `src/ui/overlay.rs` | [overlay](systems/overlay.md) | |
| `src/data/settings/*` | [data](systems/data.md) | |
| `src/tests/**` | [testing](systems/testing.md) | mirror `src/` layout |
| `src/prelude.rs`、`src/lib.rs` | [public-api · prelude](systems/public-api.md) | 推荐入口 `uix::prelude::*`；导出 App、View、State、ComponentId、常用组件、事件、handle 与布局类型 |

---

## 维护

- 阶段划分或原则变更 → 同步 [decisions.md](../decisions.md) #154（或 #176+ 新决策）与本文件。
- 能力落地或产生新差距 → 更新 [实现进度总览](#实现进度总览) 与各系统 `> **实现注记**`；**不**在本文件恢复 per-file 接线 checklist。
- 新 backlog 项追加到 [后续工作](#后续工作)；边界说明同步 [剩余差距](systems/demand-driven.md#剩余差距)。
- 新增 `src/` 路径映射 → 更新 [源码目录详表](#源码目录详表) 与对应系统文档「源码模块」。
