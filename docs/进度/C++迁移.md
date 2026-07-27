# C++ 迁移

[← 返回进度索引](../进度.md)

> **接口**：适用于判断 UIX 从 Rust 重建为 C++23 的当前阶段、交付边界、xmake 构建契约、迁移顺序、验收门槛与止损条件。本文档权威持有迁移决策和活跃差距；产品能力仍由[产品索引](../产品.md)路由，目标分层仍由[架构索引](../架构.md)持有，Rust 当前行为和测量证据由本索引的其他进度正文持有。导出：迁移执行阶段与切换条件。

## 当前结论

迁移采用“独立重建、行为对照、分层切换”，不逐行翻译 Rust，也不在现有 Rust 模块内混入第二套实现。

| 项目 | 已决定边界 |
|---|---|
| 目标语言 | C++23；不把 C++20 或 C++26 作为项目语言模式 |
| 构建系统 | xmake；本机已确认 `xmake v3.0.9+master.9f54e096b` 可用，正式工程必须固定并记录可复现版本 |
| 目标仓库 | 新建独立 `uix-cpp` 仓库；当前 `uix-app` 保留为行为、视觉和性能基线 |
| 首个交付平台 | Windows 11 24H2 x64 |
| 首个 GPU 路径 | 原生 Vulkan；Software 仅作为确定性的参考渲染与失败回退 |
| 初始链接模型 | 各系统静态库；公共 ABI 稳定前不发布 DLL ABI |
| 错误模型 | 可恢复失败使用 typed error / `std::expected`；异常不得跨 OS、图形驱动或 C ABI callback |
| 所有权模型 | RAII + `std::unique_ptr` + 代际句柄；裸指针只表达非拥有借用，`std::shared_ptr` 不作为默认组件所有权 |
| 当前状态 | 仅计划已建立；C++ 仓库、xmake 工程和迁移代码均未开始 |

## 范围与非目标

### 本次迁移保持

- 产品能力、公开语义、主循环责任、错误上下文和六系统单向依赖。
- `core → platform → graphics → ui → data → app` 的逻辑所有权。
- 组件身份、布局、事件、绘制、文本命中、IME、无障碍和 Agent 动作共用同一 UI 管线。
- Windows 首发目标及现有视觉、性能和内存证据的可比较性。

### 本次迁移不做

- 不逐文件、逐类型机械翻译约 17.9 万行 Rust。
- 不为兼容历史内部模块图复制过时抽象。
- 不在第一阶段同时实现 Vulkan、D3D12、OpenGL、Metal 和 Wayland。
- 不以 C++23 Modules、C++26 反射或其他预览特性作为可构建前提。
- 不在功能等价前删除 Rust 仓库、历史、基线标签或验证产物。
- 不把“能显示窗口”当成迁移完成；切换必须经过功能、视觉、性能、内存和长稳门槛。

## C++23 使用边界

工程统一使用 `set_languages("c++23")`。允许优先采用以下已进入 C++23 或此前标准且适合本项目的能力：

- `std::expected` 表达可恢复失败。
- `std::span`、`std::string_view` 表达受限借用；不得越过所有者生命周期。
- `std::move_only_function` 表达拥有捕获状态的回调。
- `std::pmr` 为逐帧 scratch、命令录制和短期布局数据提供显式内存域。
- Concepts、Ranges、`constexpr` 和强类型枚举约束公共模板与状态机。
- `std::jthread`、`std::stop_token` 表达后台任务取消和退出。

以下能力不得直接成为首版核心依赖：

- C++26 或厂商预览扩展。
- 编译器支持仍不一致的 Modules 工作流。
- 隐式跨线程协程生命周期。
- 依赖 RTTI 遍历组件树或以异常代替正常控制流。

每项非基础 C++23 库能力必须通过 feature-test macro 和 MSVC、clang-cl 双工具链 CI 证明；缺失时提供局部兼容实现，不降低整个项目的语言标准。

## xmake 构建契约

### 目标目录

```text
uix-cpp/
├── xmake.lua
├── xmake/
│   ├── options.lua
│   ├── packages.lua
│   ├── rules.lua
│   └── warnings.lua
├── include/uix/
├── src/
│   ├── core/
│   ├── platform/
│   ├── graphics/
│   ├── ui/
│   ├── data/
│   └── app/
├── demo/
├── tests/
│   ├── unit/
│   ├── integration/
│   ├── pixel/
│   └── soak/
├── assets/
│   ├── fonts/
│   └── shaders/
└── tools/
```

根 `xmake.lua` 只声明项目、C++23、构建模式、全局规则和子目录路由；模块目标在所属目录声明。

### 目标依赖图

```text
uix_core
  ├── uix_platform
  │     └── uix_graphics
  │             └── uix_ui
  └── uix_data          │
           └──────── uix_app
                         ├── uix_demo
                         └── uix_tests
```

具体依赖保持：

| xmake target | kind | 允许依赖 |
|---|---|---|
| `uix_core` | static | 无 |
| `uix_platform` | static | `uix_core` |
| `uix_graphics` | static | `uix_core`、`uix_platform` |
| `uix_ui` | static | `uix_core`、`uix_platform`、`uix_graphics` |
| `uix_data` | static | `uix_core` |
| `uix_app` | static | 全部下层系统 |
| `uix_demo` | binary | `uix_app` |
| `uix_tests` | binary/test group | 按测试所属模块依赖 |

### 构建模式与命令面

首版提供 `debug`、`releasedbg`、`release` 和 sanitizer 模式。预期命令面为：

```powershell
xmake f -p windows -a x64 -m debug --toolchain=msvc
xmake
xmake test
xmake run uix_demo

xmake f -p windows -a x64 -m releasedbg --toolchain=clang-cl
xmake
xmake test
```

正式落地时必须补齐：

- MSVC `/W4`、严格一致性和标准 `__cplusplus`；clang-cl `-Wall -Wextra -Wpedantic`。
- Debug/CI 的 AddressSanitizer；clang-cl 可用时增加 UndefinedBehaviorSanitizer。
- Release 与 releasedbg 的 LTO、符号和崩溃栈策略。
- 依赖精确版本与 lock 文件；禁止无版本上界地追踪包仓库头部。
- shader、资源和测试数据作为显式 xmake 生成/复制规则，不依赖工作目录偶然状态。
- 构建产物只进入 xmake 输出目录，不写入源码目录。

## 生产代码护栏

| 边界 | 强制规则 |
|---|---|
| 资源所有权 | 拥有资源的裸指针和散落 `new/delete` 禁止；平台与 GPU 句柄由移动型 RAII owner 持有 |
| 组件关系 | 长寿命组件关系使用带 generation 的 ID；释放后旧 ID 必须失效 |
| 树修改 | reconcile、layout、paint 或事件遍历期间不直接改变当前遍历结构；写入有序 mutation queue 后在明确屏障提交 |
| 内存域 | 每帧/每次布局临时值进入 `std::pmr` arena；对象不得逃逸出其 memory resource 生命周期 |
| 回调 | Win32 wndproc、Vulkan callback 和 C ABI thunk 全部 `noexcept`，捕获失败后投递到 owner thread |
| GPU 生命周期 | command、descriptor、buffer、texture 和 swapchain 资源按 submission/fence generation 延迟回收 |
| 并发 | 主线程亲和对象不可跨线程借用；后台任务通过消息与拥有型数据返回，必须支持取消和 join |
| 错误 | 解析、I/O、平台和 GPU 边界保留 cause；最终责任边界只报告一次 |
| 类型擦除 | 不依赖 `dynamic_cast` 遍历组件；使用显式 capability、type ID 或受控 variant |
| 公共 ABI | 首版只承诺源码级 C++ API；插件/跨语言边界另设稳定 C ABI，不导出 STL 容器 ABI |

## 迁移阶段

### M0：冻结 Rust 基线

交付：

- 在任何 C++ 源码工作前，把当前工作树全部提交为可恢复基线并创建带注释标签。
- 记录当前能够通过和不能通过的 Cargo 构建、测试、示例与文档检查；不得把旧证据冒充最新工作树结果。
- 固化 Windows/NVIDIA/Vulkan 参考场景、截图、像素输出、帧耗时、working set 和 private bytes。
- 导出公开 API、110 项功能、92 项组件/663 个声明状态、文本/IME和错误语义清单。

完成条件：任意后续阶段都能回到精确 Rust 基线，并能在同一环境执行可比较测量。

### M1：建立 C++23 与 xmake 骨架

交付：

- 创建独立 `uix-cpp` 仓库和上述目录、target DAG、模式、警告、测试与 sanitizer 规则。
- 落地 `uix_core` 的基础值类型、typed error、代际 ID、slot storage 和 memory resource 工具。
- 建立格式化、静态分析、单元测试、包版本锁和 CI。

完成条件：MSVC 与 clang-cl 均能从干净 checkout 完成 debug/releasedbg 构建和核心测试；源码目录无生成物。

### M2：Windows 平台闭环

交付：

- Win32 窗口、消息泵、唤醒、计时、DPI、输入、光标、剪贴板和生命周期。
- 原生句柄 RAII、owner-thread 约束、callback `noexcept` 边界和 typed failure。
- 最小 IME/文本输入闭环，不把按键事件冒充文本输入。

完成条件：创建、显示、resize、最小化、恢复、输入和关闭均有自动化证据；sanitizer 无资源生命周期错误。

### M3：图形垂直切片

交付：

- API-neutral PaintOp/DisplayList、Renderer、RenderTarget 和资源表。
- 原生 Vulkan surface、swapchain、命令提交、fence 回收和 device-lost 路径。
- Software 参考渲染器用于确定性像素测试。
- 矩形、圆角、路径、图像、clip、transform、opacity 和 retained framebuffer。

完成条件：同一 DisplayList 在 Vulkan 与 Software 上满足规定的像素容差；resize、最小化和 device-lost 不泄漏或双重释放。

### M4：文本与字体

交付：

- 字体发现、fallback、shaping、换行、命中、光标和选区的共享布局结果。
- 首版优先评估 FreeType + HarfBuzz；平台字体发现保持独立边界。
- glyph atlas、缓存预算、R8/大字号缩放策略与 GPU 上传。
- CRLF、空行、Tab、标点约束、复杂脚本、双向文本和字素簇测试。

完成条件：测量、绘制、选择和 hit-test 使用同一字符/字素映射；中英文基线和首批复杂脚本通过真窗矩阵。

### M5：UI 引擎主干

交付：

- 声明式 View、持久 WidgetTree、keyed reconcile 和 generation-safe identity。
- Constraints、Flex、Grid、盒模型、增量失效和 layout scratch。
- 平台事件到 SystemEvent、语义事件、捕获/冒泡、焦点与命中的单一路径。
- PaintContext 录制、逐窗 scheduler、动画和 mutation barrier。

完成条件：窗口中的文本、图片、Button、Flex/Grid、Scroll、Input 和 Overlay 构成首个端到端可交互切片。

### M6：组件分批迁移

按依赖顺序迁移，不按 Rust 文件顺序迁移：

1. 基础显示与交互：Text、Image、Icon、Button、Input。
2. 布局与滚动：Container、Flex、Grid、Scroll、Virtualization。
3. 反馈与浮层：Tooltip、Popover、Modal、Drawer、Toast。
4. 数据与选择：List、Table、Tree、Menu、Select。
5. 复杂展示：Chart、Calendar、Carousel、富文本及其组合组件。

每批必须同时交付行为测试、状态矩阵、浅/深色、DPI、键盘、IME、无障碍语义和截图证据，不积压到最终阶段。

### M7：data、app 与控制面

交付：

- Settings codec、原子保存和恢复 sidecar。
- App 组装、逐窗 WindowSession、DI、活动工作与多窗口生命周期。
- Diagnostics、崩溃上下文、恢复注册和日志。
- Agent Bridge 的本机 IPC、鉴权、语义动作和 owner-thread 投递。

完成条件：数据冷路径不进入每帧热路径；跨线程和 IPC 输入均在边界验证；失败只在最终责任边界报告一次。

### M8：对等验证与切换

交付：

- 公开用法迁移为 C++ 示例并建立 xmake 驱动的示例编译检查。
- 110 项功能账本逐项映射到 C++ 实现和证据。
- 92 项组件/663 个声明状态重新生成并人工验收视觉矩阵。
- 性能、内存、长稳、DPI、AMD/Intel 和最低 Windows build 复测。

完成条件：满足下节全部硬门槛后，C++ 才成为默认开发主线；Rust 进入只读维护而不是立即删除。

### M9：Rust 退役

交付：

- 保存最终 Rust 标签、构建说明、行为证据和已知差距。
- 把仍有价值但未迁移的测试向量、shader 和资源转为语言中立资产。
- 只有 C++ 连续通过发布门槛后，才停止 Rust 功能开发。

完成条件：任何公开行为都能在 C++ 代码、测试或明确差距中找到唯一归属，不依赖未归档的 Rust 工作树。

## 验收与止损门槛

当前 Rust 数据仅作为参考，M0 必须在同一机器、同一场景重新采样。

| 类别 | C++ 切换硬门槛 |
|---|---|
| 构建 | 干净 checkout 可由固定 xmake/工具链复现；MSVC 与 clang-cl 通过 |
| 功能 | 首发范围功能测试全通过，公开示例全部编译运行 |
| 视觉 | 组件状态矩阵完成人工验收；无未解释像素差异 |
| 文本 | 测量、换行、绘制、selection、caret、hit-test 和 IME 字符索引一致 |
| 性能 | 参考场景稳定帧中位数不比重新采样 Rust 基线慢 10%；不得用功能缺失换取结果 |
| 内存 | 同场景 working set/private bytes 不比 Rust 基线高 10%；无增长型缓存或资源滞留 |
| 稳定性 | 8 小时长稳、重复窗口/Modal/Drawer/主题/DPI 场景无累积退化 |
| 安全 | sanitizer、静态分析和 GPU validation 无未豁免高严重问题 |
| 恢复 | resize、最小化、surface 重建和 device-lost 的资源状态可恢复或 typed fail |

M3 垂直切片是首个止损点。若包含窗口、文本、输入、滚动、动画和 Vulkan present 的最小场景不能同时满足以下条件，则暂停扩大迁移：

- 架构和开发体验相对 Rust 明确改善。
- 性能与内存达到可比较基线，而非仅“能运行”。
- 资源生命周期能由局部规则和工具验证，而不是依赖全局人工记忆。
- xmake、依赖获取和双工具链构建可稳定复现。

## 活跃风险

| 风险 | 控制 |
|---|---|
| C++23 实现差异 | 固定 MSVC/clang-cl 版本、feature-test macro、双工具链 CI |
| 机械翻译保留旧复杂度 | 只迁移产品语义与逻辑系统契约，每阶段先定义最小公共边界 |
| 组件悬空引用 | 代际 ID、集中 slot storage、mutation barrier、sanitizer |
| callback 越过已销毁 owner | `noexcept` thunk 只投递 owned message，owner thread 决定恢复或丢弃 |
| GPU 提前释放 | submission generation + fence retirement queue |
| PMR 对象逃逸 | memory resource 所有权进入类型/模块契约，测试使用毒化资源检查 |
| 文本语义回退 | shaping、layout、绘制和 hit-test 共用结果；复杂脚本纳入 M4 而非最终补丁 |
| 依赖和 xmake 漂移 | 精确版本、lock、离线缓存验证、禁止隐式系统包兜底 |
| 范围膨胀 | Windows/Vulkan 首发闭合后再增加第二平台或第二 GPU 后端 |
| Rust 基线不可复现 | M0 先提交、标签、环境记录和测量报告，之后才写 C++ |

## 工作量边界

本计划按验收门槛推进，不承诺以日期代替完成。单人实现 Windows/Vulkan 首发功能对等预计以季度为单位；完整组件矩阵和多平台对等预计以年为单位。任何并行工作都必须落在独立模块和明确接口上，不能靠同时复制多套尚未稳定的实现换取表面进度。
