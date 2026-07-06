# UIX 设计文档

这里存放 UIX 的系统级设计文档。设计按“系统”组织，而不是按源码目录组织。

## 从哪里开始

| 用途 | 文档 |
|------|------|
| 看设计总目录 | [`Main.md`](Main.md) |
| 查设计决策 | [`decisions.md`](decisions.md) |
| 查核心术语 | [`glossary.md`](glossary.md) |
| 看维护规则与源码边界 | [`../AGENTS.md`](../AGENTS.md) |

## 目录结构

```text
docs/
├── Main.md              # 设计文档总入口
├── decisions.md         # 决策台账，追加编号从 #101 开始
├── glossary.md          # 核心术语表
├── README.md            # 本文件
└── systems/             # 各系统设计正文
```

## 维护原则

- 系统正文写定稿行为、边界和跨系统关系。
- 决策编号只在 `decisions.md` 分配；系统文档只引用。
- 源码边界和硬约束写在 `../AGENTS.md`；运行数据流写在相关系统文档。
- 新增系统文档时，先同步 `Main.md` 的系统全景和阅读顺序。
- 新增或重命名术语时，同步 `glossary.md`。
