# UIX Demo GUI 全面测试报告

- 轮次：`20260711-082208-uix-demo`
- 时间：2026-07-11 08:22–08:41（Asia/Shanghai）
- 结论：**不通过**。发现 7 个已确认产品缺陷，其中 2 个 High、3 个 Medium、2 个 Low。
- Bug 清单：[docs/问题.md](../../docs/问题.md)

## 执行摘要

本轮在真实 Windows 窗口中完成了 11 个 Demo 页面逐页巡检，并覆盖启动、导航、响应式 State、主题、多窗口、文本输入、键盘焦点、滚动、浮层、Transfer、Upload、窄窗 resize、关闭副窗以及 D3D11/OpenGL ES 对照。

最严重问题是持续的黑块/空白帧：窗口在动态刷新时会在完整帧与大片黑色缺损帧之间反复切换。该问题经后台 Windows Graphics Capture、前台屏幕采样、进程重启以及 D3D11/OpenGL ES 两条后端路径交叉复现，排除了单次截图抖动。

## 基线与环境

| 项目 | 值 |
| --- | --- |
| Git HEAD | `16596b233e2446256c2075c0a076dc2f268c026a` |
| 二进制 | `target/debug/uix-demo.exe`，SHA-256 `E99BC2FD8A17C129E95A7DF1634DA299BE50BA8314FCDFA52144813699601A55` |
| 工作区 | 基于当前 dirty tree 构建；测试未覆盖或回退用户现有修改 |
| OS | Windows 11 专业版 `10.0.26200` |
| 显示 | 2560×1440，96 DPI，100% 缩放 |
| GPU | NVIDIA GeForce RTX 4070 Ti SUPER，D3D11 hardware |
| Rust | `rustc 1.96.0`，`cargo 1.96.0` |
| 渲染路径 | D3D11 主轮次；OpenGL ES 对照轮次 |
| 窗口 | 基线客户区 1200×800；窄窗客户区 640×480 |

构建门禁：

- `cargo test --bin uix-demo`：19 passed，0 failed。
- `cargo build --bin uix-demo`：通过。
- 构建有 3 个既存 `dead_code` warning，不影响启动。
- 后端日志：[D3D11](restart-stdout.log) · [OpenGL ES](opengles-stdout.log)

## 覆盖与结果

| 场景 | 结果 | 证据/说明 |
| --- | --- | --- |
| 启动与首帧 | Fail | 正常启动，但持续出现黑块帧 |
| 11 页侧栏导航 | Partial | 11 页均可到达，标题与正文同步；渲染缺损影响所有页面 |
| 首页 State | Pass | `+1` 从 0 更新到 1：[前](evidence/01-home-baseline.png) / [后](evidence/02-home-counter-plus.png) |
| Theme light/dark | Partial | 切换生效；暗色存在低对比固定亮色 token |
| 多窗口主题联动 | Pass | 副窗可创建、主题双向同步、关闭副窗后主窗继续工作：[暗色](evidence/19-theme-window-1.png) / [亮色](evidence/20-theme-window-light-1.png) / [主窗同步](evidence/21-main-theme-sync-2.png) |
| 文本输入与键盘焦点 | Fail | 获得焦点后文本不写入，Tab 不前移 |
| Transfer / Upload | Fail | Transfer 命中错行；Upload 过滤与结果冲突 |
| Overlay | Partial | Popover 可打开；Modal/Drawer 无 live 入口；Tooltip hover、Popconfirm 完整闭环未完成 |
| Resize 1200×800→640×480→恢复 | Fail | 壳层可 resize 和恢复，但主内容横向被裁且无访问路径 |
| D3D11 / OpenGL ES 对照 | Fail | 两条后端均出现真实黑块帧 |
| 稳定性 | Pass | 测试过程中无 crash、hang；主副窗生命周期正常 |

页面首屏证据：[应用能力](evidence/03-runtime.png) · [通用](evidence/04-common.png) · [布局](evidence/05-layout.png) · [导航](evidence/06-navigation.png) · [输入](evidence/07-input.png) · [数据展示](evidence/08-data.png) · [反馈](evidence/09-feedback.png) · [图表](evidence/10-charts.png) · [其他](evidence/11-other.png) · [覆盖清单](evidence/12-coverage.png)

## 已确认缺陷

### BUG-20260711-001 — GPU 窗口持续交替出现大片黑块/空白帧

- 严重度：High
- 置信度：高
- 范围：Windows 11；D3D11 hardware 与 OpenGL ES；主窗和主题副窗。
- 预期：动态 State/动画只更新对应内容，完整窗口始终连续可见。
- 实际：相邻帧在完整内容与大片黑色缺损区域之间反复切换，肉眼可见闪烁，部分帧几乎全黑。
- 最小复现：启动 Demo，停留任意页面，等待全局 tick/动画刷新并连续观察或采样窗口。
- 复现强度：D3D11 两次独立进程均复现；OpenGL ES 对照也复现；后台截图与 `CopyFromScreen` 均复现。
- 证据：D3D11 [完整帧](evidence/15-restart-screen-1.png) / [相邻黑块帧](evidence/15-restart-screen-2.png)；OpenGL ES [完整帧](evidence/34-opengles-screen-1.png) / [相邻黑块帧](evidence/34-opengles-screen-2.png)；副窗 [完整](evidence/19-theme-window-1.png) / [黑帧](evidence/19-theme-window-2.png)。
- 建议定位：优先检查双缓冲/交换链、partial damage 与每帧 clear/preserve 契约，不要先在截图层规避。

### BUG-20260711-002 — 输入控件键盘路径失效

- 严重度：High
- 置信度：高
- 范围：输入页自绘 `Input`；Windows 前台键盘输入。
- 预期：点击 Input 后可输入 ASCII/中文；Tab 将焦点移到下一个可聚焦控件。
- 实际：焦点边框和 caret 已出现，但发送 `Test中文123` 或连续 `A` 后仍为空；Tab 后焦点仍停在第一个 Input。
- 最小复现：输入页点击第一个 `Small...` → 输入文本或按 `A` → 按 Tab。
- 证据：[Unicode/ASCII 输入后仍为空](evidence/22-input-text-2.png) · [连续 A 后仍为空](evidence/23-input-key-a-1.png) · [Tab 后仍在首个 Input](evidence/24-input-tab-1.png)。
- 备注：分别用 Unicode SendInput 与单键 virtual-key 两条输入路径复现，不是单一文本注入方式失效。

### BUG-20260711-003 — 暗色主题存在不可读的固定浅色主题文本

- 严重度：Medium
- 置信度：高
- 范围：暗色主题，应用能力页及其他直接使用固定 `DesignTokens::antd_light()` 解析色的节点。
- 预期：暗色下所有正文、标签、说明和卡片内容保持可读对比度。
- 实际：`全局 tick 计数`、`PulseRing`、`BounceBall`、`Card = 静态候选` 等文字接近黑色，落在深色背景上几乎不可读。
- 最小复现：首页右上角切暗色 → 进入应用能力 → 查看顶部和滚动到底部。
- 证据：[顶部低对比文字](evidence/17-runtime-top-1.png) · [底部低对比文字](evidence/18-runtime-bottom-2.png)。

### BUG-20260711-004 — Transfer 可见行与点击命中行错位

- 严重度：Medium
- 置信度：高
- 范围：其他页 `Transfer` 左右列表。
- 预期：点击“选项 A”行只切换 A。
- 实际：点击 A 的可见行中心后 B 被选中，原有 C 保持选中；随后移动会把 B/C 移到右侧。
- 最小复现：进入其他页 → 点击左列表第一行“选项 A”中心。
- 证据：[初始 C 选中](evidence/11-other.png) · [点击 A 后 B/C 选中](evidence/26-transfer-click-a-2.png) · [错误选择被移动](evidence/27-transfer-move-1.png)。

### BUG-20260711-005 — 640×480 窄窗下主内容横向裁切且不可达

- 严重度：Medium
- 置信度：高
- 范围：Demo 多个固定宽页面；本轮以其他页复现。
- 预期：窗口变窄时内容重排、缩放，或提供可见横向滚动/访问路径。
- 实际：Transfer 右侧列表与中间按钮被裁到客户区外，Upload 也只剩左侧部分；主页面只有垂直滚动，无法访问被裁区域。
- 最小复现：进入其他页 → 将客户区从 1200×800 缩至 640×480。
- 证据：[640×480 完整窗口](evidence/29-narrow-640x480-1.png)。

### BUG-20260711-006 — Upload 的 accept 声明与实际新增文件扩展名冲突

- 严重度：Low
- 置信度：高
- 范围：其他页 Upload Demo。
- 预期：界面声明“筛选 .png,.jpg”时，只接受或模拟这两类文件。
- 实际：点击上传区后新增 `upload_1.txt`。
- 最小复现：进入其他页 → 点击 Upload 区。
- 证据：[新增 upload_1.txt](evidence/28-upload-click-2.png)。

### BUG-20260711-007 — 反馈页将 Modal/Drawer 标记为已覆盖但没有可操作入口

- 严重度：Low
- 置信度：高
- 范围：反馈页 Demo 覆盖完整性。
- 预期：标记 `Modal ✓` / `Drawer ✓` 的 live demo 提供按钮或其他触发器，可观察打开、关闭、遮罩与焦点恢复。
- 实际：两个条目只有标签和勾选，无 trigger；滚动到该区也无法打开控件。
- 最小复现：进入反馈页 → 滚动到 `Overlay — Modal / Drawer`。
- 证据：[空白 Modal/Drawer 行](evidence/30-feedback-bottom-1.png)。

## 通过项与非缺陷

- 11 个页面均可通过固定侧栏到达，active 高亮、页名和正文标题一致。
- 首页 State `+1` 正常更新。
- 明暗主题切换生效；主题副窗继承当前主题，并可从副窗反向同步主窗。
- Popover 能正确显示在触发器上方，未被主 ScrollView 裁切：[证据](evidence/31-popover-open-1.png)。
- Transfer 的已选项左右移动逻辑本身可工作；缺陷集中在行命中偏移。
- 窗口从窄窗恢复后未崩溃，页面继续响应。

## 限制与待补矩阵

- UIX 为自绘 GUI，UIA 无法稳定暴露内部控件；本轮用窗口精确定位、窗口后台截图和受控前台输入完成验证。
- 工具当前无原生 hover、drag、wheel、resize、close；wheel/resize/副窗关闭使用精确 HWND 的 Win32 限域补充，未操作其他应用。
- Tooltip hover、Splitter drag、嵌套滚动边界、Select 100/TreeSelect 80 的完整键盘闭环未完成。
- 未覆盖 CPU software、D3D12、150%/200% DPI、Linux、macOS 和屏幕阅读器平台桥；本报告不把 Windows 结果外推为跨平台结论。
- Popconfirm 在 Popover 已打开时的后续点击未形成稳定闭环，未登记缺陷。
