# uix-app

UIX 的 Rust 原生跨平台应用开发框架，目标是提供开发完整应用所需的通用能力，而不只是 UI。范围包括组件与绘制、应用运行时、网络、数据库与 ORM、文件与平台服务、诊断，以及 UIX 语言和开发工具；能力按需组合，不要求应用自行拼装所有基础设施。

当前一个 Cargo package 已提供响应式状态、布局求解、CPU/GPU 绘制、View 树、窗口与事件循环、部分平台服务、设置与键值存储、诊断及语言工具。**网络和 ORM 已纳入产品目标，尚未提供对应的统一公开 API**；目标范围与当前入口见[能力与边界](docs/产品/能力.md#完整应用开发能力目标)。

本仓库独立维护框架文档。具体按钮、输入、图表、表单、Ant Design 预设和图标资源由组件库拥有，不属于本仓库的 API 清单。

组件体系的定位是 **uix-app 的组件层对应 React，uix-widgets 对应 Ant Design 的可复用组件库**：这个类比不限定 uix-app 的整体范围。框架提供通用组件机制及应用基础能力，组件库基于框架公开能力提供可供多个应用复用的控件与设计系统。应用也可以直接用框架创建自己的组件和组件库，详见[框架与组件库定位](docs/产品/定位与原则.md#框架与组件库定位)。

**UIX Lang 的目标定位是 TSX 式的组件编程语言**：代码与 UI 表达自然组合，同一份定义支持编译与动态加载。现有标签语法是可演进的实现基线，设计不合理时可以调整，不要求永久保留；新语法尚未实现，见[语言定位与语法演进](docs/uix-lang/设计/语言边界.md#语言定位与语法演进)。

> 当前源码处于早期开发阶段，存在不兼容变化。历史 `0.0.8` 发行快照不包含当前 main 的全部能力，本次源码更新不代表新的 crates.io 或平台制品发布。

## 按能力使用

关闭默认功能后选择所需能力；完整依赖关系见 [框架能力](docs/架构/框架能力拆分.md)。

| 能力 | 用途 | 本仓库示例 |
| --- | --- | --- |
| `reactive` | 状态、派生值、订阅 | [reactive](examples/reactive.rs) |
| `layout` | 纯布局求解 | [layout](examples/layout.rs) |
| `graphics` | CPU 绘制、文本与图像资源 | [offscreen](examples/offscreen.rs) |
| `ui` / `test-harness` | 无窗口 View 与行为验证 | [headless_ui](examples/headless_ui.rs) |
| `platform` | OS 服务与窗口适配 | [平台能力](docs/使用/框架设施/平台能力.md) |
| `application` | 窗口应用组装 | [快速开始](docs/使用/入门/快速开始.md) |
| `lang-build` / `lang-tools` | 编译器、构建器、CLI 和 LSP | [UIX 语言](docs/uix-lang/README.md) |
| `uix-modules` / `extensions` | 可移植模块与脚本扩展 | [模块规范](docs/uix-lang/规范/应用模块.md)、[软件扩展](docs/使用/能力与边界/软件动态扩展.md) |

```sh
cargo run --no-default-features --features reactive --example reactive
cargo run --no-default-features --features layout --example layout
cargo run --no-default-features --features graphics --example offscreen
cargo run --no-default-features --features test-harness --example headless_ui
```

旧 `runtime` 总开关已移除。应用使用具体组件库时，依赖和声明由应用选择；框架通过通用组件目录计算所需单元，不内置某个具体控件库的裁剪规则。

## 构建与平台

```sh
cargo build --release
```

当前主要面向 Windows 与 Linux。`rust-toolchain.toml` 选择 Rust stable；原生构建需要目标平台 SDK、链接器与开发库。`application` 组装窗口应用，`opengles`、`vulkan`、`d3d11`、`d3d12` 或 `metal` 选择相应后端。交叉编译通过不等于目标平台原生窗口、GPU 与输入验收。

严格按需交付使用 `uix build --release`，配置与 host/target 边界见 [使用方按需编译](docs/架构/使用方按需编译.md)。开发构建与已选代码的链接优化不能替代未选模块不参与目标编译的验证。

## 图形与字体

绘制入口、图像帧和字体输入统一在 [绘制文档](docs/使用/界面构建/绘制.md) 维护。框架不分发完整正文字体；应用使用系统字体或显式字体包。图标字体由采用它的组件库或应用维护。

## 目录

| 路径 | 内容 |
| --- | --- |
| `src/` | 框架、平台、语言与工具源码 |
| `examples/` | 框架能力示例 |
| `tests/`、`tests-src/` | 仓库验证入口与测试实现 |
| `editors/zed/` | 产品的 Zed 语言扩展 |
| `docs/` | 本框架的产品、使用、语言和架构文档 |
| `scripts/` | 构建与维护脚本 |

本仓库没有独立的过程宏、编译器、运行时、CLI 或 LSP Cargo 子包。

## 公开 API 测试

按改动选择公开测试与实际消费者验证。基础目录合同可运行：

```sh
cargo test --no-default-features --features lang-tools --test library_catalog_public_api
```

图形一致性入口为 `scripts/run_gpu_parity.sh`。纯文档修改检查内容与链接；编译、无窗口行为和目标平台真实呈现分别记录，不互相替代。

## 文档与许可

[文档中心](docs/README.md) · [快速开始](docs/使用/入门/快速开始.md) · [框架接口速查](docs/使用/界面构建/组件速查.md) · [两包迁移](docs/使用/两包迁移.md)

源码使用 [MIT](LICENSE)，第三方材料见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
