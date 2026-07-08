# 当前计划

← [index](index.md)

本文只记录项目级状态和下一步；详细能力清单、阶段摘要与源码目录映射见 [areas/implementation.md](areas/implementation.md)。

## 当前状态

| 工作流 | 状态 | 依据 |
|--------|------|------|
| P0–P5 按需零闲置主体 | 已落地 | [implementation · 已落地阶段](areas/implementation.md#已落地阶段p0p5) |
| P6 多图形后端 | 部分已落地，仍有规划项 | [implementation · P6 图形后端](areas/implementation.md#p6-图形后端) |
| 文档结构合格化 | 已纳入项目级入口 | [index.md](index.md) 与各分区索引 |
| 后续 backlog | 以 implementation 为权威清单 | [implementation · 后续工作](areas/implementation.md#后续工作) |

## 里程碑与工作域

| 里程碑 | 内容 | 下一步判定 |
|--------|------|------------|
| P0–P5 维护 | 主循环、热更新、App API、渲染窄路径、多窗、Handle 体系 | 新改动须同步系统文档与最小测试 |
| P6 图形后端 | D3D11/Vulkan/配置已落地，D3D12/Metal/更完整 native GPU renderer 待推进 | 推进前读 [rendering](areas/systems/rendering.md)、[platform](areas/systems/platform.md)、[#162](decisions.md#d162) |
| 无障碍 v2 | ARIA、屏幕阅读器、键盘导航扩展 | 需要新设计决策或明确阶段切片 |
| 文档维护 | 保持 AGENTS、index、areas/architecture、implementation、areas/systems 同步 | 新目录必须补 `index.md`；新决策写入 [decisions.md](decisions.md) |

## 下一步

- 若继续实现 P6：先收敛 D3D12 / Metal / native GPU renderer 的具体阶段目标，再改代码。
- 若修实现差距：从 [implementation · 后续工作](areas/implementation.md#后续工作) 选一项，打开对应系统文档，不通读全部 docs。
- 若改公开 API：同步 [areas/systems/public-api.md](areas/systems/public-api.md)、[glossary.md](glossary.md)，必要时追加 [decisions.md](decisions.md) #163+。

## 风险与阻塞

| 风险 | 处理 |
|------|------|
| 文档与源码漂移 | `docs` 为权威；实现按文档重构，完成后同步 implementation 与实现注记 |
| 后续项范围过大 | 按阶段切片验证，避免一次性跨域大改 |
| 平台能力差异 | 仅在 native backend/factory 内分化，上层通过 trait 和 `Result` 处理 |
| 零闲置规则被破坏 | 新主循环、Timer、事件、渲染路径必须过 #105 审查 |

## 开放问题

- D3D12、Metal 与更完整 native GPU renderer 的推进顺序待主人确定。
- 无障碍 v2 需要先形成产品边界与设计决策。
- Handler 宏层 fingerprint 仍需独立设计切片；Computed 已具备独立 slot id，后续仅按回归维护。
