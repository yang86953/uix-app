# 性能决策记录

[← 返回文档中心](../README.md)

本目录登记 UIX 已落地的性能与内存优化决策记录。每份记录统一包含：场景边界、可复核命令、剖析归因、变更内容与 A/B 对照数据，并声明各自的 SMC 边界与停止边界；这些记录是内部实现的证据档案，不是公开 API 契约，也不建立项目测试（测试边界见[公开 API 测试](../../README.md#公开-api-测试)）。

> **接口**：本页只做导航索引。各记录中的量化结果、命令与环境由对应正文持有；架构层的性能要求映射见[架构 · 性能与节能架构索引](../架构.md#性能与节能架构索引)。

## 使用限制

- 记录中的复现命令依赖内部工具包装器（如 `rtk cargo`、`rtk proxy`），在外部环境不可直接复制。
- 编号按登记顺序分配，不保证连续；本页列出的即当前仓库内已登记的全部记录。
- 结果是登记时点的测量证据，不代表后续版本仍然保持同一数值。
- 部分记录引用的非公开 API 测试（内部分配合同套件与 `tests/unit` 单元层）已随测试面收敛移除；正文中的对应命令仅作登记时点的方法存档，不再是可执行入口，其中 GPU parity harness 实现迁移至 `tests/support/native/gpu_parity/`。

## 记录索引

### UIX Lang 编译链路

| 记录 | 场景摘要 |
|---|---|
| [UIX-PERF-001](UIX-PERF-001.md) | `CompilerSession` 多文件 overlay 持续变更下的首轮性能与内存优化 |
| [UIX-PERF-002](UIX-PERF-002.md) | 同场景的峰值存活内存优化 |
| [UIX-PERF-003](UIX-PERF-003.md) | 跨编译场景热点再评估 |
| [UIX-PERF-004](UIX-PERF-004.md) | `CheckOutput` 兼容所有权优化 |
| [UIX-PERF-005](UIX-PERF-005.md) | PERF-004 之后的热点再评估 |

### UI 运行时热路径

| 记录 | 场景摘要 |
|---|---|
| [UIX-PERF-006](UIX-PERF-006.md) | 自定义组件快照分派快路 |
| [UIX-PERF-007](UIX-PERF-007.md) | 空处理器协调快路 |
| [UIX-PERF-008](UIX-PERF-008.md) | 空渲染 sidecar 清理快路 |
| [UIX-PERF-009](UIX-PERF-009.md) | 空焦点句柄 sidecar 清理快路 |
| [UIX-PERF-010](UIX-PERF-010.md) | 根布局失效覆盖快路 |
| [UIX-PERF-011](UIX-PERF-011.md) | 声明样式原位借用评估 |
| [UIX-PERF-012](UIX-PERF-012.md) | 稳定 keyed 预检单次身份查询 |
| [UIX-PERF-013](UIX-PERF-013.md) | 协调器快照复用门禁 |
| [UIX-PERF-014](UIX-PERF-014.md) | 空结构 State 绑定快路 |
| [UIX-PERF-015](UIX-PERF-015.md) | 复用类型前置消除专属 owner 查询 |
| [UIX-PERF-016](UIX-PERF-016.md) | 单次可变访问合并上下文查询 |
| [UIX-PERF-017](UIX-PERF-017.md) | 复用 incoming TypeId 减少 owner 动态分派 |
| [UIX-PERF-018](UIX-PERF-018.md) | 按禁用状态快路跳过无障碍名称副本 |
| [UIX-PERF-022](UIX-PERF-022.md) | 无 key 同序声明按位置直接复用 |
| [UIX-PERF-023](UIX-PERF-023.md) | 焦点空注册表删除短路 |
| [UIX-PERF-025](UIX-PERF-025.md) | ProviderContext 共享快照身份快返 |
| [UIX-PERF-026](UIX-PERF-026.md) | RenderHandler 空 sidecar 查询短路 |
| [UIX-PERF-027](UIX-PERF-027.md) | Layout 根条目提示跳过重复哈希 |
| [UIX-PERF-028](UIX-PERF-028.md) | 可搜索 Select 稳态行计数快路 |
| [UIX-PERF-029](UIX-PERF-029.md) | 布局有效可见性门控评估与测量校准（不实施） |
| [UIX-PERF-030](UIX-PERF-030.md) | 动画延续帧 cadence 门控（页面切换卡顿修复） |

### 原生图形资源

| 记录 | 场景摘要 |
|---|---|
| [UIX-PERF-031](UIX-PERF-031.md) | Vulkan 像素上传按需申请、截图读回缓冲及时回收 |
| [UIX-PERF-032](UIX-PERF-032.md) | 软件动态扩展引擎参考负载基线（冷装载 / 命令延迟 / 解释器步速） |
