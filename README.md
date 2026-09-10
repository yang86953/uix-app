# UIX

UIX 是一个跨平台原生应用框架：界面用声明式 UI 语言编写，在构建期编译为原生代码，
运行时由 Rust 实现。

UIX is a cross-platform native app framework. Interfaces are written in a declarative
UI language, compiled to native code at build time, and executed by a Rust runtime.

- 目标平台：Linux 与 macOS
- 语言版本：Rust 2024（工具链由 `rust-toolchain.toml` 固定）
- 许可证：MIT（第三方材料见 `THIRD_PARTY_NOTICES.md`）

## 构建 / Build

需要 Rust 工具链。`rust-toolchain.toml` 已固定版本，rustup 会自动安装对应工具链。

```bash
cargo build --release          # 构建
cargo test                     # 运行测试
```

## 工作区构成 / Workspace layout

本仓库是一个 Cargo 工作区，根 crate 为 `uix`，另含五个成员 crate：

| Crate | 说明 |
| --- | --- |
| `uix` | 框架本体（根 crate） |
| `uix-derive` | 过程宏：路由键的 `Display` 派生 |
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
| `assets/` | 字体与图片素材（第三方来源见 `THIRD_PARTY_NOTICES.md`） |
| `scripts/` | 构建与维护脚本 |

## 编辑器支持 / Editor support

`editors/zed/` 提供 Zed 扩展，包含 UIX 语言的 tree-sitter 语法与语言服务器接入。

## 许可证 / License

以 MIT 许可证授权，见 `LICENSE`。随仓库分发的第三方材料（字体、tree-sitter
运行时等）仍归各自许可条款约束，详见 `THIRD_PARTY_NOTICES.md`。
