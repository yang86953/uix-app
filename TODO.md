# UIX 项目待办清单

> 根据项目文档、当前实现结构和针对性测试整理。
>
> 当前仓库状态：`main` 与 `origin/main` 一致，无未提交源码改动。

## 已确认完成边界

- UIX Lang AOT 链路、`TypedUiIr`、SourceMap、CLI/LSP 与纯 UI 热重载 Adapter 已落地。
- 文档链接检查 9/9 通过；`uix-lang-compiler` 测试 502 项通过；两个真实宏消费者测试通过。
- 图形共享管线、Vulkan、Linux OpenGL ES 已有验收证据；Windows D3D11 真实运行仍暂缓。
- `0.0.1` 仍处于开发中，尚未正式发布。

## 待办

### P0：发布前门禁

- [x] **冻结 0.0.1 平台矩阵**
  - 产品文档、使用文档、根 `Cargo.toml` 默认 feature、Demo feature 与 registry 优先级已统一。
  - Windows/Linux 统一首选 Vulkan；Windows 保留 D3D11、Linux/Wayland 保留 OpenGL ES 兼容回退。
  - 依据：[交付与许可](docs/产品/交付与许可.md)、[图形后端状态](docs/架构/graphics/backend.md)、[Cargo.toml](Cargo.toml)。
  - 通过条件：唯一平台/feature/验收矩阵已由自动化契约冻结；提交后同步到 Gitea Issue #1。

- [ ] **完成候选包与发布闭环**
  - Linux 候选包已在真实 Vulkan 与 Wayland/OpenGL ES 环境完成两次确定性构建、独立校验和 GPU/WSI 验收。
  - Windows 运行 `scripts/accept_windows_release.ps1`，一次完成真实 Vulkan/D3D11、自动 surface 回读、两次确定性 ZIP 与环境证据。
  - 明确签名、分发、升级和回滚方案。
  - 通过条件：候选包校验通过，发布状态、制品和环境证据已登记到 Gitea Issue #1。

### P1：下一阶段工程任务

- [ ] **完成真实 Windows D3D11 验收**
  - 在真实 Windows 桌面环境执行：
    `cargo test --no-default-features --features d3d11-parity-test --test d3d11_gpu_parity -- --nocapture`
  - 覆盖 11 类 pipeline、Blur、present、resize、设备丢失与恢复。
  - 通过条件：保留实际设备、驱动和运行结果；静态链接或交叉编译不能替代运行证据。

- [ ] **实现 UIX Lang 真正的增量编译**
  - 当前已有 `CompilationKey` 和递归依赖摘要，但编译入口仍会重新执行分析、降低和生成。
  - 为 Compiler System/LSP 增加阶段缓存与会话复用。
  - 通过条件：只失效受影响依赖；源码未变返回稳定结果；诊断顺序和失败语义不变。
  - 依据：[语言完善计划](docs/uix-lang/变更/语言完善计划.md)、[编译器架构](docs/uix-lang/设计/编译器架构.md)。

- [ ] **建立诊断质量验收矩阵**
  - 覆盖 AOT、`check`、LSP、递归导入、SourceMap 和生成错误。
  - 补充稳定错误 code、SourceSpan、suggestion 的测试，尤其覆盖 `compile_file`、`check_file` 和真实宏消费者。
  - 通过条件：同一无效输入在 AOT、`check`、LSP 中返回一致的主诊断身份。

### P2：架构与条件性未来任务

- [ ] **收敛 SMC 架构治理遗留**
  - 明确 `core`、`bus` 的 System 定级，清理迁移路由和过时链接。
  - 依据：[系统列表](docs/架构/系统列表.md)。

- [ ] **补齐平台 EventBus 生命周期契约**
  - 明确关闭、重入、注册/注销和失效句柄语义，补充对应测试。
  - 依据：[路由清单](docs/架构/路由清单.md)。

- [ ] **决定 D3D12 等未来 backend 的去留**
  - 当前 D3D12 thin RHI 明确返回未实现错误，registry 也保留 Planned 状态。
  - 若纳入后续路线，补齐 RHI、registry、真实验收和文档；否则保持 capability gate 并明确为非 0.0.1 目标。

## 明确不列入当前待办

- UIX Lang 的 Service、异步 action、业务工作流、路由决策和生命周期语言化；这些职责继续由 Rust 持有。
- 移动端 backend、macOS 首发支持及远程控制；当前产品范围未承诺。
- MIME `accept`、状态伪类启动动画、关键帧主题 token/百分比；当前文档和实现均明确标注为暂不支持，除非产品范围变更。

## 验证记录

- 文档链接：`/home/yang/.local/bin/rtk test python tests/test_check_docs_links.py -q` → 9 passed。
- 编译器：`/home/yang/.local/bin/rtk cargo test -p uix-lang-compiler` → 502 passed。
- 宏消费者：`/home/yang/.local/bin/rtk cargo test --test uix_lang_app_consumer --test uix_lang_action_consumer` → 2 passed。
