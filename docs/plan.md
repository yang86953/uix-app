# 当前计划

← [index](index.md)

本文只记录项目级状态和下一步；详细能力清单、阶段摘要与源码目录映射见 [areas/implementation.md](areas/implementation.md)。

## 当前状态

| 工作流 | 状态 | 依据 |
|--------|------|------|
| P0–P5 按需零闲置主体 | 已落地 | [implementation · 已落地阶段](areas/implementation.md#已落地阶段p0p5) |
| **P6 生产级框架** | 设计完成；**Windows 优先**生产可用，随后 Linux/macOS parity 与验证 | [implementation · P6 生产级框架](areas/implementation.md#p6-生产级框架) |
| 文档结构合格化 | 已纳入项目级入口 | [index.md](index.md) 与各分区索引 |
| 后续 backlog | 以 implementation 为权威清单 | [implementation · 后续工作](areas/implementation.md#后续工作) |

## 里程碑与工作域

| 里程碑 | 内容 | 下一步判定 |
|--------|------|------------|
| P0–P5 维护 | 主循环、热更新、App API、渲染窄路径、多窗、Handle 体系 | 新改动须同步系统文档与最小测试 |
| **P6 生产级框架** | **Windows 优先**生产可用 · 稳定性 · demo/docs 准确 · 生产阻塞差距；可组合渲染轴（[#169](decisions.md#d169)）与 D3D11 GPU raster；Linux/macOS parity 与 macOS 验证 **随后** | 按 [P6 生产级优先项](areas/implementation.md#生产级优先项) · [P6.8](areas/implementation.md#p68-可组合渲染轴) 排序推进 |
| **P7+ 移动端**（[#166](decisions.md#d166)） | iOS / Android 原生 backend；复用 `ui`/`app`/`draw` trait 契约 | P6 基线达标后切片；须新决策明确首批 OS 与图形栈 |
| **全栈扩展**（[#166](decisions.md#d166)） | 在 `data` 基线之上扩展网络/同步等能力 | 产品边界与 API 须人类决策后再排期 |
| 无障碍（屏幕阅读器桥） | v1 基线已落地；屏幕阅读器平台桥待后续 | 需要新设计决策或明确阶段切片 |
| 文档维护 | 保持 AGENTS、index、areas/architecture、implementation、areas/systems 同步 | 新目录必须补 `index.md`；新决策写入 [decisions.md](decisions.md) |

## 下一步

- **P6 优先（Windows 优先，[#167](decisions.md#d167)）**：Windows 端稳定性回归、生产阻塞项闭合、demo/docs 与实现同步；从 [implementation · 后续工作](areas/implementation.md#后续工作) 选取项时默认以 Windows 为主验证环境。
- **P6 代码优先（[#169](decisions.md#d169)）**：可组合渲染轴落地 — 1) D3D11 GPU native raster（Windows）；2) `RasterMode` × `PresentMode` **breaking 替换** `RenderPipelineProfile`；3) `BackendKind` 统一 + 表驱动 factory/engine 装配。详见 [implementation · P6.8](areas/implementation.md#p68-可组合渲染轴) · [graphics-backend-pluggable · 可组合渲染轴](areas/systems/graphics-backend-pluggable.md#可组合渲染轴)。
- **P6 随后**：Linux / macOS parity、macOS 真机验证 — 上层代码不变，仅完善或验收 `native` backend。
- 若改公开 API：同步 [areas/systems/public-api.md](areas/systems/public-api.md)、[glossary.md](glossary.md)，必要时追加 [decisions.md](decisions.md) #170+。

## 风险与阻塞

| 风险 | 处理 |
|------|------|
| 文档与源码漂移 | `docs` 为权威；实现按文档重构，完成后同步 implementation 与实现注记 |
| 后续项范围过大 | 按阶段切片验证，避免一次性跨域大改 |
| 平台能力差异 | **仅** `native` backend/factory 内分化；`core`/`draw`/`ui`/`app`/`data` 代码一致，上层通过 trait 和 `Result` 处理（[#167](decisions.md#d167)） |
| 零闲置规则被破坏 | 新主循环、Timer、事件、渲染路径必须过 #105 审查 |
| Windows 生产差距 | Windows 为当前主验证与交付环境；阻塞项优先在 Win 端闭合 |
| macOS 验证缺口 | AppKit/Metal 已编码；真机验收列入 P6 P1（Windows 基线达标后） |

## 开放问题

- macOS 原生运行验证的具体验收标准待主人确认。
- 移动端（[#166](decisions.md#d166)）：首批目标 OS（iOS / Android 优先级）、图形栈与 `native` backend 切片待确认。
- 全栈（[#166](decisions.md#d166)）：网络/同步/API 客户端是否纳入 `data` 域或新域，待产品边界决策。
- 无障碍（屏幕阅读器桥）需要先形成产品边界与设计决策。
- Metal/D3D12 native raster 推进时机：D3D11 GPU raster（[#169](decisions.md#d169)）完成后再排。
- `RenderPipelineProfile` 移除时机：与 P6.8 正交轴 refactor 同批 breaking 变更（[#169](decisions.md#d169)）。
