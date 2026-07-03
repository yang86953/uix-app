# UIX — Rust Native UI Framework

**跨平台全栈应用开发框架。** 用 Rust 编写的高性能原生 UI 框架，支持 Windows 和 Linux，纯 CPU 渲染 + 可选 GPU 后端。

---

## 项目目标

- **一套代码，多平台运行** — Windows (Win32) + Linux (Wayland)，行为一致
- **组合优先，无继承** — Trait-based 组件体系，拒绝基类层次
- **增量渲染，按需绘制** — CPU 渲染下只重绘变化像素，非全帧刷新
- **响应式状态，自动追踪** — `State<T>` / `Computed<T>` 驱动 UI 更新
- **生产级组件库** — 60+ 开箱即用 Widget（Ant Design 5 设计风格）

---

## 快速开始

```bash
# 运行 GUI 演示
cargo run --bin uix-demo

# 运行 CLI 演示
cargo run --bin uix-demo -- --cli

# 运行全部测试（332+ 个）
cargo test
```

---

## 核心能力

| 能力 | 说明 |
|------|------|
| **声明式 UI** | `ui!` 宏，类 JSX 语法声明组件树 |
| **Flexbox + Grid 布局** | 完整 Flex 实现，支持 grow/shrink/wrap/align |
| **响应式状态** | `State.set()` → 自动通知依赖方 → 局部重绘 |
| **主题体系** | Ant Design 5 设计令牌，运行时亮/暗切换 |
| **增量渲染** | 脏矩形 + 像素滚动 + FrameGraph Pass 裁剪 |
| **3D 空间支持** | 统一空间坐标系统，2D 零开销 + 3D 按需启用 |
| **DI 容器** | 类型擦除依赖注入 |

---

## 仓库结构

```
Cargo.toml                 # workspace 根
├── platform/              # OS 抽象层 — Win32/Wayland/文件/日志/通知/设置
├── graphics/              # 2D 渲染引擎 — CPU 光栅化 + FrameGraph + 空间坐标
├── ui/                    # Widget 框架 — 60+ 组件 + 布局 + 动画 + 状态 + 主题
│   └── macros/            # proc-macro 库（ui!/define_widget!/tree!）
├── app/                   # 应用入口 — 窗口生命周期 + CLI + DI
├── demo/                  # 演示应用
├── AGENTS.md              # 编码规范与设计原则
└── ARCHITECTURE.md        # 架构设计与模块详解
```

详细架构说明见 [`ARCHITECTURE.md`](ARCHITECTURE.md)。

---

## 平台支持

| 平台 | 后端 | 状态 |
|------|------|------|
| Windows | Win32 API（GDI DIB 呈现） | ✅ 完成 |
| Linux | Wayland（SHM buffer + EGL） | ✅ 运行 |
| macOS | — | ❌ 暂不支持 |

---

## 许可

MIT
