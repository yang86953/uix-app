# UIX 项目文档入口

← [AGENTS.md](../AGENTS.md)

本页是项目级路由图；编码前硬约束仍以 [AGENTS.md](../AGENTS.md) 为准，系统设计导航使用 [areas/architecture.md](areas/architecture.md)。不要通读 `docs/`，按任务进入相邻索引或叶子文档。

## 核心文档

| 文档 | 何时阅读 |
|------|----------|
| [project.md](project.md) | 了解项目目的、范围、约束和成功标准 |
| [plan.md](plan.md) | 查看当前状态、里程碑、下一步、风险和开放问题 |
| [areas/architecture.md](areas/architecture.md) | 按任务定位架构、系统设计、API、术语和实现状态 |
| [areas/implementation.md](areas/implementation.md) | 对照已落地能力、后续 backlog 和源码目录映射 |
| [decisions.md](decisions.md) | 查询或追加持久设计决策；新决策从 #163+ 起 |
| [glossary.md](glossary.md) | 查询术语、设计名和源码名对照 |

## 分区索引

| 分区 | 内容 |
|------|------|
| [systems/index.md](areas/systems/index.md) | 14 个系统设计文档的目录索引 |
| [areas/index.md](areas/index.md) | 当前项目工作域索引；具体技术域由 `systems/` 承载 |
| [references/index.md](references/index.md) | 可复用参考资料索引；当前主要引用仓内规范文档 |

## 维护规则

- 改硬约束、核心理念或 AI 工作流：同步 [AGENTS.md](../AGENTS.md)。
- 改系统设计或架构导航：同步 [areas/architecture.md](areas/architecture.md) 与最近的 [systems/index.md](areas/systems/index.md) 条目。
- 改实现状态、backlog 或源码路径：同步 [plan.md](plan.md)、[areas/implementation.md](areas/implementation.md) 与对应系统文档的实现注记。
- 新增活跃文档目录时必须添加该目录的 `index.md`，并确保能从本页或相邻索引到达。
