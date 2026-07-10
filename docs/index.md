# UIX 项目文档入口

← [AGENTS.md](../AGENTS.md)

本页是项目级路由图；编码前硬约束仍以 [AGENTS.md](../AGENTS.md) 为准，系统设计导航使用 [areas/architecture.md](areas/architecture.md)。不要通读 `docs/`，按任务进入相邻索引或叶子文档。

## 核心文档

| 文档 | 何时阅读 |
|------|----------|
| [project.md](project.md) | **首次了解 UIX 为什么存在** — 产品愿景（跨平台含移动端 + 全栈）、开发策略（Windows 优先）、当前阶段 vs 目标、范围、约束和成功标准 |
| [requirements.md](requirements.md) | 查询编号需求、优先级、当前满足状态和验收检查 |
| [design.md](design.md) | 阅读跨系统总设计、需求追踪、替代方案、质量属性、rollout、风险和开放问题 |
| [plan.md](plan.md) | 查看当前状态、里程碑、下一步、风险和开放问题 |
| [areas/architecture.md](areas/architecture.md) | 按任务定位架构、系统设计、API、术语和实现状态 |
| [areas/implementation.md](areas/implementation.md) | 对照已落地能力、后续 backlog 和源码目录映射 |
| [decisions.md](decisions.md) | 查询或追加持久设计决策；新决策从 #176+ 起 |
| [glossary.md](glossary.md) | 查询术语、设计名和源码名对照 |

## 分区索引

| 分区 | 内容 |
|------|------|
| [systems/index.md](areas/systems/index.md) | 14 个核心系统设计文档 + P6 专项（graphics-backend-pluggable）索引 |
| [areas/index.md](areas/index.md) | 当前项目工作域索引；具体技术域由 `systems/` 承载 |
| [references/index.md](references/index.md) | 可复用参考资料索引；当前主要引用仓内规范文档 |

## 维护规则

- 改硬约束、核心理念或 AI 工作流：同步 [AGENTS.md](../AGENTS.md)。
- 改产品需求或验收：同步 [requirements.md](requirements.md)；改跨系统方案或质量属性：同步 [design.md](design.md)。
- 新增/删除系统：只在 [systems/index.md](areas/systems/index.md) 维护系统清单；仅当任务路由或跨系统主题变化时更新 [areas/architecture.md](areas/architecture.md)。
- 改实现状态、backlog 或源码路径：同步 [plan.md](plan.md)、[areas/implementation.md](areas/implementation.md) 与对应系统文档的实现注记。
- 新增活跃文档目录时必须添加该目录的 `index.md`，并确保能从本页或相邻索引到达。
