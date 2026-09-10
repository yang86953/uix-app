# UIX

> **⚠️ 早期开发阶段（0.0.8）：API 与功能可能发生不兼容变化，不承诺生产稳定性。**
>
> **Early development (0.0.8): APIs and features may change in incompatible ways; production stability is not guaranteed.**

UIX 是一个 **Rust 原生跨平台应用框架**，目标是让同一套界面与应用逻辑运行在多个平台的原生窗口上。

跨平台复用的单位是**完整应用，而不只是组件树或 UI 描述语言**：UIX 统一界面、应用组合根、
生命周期、窗口与事件循环、平台能力、本地数据和运行保障。框架通过自身图形管线绘制到原生窗口，
不把浏览器或 WebView 作为界面运行时。

UIX is a **Rust-native, cross-platform application framework**. Its goal is to reuse
both UI and application logic across native windows—not just share a widget tree.
It brings together UI, application lifecycle, windows and event loops, platform
capabilities, local data, and diagnostics, without a browser or WebView runtime.

- 当前主要面向 **Windows 与 Linux 桌面应用**；macOS 的交叉检查不等于已完成原生窗口、GPU 和输入验收，移动端也不是当前交付承诺。
- 语言版本：Rust 2024（工具链由 `rust-toolchain.toml` 固定）
- 开源许可证：[MIT](LICENSE)（第三方材料见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)）

## 界面与业务 / UI and application logic

| 层次 | 开发方式与边界 |
| --- | --- |
| 界面，推荐方式 | **UIX Lang**：用 `.uix` 描述结构、样式、展示数据、界面状态与局部交互；普通界面在正式构建时生成 Rust，默认不携带语言解释器。 |
| 界面，同样受支持 | **Rust 声明式 API**：直接在 Rust 中构建界面；与 UIX Lang 已登记能力共享 UI 语义，而不是另一套渲染引擎。 |
| 应用业务与宿主 | **Rust**：持有业务规则、领域状态、持久化、网络与平台服务调用、异步任务、导航与窗口编排、应用生命周期。 |
| 按需动态扩展 | **标准 Scheme R7RS-small** 承载扩展自己的逻辑和可选界面；可移植 UIX 应用模块另有 AOT/动态执行通路。两者都受显式宿主能力与生命周期约束，不获得任意宿主状态或服务访问权。 |

UIX Lang 是推荐的界面入口，但 UIX **不要求所有界面都使用一种专用语言**，也不把 Rust 限定为
UI 语言背后的运行时。应用的业务与宿主仍由 Rust 管理，界面通过 props、回调或显式根输入消费窄契约。

UIX Lang is the recommended UI authoring path, not a mandatory application language.
Ordinary `.uix` interfaces generate Rust at build time; Rust's declarative UI API is
also supported. Rust owns application business logic and lifecycle. Optional dynamic
extensions have explicit host capability boundaries.

## 产品范围与原则 / Scope and principles

- **应用框架而非 Web 封装**：提供声明式组件、响应式状态、布局与主题、自定义绘制、多窗口、导航和应用运行设施，不依赖 HTML/CSS/JavaScript 界面技术栈。
- **原生与可观测边界**：平台差异在框架边界内处理；不支持的能力返回可识别错误，不把平台分支散落到普通应用逻辑中。
- **资源与按需编译**：空闲休眠、按需重绘；通过 feature 选择能力，并由 release 链接裁剪不可达代码。实际体积、内存与速度取决于应用和构建配置，不承诺固定性能倍数。
- **最小必要外部依赖**：按真实用途、许可、编译成本与维护收益取舍，不为减少依赖数量重复实现成熟能力。
- **明确的功能边界**：本地数据能力不等于内建网络或云同步；媒体帧呈现不等于解码和音频播放。可选 Agent 控制面服务于显式授权的本机 UIX 应用，不是通用第三方桌面控制工具。

UIX 仍处于早期阶段。产品方向、源码包含某个后端、交叉编译通过、目标平台实机验收和正式制品发布
是不同事实；不能从其中一项推导其余项。仓库中的版本号也不表示已发布对应的公开安装包或 crates.io 包。

## 包身份与发布状态 / Package identity and status

- Cargo 包名为 **`uix-app`**；在 Rust 代码中通过库名 **`uix_app`** 引用，例如 `use uix_app::prelude::*;`。
- **尚未上传 crates.io**：当前版本只能以源码方式消费（本仓库的 git 依赖或 path 依赖），
  不要按 `uix-app = "0.0.8"` 的 registry 写法引用；实际上架后另行公告。
- The Cargo package is **`uix-app`**; import it in Rust as **`uix_app`** (e.g. `use uix_app::prelude::*;`).
  It is **not yet published to crates.io** — consume it from this repository (git or path dependency) only.

## 构建 / Build

需要 Rust 工具链及目标平台所需的原生开发依赖。`rust-toolchain.toml` 固定 Rust 版本。

```bash
cargo build --release
```

公开 API 回归入口：

```bash
cargo test --features agent-control,test-harness --test "*_public_api" --quiet
```

日常修改只运行受影响的 `--test <目标名>`；`test-harness` 用于无窗口的公开行为测试，
不进入发布构建。编译或公开 API 测试通过，不代替真实窗口、GPU、输入和跨平台验收。
纯说明文字修改只需核对内容与链接，无需重跑构建或测试。

## 字体 / Fonts

UIX 不随仓库或 crate 分发完整正文字体：

- 未显式配置字体时，应用通过平台系统字体发现装载正文字体与 CJK 回退
  （fontconfig / 注册表 / CoreText）；无窗口后台操作面同样只走该窄接口，
  不创建显示连接、窗口或访问用户应用。
- 需要确定性正文外观的应用随应用自带字体，并以 `App::font_bundle`（窗口应用）
  或 `AgentWorkspace::font_bundle`（后台操作面）显式注入；显式字体包优先于系统发现。
- 找不到可加载的正文字体时，启动返回可识别错误。随包的 `assets/fonts/lucide.ttf`
  只是图标字体（Lucide，ISC），不会也不得被用作正文字体。
- `tests/fixtures/fonts/uix-test-body.ttf` 是仅用于自动化测试的 OFL 子集 fixture，
  详见 `THIRD_PARTY_NOTICES.md`。

UIX does not bundle a body font. Apps default to platform font discovery, or bundle
their own font via `App::font_bundle` / `AgentWorkspace::font_bundle` (explicit bundles
take priority). Missing loadable body fonts fail startup with a typed error; the bundled
`lucide.ttf` is icon-only. `tests/fixtures/fonts/uix-test-body.ttf` is an OFL subset
fixture used by automated tests only.

## 工作区构成 / Workspace layout

本仓库是一个 Cargo 工作区，根 crate 为 `uix-app`（Rust 库名 `uix_app`），另含五个成员 crate：

| Crate | 说明 |
| --- | --- |
| `uix-app` | 框架本体（根 crate，Rust 库名 `uix_app`） |
| `uix-derive` | UIX 过程宏 |
| `uix-lang-compiler` | 构建期共享编译器 |
| `uix-lang-runtime` | 可移植的模块执行契约 |
| `uix-lang-cli` | 语言工具链命令行适配器 |
| `uix-lang-lsp` | 语言服务器 stdio 适配器 |

## 目录结构 / Repository layout

| 路径 | 内容 |
| --- | --- |
| `src/` | 框架源码：`app`、`core`、`data`、`draw`、`graphics`、`native`、`platform`、`diagnostics` 等模块 |
| `tests/` | 公共 API 与契约测试 |
| `examples/` | 可运行示例 |
| `demo/` | 示例应用（含 `task-list` 与 `uix-lang-demo`） |
| `editors/zed/` | Zed 编辑器扩展（语法高亮 + 语言服务器） |
| `extensions/` | 扩展样例 |
| `assets/` | 图标字体与图片素材（第三方来源见 `THIRD_PARTY_NOTICES.md`） |
| `scripts/` | 构建与维护脚本 |

## 编辑器支持 / Editor support

`editors/zed/` 提供 Zed 扩展，包含 UIX 语言的 tree-sitter 语法与语言服务器接入。

## 许可证 / License

以 MIT 许可证授权，见 `LICENSE`。随仓库分发的第三方材料（字体、tree-sitter
运行时等）仍归各自许可条款约束，详见 `THIRD_PARTY_NOTICES.md`。
