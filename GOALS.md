# UIX Project Goals

> 项目愿景："最小占用、最高性能、最丰富表现力、最灵活"
> Rust 原生 UI 框架 · 纯软件渲染 · Wayland/Win32

## 完成标准（全部达成后终止 AI 自动工作）

### Phase 1 — 核心稳定 ✅ (当前状态)
- [x] 四层架构（app/ui/graphics/platform）完整
- [x] SoftwareEngine + FrameGraph 3-Pass 渲染管线
- [x] 30+ 基础 Widget（button/card/input/tabs/table/scroll_view/chart…）
- [x] fontdue 纯 Rust 字体系统（零扫描、fc-match 发现）
- [x] 主题 Token 系统（color/spacing/typography）
- [x] Wayland SHM 后端可用
- [x] Win32 GDI 后端可用
- [x] 状态管理（State<T>/Computed<T>）
- [x] 依赖注入容器
- [x] 诊断系统（log/recovery/circuit breaker/collector）
- [x] 动画系统（Easing/Tween/Animation）

### Phase 2 — 生产就绪 ⬅️ 当前目标
- [ ] Platform: Wayland 窗口大小调整、最小化/最大化事件完成
- [ ] Platform: KDE 字体探测适配（非 GNOME 环境）
- [ ] Widget: 表单组件完整（select/radio/rate/segmented 完整交互）
- [ ] Widget: Dropdown/Popover/Tooltip 弹出层定位正确
- [ ] Layout: ScrollView 滚动到底无死角
- [ ] Layout: 响应式布局（Flexbox/Grid）可工作
- [ ] Graphics: 像素缓冲滚动优化（scroll_region memmove）
- [ ] Graphics: FrameGraph 编译优化（零帧 Idle 跳过）
- [ ] Theme: 运行时主题切换（DynTokens）
- [ ] 无 unwrap/expect 生产代码
- [ ] 每个文件 ≤400 行
- [ ] 所有已有功能 cargo check 通过

### Phase 3 — 完善
- [ ] 完整的 Widget 文档
- [ ] Integration test 覆盖
- [ ] 性能 benchmark（FPS/内存/启动时间）
- [ ] 示例应用（dashboard/playground）
- [ ] macOS 平台抽象（可选）
- [ ] 硬件加速渲染后端（可选）

### Phase 4 — 产品化
- [ ] 用 uix-app 开发一个真实产品
- [ ] 发布到 crates.io
- [ ] API 稳定性达到 0.x

## 终止条件

当 Phase 2 全部完成时，项目规划师（pro 模型）自动：
1. 确认所有 Phase 2 标准达到
2. 停用所有 uix-app 相关的 Hermes cron 任务
3. 向主人发送完成报告

## 编码规范（AI 自动工作必须遵守）

- 组合优于继承，trait 抽象 + 依赖注入
- 禁止 unwrap() / expect()（用 Result + ?）
- 禁止 unsafe（除非平台 FFI，且必须加 SAFETY 注释）
- 禁止全局可变状态
- 每个文件 ≤400 行，每个函数 ≤50 行
- 强类型消除非法状态（make illegal states unrepresentable）
- 中文注释
- 每个 commit 关联 Plane 任务编号
