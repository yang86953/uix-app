# 当前计划

← [index](index.md)

本文只记录项目级状态和下一步；详细能力清单、阶段摘要与源码目录映射见 [areas/implementation.md](areas/implementation.md)。

## 当前状态

| 工作流 | 状态 | 依据 |
|--------|------|------|
| P0–P5 按需零闲置主体 | 主体已落地；全量自动化回归已恢复零失败 | [implementation · 已落地阶段](areas/implementation.md#已落地阶段p0p5) · [当前验证基线](areas/implementation.md#当前验证基线-2026-07-10) |
| **P6 生产级框架** | 项目级设计基线 **Executable**；实现中，Windows 生产 gate 尚未闭合 | [design.md](design.md) · [implementation · P6 生产级框架](areas/implementation.md#p6-生产级框架) |
| 文档合格化与契约收敛 | 结构、追踪、依赖/cfg、Result-only、wake/L1、caps 组合、证据等级与布局两阶段输入已在 2026-07-10 审计修复 | [requirements.md](requirements.md) · [design.md](design.md) · [#170–#174](decisions.md#d170) |
| 验证基线 | lib 1057/1057、demo 19/19、doc 3/3；三目标 compile、Clippy 与严格文档检查通过 | [implementation · 当前验证基线](areas/implementation.md#当前验证基线-2026-07-10) |
| 后续 backlog | 以 implementation 为权威清单 | [implementation · 后续工作](areas/implementation.md#后续工作) |

## 里程碑与工作域

| 里程碑 | 内容 | 下一步判定 |
|--------|------|------------|
| P0–P5 维护 | 主循环、热更新、App API、渲染窄路径、多窗、Handle 体系 | 新改动须同步系统文档与最小测试 |
| **P6 生产级框架** | **Windows 优先**生产可用 · 稳定性 · demo/docs 准确 · 生产阻塞差距；可组合渲染轴（[#169](decisions.md#d169)）与 D3D11 GPU raster；Linux/macOS parity 与 macOS 验证 **随后** | 按 [P6 退出门槛](#p6-退出门槛默认工程-gate) 与 [P6 生产级优先项](areas/implementation.md#生产级优先项) 排序推进 |
| **P7+ 移动端**（[#166](decisions.md#d166)） | iOS / Android 原生 backend；复用 `ui`/`app`/`draw` trait 契约 | P6 基线达标后切片；须新决策明确首批 OS 与图形栈 |
| **全栈扩展**（[#166](decisions.md#d166)） | 在 `data` 基线之上扩展网络/同步等能力 | 产品边界与 API 须人类决策后再排期 |
| 无障碍（屏幕阅读器桥） | v1 基线已落地；屏幕阅读器平台桥待后续 | 需要新设计决策或明确阶段切片 |
| 文档维护 | 保持 AGENTS、index、areas/architecture、implementation、areas/systems 同步 | 新目录必须补 `index.md`；新决策写入 [decisions.md](decisions.md) |

## P6 退出门槛（默认工程 gate）

在主人补充产品级 SLA/兼容矩阵前，Windows “生产可用”至少同时满足：

1. `cargo test --all-targets` 零失败；当前 lib 1057/1057、demo 19/19，历史两项红测、D3D11 复杂填充拓扑、颜色通道、GDI damage 越界、首帧 LayerTree/Picture 回退、WGL core loader/caps、Win32 多窗状态串用与 UTF-16 代理对错误已用代码与守卫修复，未使用豁免。
2. Windows 默认 D3D11 路径与无 GPU feature 的 SoftwareEngine 回退已完成首帧、颜色、按钮点击、最大化/恢复和关闭 smoke；输入/IME、主题切换和多窗仍待完整回归，因此本项未闭合。
3. 架构边界测试无真实违规，也不因注释/Rustdoc 文本产生假阳性。
4. P0 生产阻塞项归零；错误日志、graphics probe report 与资源 shutdown 路径可诊断。probe 候选/阶段/选中 API/完整错误已有自动化守卫；session 与 native window 幂等性已有分层测试，GL 资源 → context → native window 的组合顺序仍须真实 GUI smoke 与驱动矩阵验证。
5. 项目文档普通检查、`--strict-design`、本地锚点与旧路径扫描全部通过，公开示例和实现状态一致。

Linux/macOS parity 和移动端不属于 Windows gate，但不得通过上层平台分支换取 Windows 通过。

## 下一步

- **P6 优先（Windows 优先，[#167](decisions.md#d167)）**：Windows 端稳定性回归、生产阻塞项闭合、demo/docs 与实现同步；从 [implementation · 后续工作](areas/implementation.md#后续工作) 选取项时默认以 Windows 为主验证环境。
- **下一生产证据**：在已完成 D3D11/Software 基础 GUI smoke、WGL engine 绘制/readback 与 native IME/UTF-16 自动化上，补齐 Windows OpenGL ES 屏幕呈现、Input 焦点/预编辑/真实输入法、主题切换、多窗与 GPU/驱动矩阵，并留存可复核记录。
- **P6 代码优先（[#169](decisions.md#d169)）**：可组合渲染轴落地 — 1) D3D11 `GpuNative` × `Swapchain` 垂直切片（soft Canvas2D + RTV）✅；原生 D3D 几何着色器 backlog；2) `RasterMode` / `PresentMode` 类型与 caps + 表驱动装配 ✅；3) `BackendKind` 统一 ✅。详见 [implementation · P6.8](areas/implementation.md#p68-可组合渲染轴) · [graphics-backend-pluggable · 可组合渲染轴](areas/systems/graphics-backend-pluggable.md#可组合渲染轴)。
- **P6 随后**：Linux / macOS parity、macOS 真机验证 — 上层代码不变，仅完善或验收 `native` backend。
- **布局契约收敛**：父级上限违规、Card actions/body 重叠与 `WidgetLayout` pass-local `LayoutChild` 两阶段输入均已闭合（[#174](decisions.md#d174)）；Arrange 不再递归 measure，也不跨收敛轮缓存陈旧尺寸。
- 若改公开 API：同步 [areas/systems/public-api.md](areas/systems/public-api.md)、[glossary.md](glossary.md)，必要时追加 [decisions.md](decisions.md) #175+。

## 风险与阻塞

| 风险 | 处理 |
|------|------|
| 文档与源码漂移 | `docs` 为权威；实现按文档重构，完成后同步 implementation 与实现注记 |
| 后续项范围过大 | 按阶段切片验证，避免一次性跨域大改 |
| 平台能力差异 | **仅** `native` backend/factory 内分化；`core`/`draw`/`ui`/`app`/`data` 代码一致，上层通过 trait 和 `Result` 处理（[#167](decisions.md#d167)） |
| 零闲置规则被破坏 | 新主循环、Timer、事件、渲染路径必须过 #105 审查 |
| Windows 生产差距 | Windows 为当前主验证与交付环境；阻塞项优先在 Win 端闭合 |
| macOS 验证缺口 | AppKit/Metal 已编码；真机验收列入 P6 P1（Windows 基线达标后） |
| Clippy 与平台 warning 债务 | 当前 Clippy gate 通过但仍有既存 warning；分批治理，不把 warning-free 伪装成已完成 |

## 开放问题

- macOS 原生运行验证的具体验收标准待主人确认。
- 移动端（[#166](decisions.md#d166)）：首批目标 OS（iOS / Android 优先级）、图形栈与 `native` backend 切片待确认。
- 全栈（[#166](decisions.md#d166)）：网络/同步/API 客户端是否纳入 `data` 域或新域，待产品边界决策。
- 无障碍（屏幕阅读器桥）需要先形成产品边界与设计决策。
- Metal/D3D12 native raster 推进时机：D3D11 `GpuNative` 垂直切片已落地后可排；D3D11 原生几何着色器可并行深化。
- 已废弃 bundled 管线枚举（`RenderPipelineProfile`）已随 P6.8 正交轴 refactor **删除**（[#169](decisions.md#d169)）；D3D11 soft `GpuNative` × `Swapchain` ✅；原生 D3D 几何着色器仍 backlog。
