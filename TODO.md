# UIX 项目待办清单

> 根据项目文档、当前实现结构和公开 API 测试边界整理。
>
> 当前仓库状态：`main` 与 `origin/main` 一致，无未提交源码改动。

## 已确认完成边界

- UIX Lang AOT 链路、`TypedUiIr`、SourceMap、CLI/LSP 与纯 UI 热重载 Adapter 已落地。
- 项目测试只允许外部消费者通过公开 Rust API 或公开 UIX Lang 入口执行；私有实现不建立测试。
- 图形共享管线、Vulkan、Linux OpenGL ES 已有验收证据；Windows D3D11 真实运行仍暂缓。
- `0.0.1` 仍处于开发中，尚未正式发布。

## 待办

### P0：发布前门禁

- [ ] **清理全部非公开 API 测试**
  - 只保留 `tests/*_public_api.rs`，且测试代码只能导入公开门面或编译公开 UIX Lang 用法。
  - 删除或停用 `tests/unit/`、Python 源码扫描、私有集成测试、结构门禁、测试专用 harness、GPU parity、真窗视觉、性能及打包测试；不得换名保留。
  - 通过条件：Cargo 只发现 `*_public_api` 测试目标，仓库不再包含针对私有 Module、Component、内部算法、状态机或物理目录的测试。

- [x] **冻结 0.0.1 平台矩阵**
  - 产品文档、使用文档、根 `Cargo.toml` 默认 feature、Demo feature 与 registry 优先级已统一。
  - Windows/Linux 统一首选 Vulkan；Windows 保留 D3D11、Linux/Wayland 保留 OpenGL ES 兼容回退。
  - 依据：[交付与许可](docs/产品/交付与许可.md)、[图形后端状态](docs/架构/graphics/backend.md)、[Cargo.toml](Cargo.toml)。
  - 通过条件：公开 feature 与后端选择入口由外部消费者测试锁定；提交后同步到 Gitea Issue #1。

- [ ] **完成候选包与发布闭环**
  - Linux 候选包已在真实 Vulkan 与 Wayland/OpenGL ES 环境完成两次确定性构建、独立校验和 GPU/WSI 验收。
  - Windows 只运行 `scripts/build_internal_release.ps1` 生成并校验候选 ZIP；图形结果由公开 API 外部消费者单独记录，不使用内部 surface 回读或 parity harness。
  - 签名边界、私有 Gitea Release 分发、并行版本升级和保留旧制品/Settings 的回滚方案已冻结。
  - 通过条件：候选包校验通过，发布状态、制品和环境证据已登记到 Gitea Issue #1。

### P1：下一阶段工程任务

- [ ] **完成 Windows D3D11 公开 API 验收**
  - 只允许外部应用通过公开 `GraphicsBackend::Direct3D11` 配置、启动并观察公开成功或 typed failure。
  - 不使用 `d3d11-parity-test`、`test-harness`、RHI、FramePlan、设备丢失注入或其他内部入口。
  - 通过条件：公开 API 消费者在真实 Windows 环境得到文档承诺的结果，并在 Gitea 记录环境与结果。

- [x] **实现 UIX Lang 真正的增量编译**
  - `CompilerSession` 持有会话级 Syntax、Semantic、check 与 Emit 阶段缓存；LSP 和热重载 Adapter 复用同一会话，并在文档关闭或 Adapter 销毁时释放缓存。
  - 单文件内容未变时复用 AST 或稳定语法诊断；完整 `CompilationKey` 未变时复用分析及输出，依赖 overlay 变化只重解析变化文件并失效对应源码图。
  - 缓存阶段、计数、失效图与 lowering 都是内部实现，不建立测试；只通过公开编译、检查与热重载入口观察结果是否保持一致。
  - 依据：[语言完善计划](docs/uix-lang/变更/语言完善计划.md)、[编译器架构](docs/uix-lang/设计/编译器架构.md)。

- [ ] **建立诊断质量验收矩阵**
  - 覆盖 AOT、`check`、LSP、递归导入、SourceMap 和生成错误。
  - 只从公开 AOT、`check`、LSP 与真实宏消费者入口测试稳定错误 code、SourceSpan 和 suggestion，不访问解析器、缓存或 lowering 私有状态。
  - 通过条件：同一无效输入经公开入口返回一致的主诊断身份。

### P2：架构与条件性未来任务

- [ ] **收敛 SMC 架构治理遗留**
  - 明确 `core`、`bus` 的 System 定级，清理迁移路由和过时链接。
  - 依据：[系统列表](docs/架构/系统列表.md)。

- [ ] **补齐平台 EventBus 生命周期契约**
  - 明确关闭、重入、注册/注销和失效句柄语义；该能力属于 System 私有边界，不建立项目测试。
  - 依据：[路由清单](docs/架构/路由清单.md)。

- [ ] **决定 D3D12 等未来 backend 的去留**
  - 当前 D3D12 thin RHI 明确返回未实现错误，registry 也保留 Planned 状态。
  - 若纳入后续路线，补齐 RHI、registry、真实验收和文档；否则保持 capability gate 并明确为非 0.0.1 目标。

## 明确不列入当前待办

- UIX Lang 的 Service、异步 action、业务工作流、路由决策和生命周期语言化；这些职责继续由 Rust 持有。
- 移动端 backend、macOS 首发支持及远程控制；当前产品范围未承诺。
- MIME `accept`、状态伪类启动动画、关键帧主题 token/百分比；当前文档和实现均明确标注为暂不支持，除非产品范围变更。
