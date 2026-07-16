# UIX Demo 综合测试报告

结果：Partial。已复现的产品缺陷均已修复并通过相邻场景回归；第二轮额外发现并关闭 7 个公开边界缺陷。Windows 真实 Microsoft Pinyin composition 仍缺少可判定的前台证据，因此输入法生产验收不宣称完成。

环境：Windows 11、Rust 1.96.0、`HEAD f4ebec9` 加当前工作树；测试时间 2026-07-11（UTC+08:00）。GUI 验收使用实际 `uix-demo` 窗口；默认 Auto/D3D11 路径与显式 `UIX_GRAPHICS_BACKEND=opengles` 均单独启动。

## 覆盖与结果

| 场景 | 结果 | 直接证据 |
| --- | --- | --- |
| 自动化全量回归 | 通过 | `cargo test --all-targets`：lib **1181/1181**、demo **23/23**。本轮新增 Tab→文本输入、实际 glyph 跨 Typography 选择/复制、640×480 Other 页纵向滑块交互，以及 7 项公开边界回归。 |
| 公开参数与终态边界 | 通过 | Pagination/Descriptions 的 0 与极值、Image 零目标、VirtualList 的 `NaN`/无穷/极大 offset、Timer 自取消/queue teardown/zero interval、WindowSession teardown、AppRuntime root shutdown 均有最小回归。 |
| 文档、静态与差异门禁 | 通过 | `cargo test --doc`：**3 passed / 24 ignored**；Clippy 静态检查、`git diff --check`、`python scripts/check_docs_links.py` 均退出 0。Clippy 保留既有非阻塞 warning。 |
| 后端真实窗口矩阵 | 通过（Windows 单机） | D3D11 9 项真实 HWND/context 测试、D3D12 WARP 3 项、WGL/OpenGL ES 4 项通过；显式 D3D11、D3D12、OpenGL ES、Auto 启动均创建窗口并正常退出；`--no-default-features` demo 构建通过。 |
| D3D11 动态帧与主/副窗 | 通过（范围内） | 先前 39 秒 15 帧采样首/中/末完整：[01](evidence/d3d11-long-01.png)、[08](evidence/d3d11-long-08.png)、[15](evidence/d3d11-long-15.png)；本轮动态页面在相隔 10 秒的计时值 803/822 均为完整帧。 |
| OpenGL ES 动态帧与副窗 | 通过（范围内） | 显式 OpenGL ES 动态页连续采样 tick 33、44、56 均完整；打开“主题联动窗口”后，独立 `UIX Theme Window` 正常渲染，未见黑块或空白帧。 |
| D3D11 长会话资源 | 通过（15 分钟范围） | Auto/D3D11 会话持续运行超过 15 分钟；末段工作集约 199 MB、私有字节约 233 MB、句柄 751，专用 GPU 内存 54,583,296 B、总提交 GPU 内存约 62.5–62.9 MB，连续 64 秒采样无增长趋势。 |
| 当前构建后台渲染取证 | 通过（单机） | 经 `ai-computer-toolkit` 启动当前 `uix-demo`，Windows Graphics Capture 在不改变前台窗口的条件下得到完整 D3D11 帧：[首页](evidence/background-current-frame.png)、[输入页](evidence/background-input-page.png)。此前 Computer Use 的残缺帧未复现于独立背景捕获，归类为控制层窗口句柄漂移。 |
| 640×480 窄窗与滚动条 | 通过 | 将实际窗口调至 641×481：Other 页通过纵向滚动可达 Transfer 与 Upload；首页出现横向滑块，拖动后右侧“覆盖范围”卡片进入视口；横、纵滑块均可跟手拖动。 |
| 主题、文本选择与 resize | 通过 | 暗色“应用能力”页标题、说明和卡片文字可读；从 Heading 3 向上拖至 Heading 2，两行高亮；窗口从 641×481 到最大化均铺满客户区，未见黑边。 |
| Transfer / Upload / 反馈浮层 | 通过 | Transfer 首行选择与移动、Upload accept 模拟文件名、D3D11 Modal/Drawer 打开→Escape 关闭→重开均已实机复核；先前截屏证据保留在本报告的 `evidence/`。 |
| Input / Tab / IME | 部分通过 | ASCII 输入可见；FakePlatform 端到端 Tab→`TextInput` 回归通过。当前 UIA 只暴露自绘窗口和标题栏，后台工具不能定位内部 Input，且其键盘面仅支持有限按键；Computer Use 在刷新目标后仍因窗口句柄漂移拒绝滚轮。系统已安装中文输入法，但未形成可观察候选/提交会话；这是工具限制，不能据此确认或否定产品真实 composition。 |

`cargo fmt --all -- --check` 仍失败，但差异覆盖许多与本轮无关的既有文件（含 `demo/src/common/page.rs`、平台 IME、测试和控件文件）；为避免重排用户工作树，本轮未批量格式化。新增测试的格式独立检查正常，`demo/src/tests/gui.rs` 的既有 81–83 行仍触发仓库整体 rustfmt 差异。

## 发现、修复与回归

### QA-001 [Low] `NativeGpuCanvas2D::ensure_soft` 的 deny-level `.expect()`

将 `.expect()` 改为 `Option::get_or_insert_with`，保持 software fallback 惰性分配；最终 Clippy 与全量测试通过。

### QA-002 [Low] D3D11 反馈页 Overlay 不可见、被裁切或无法重开

- 关闭态 Modal 仅使用自身触发器的 hit-test 区；Overlay 成员变动提升 tree version 并全帧失效。
- `LayerTree` 将活动 Overlay 脱离普通裁剪树，root overlay 独立渲染并隔离 canvas；零布局 Overlay 不再被 frame-culling 跳过。
- Masked Drawer 按真实 surface 尺寸布局和标脏；D3D11 soft fallback 合成前恢复全表面 viewport/scissor。

Modal/Drawer 指针、动画、Overlay 拓扑、裁剪泄漏和 D3D11 scissor 回归与实机打开/关闭/重开均通过。

### 交互与布局回归补强

- `src/tests/app/event_loop/event_loop.rs`：验证焦点在第一个 Input 时，Tab 后文本只进入第二个 Input。
- `src/tests/ui/core/widget/tree_core.rs`：真实渲染 glyph cache 下，跨三段 Typography 拖选后复制得到完整跨节点文本。
- `demo/src/tests/gui.rs`：640×480 Other 页确认 Transfer/Upload 有正 frame、只出现纵向溢出，并以真实 PointerDown/Move/Up 拖动纵向滑块。

### QA-003 [High/Medium] 公开边界参数与终态清理

- `BUG-018`：interval 回调中 cancel 后会被重新注册，`Duration::ZERO` 会同 tick 空转；以共享 active 标记、cancel epoch 和最小 1 ms 间隔修复。
- `BUG-019`、`020`：窗口关闭跳过组件生命周期，根窗退出可遗留迟到副窗 session；现在 teardown tree 后关闭 engine，并以 `shutdown_all` 封闭/清理所有 session、timer、队列、theme 和 pending request。
- `BUG-021`、`022`：Pagination/Descriptions 的零值/极值公开参数可触发除零或算术溢出；入口统一归一和夹紧。
- `BUG-023`、`024`：零尺寸图像目标和 VirtualList 的无穷/极大滚动输入可触发除零/溢出；前者 no-op，后者拒绝非有限值并饱和夹紧。

## 缺陷台账结论

- 本轮关闭：`BUG-20260711-001`、`003`、`005`、`008`、`010`–`014`、`018`–`024`；此前已关闭的 `004`、`006`、`007` 维持关闭。
- 保持待验证：`BUG-20260711-002`。需要真实 Microsoft Pinyin composition 会话（输入、候选提交、Tab 焦点遍历）的前台证据；本轮没有把控制层注入限制误记成产品缺陷。

## 剩余风险

- 本次结论只覆盖当前 Windows 单机与单 GPU；多 GPU、不同驱动、50%/200% DPI、Linux/macOS 仍需各自实机验收。
- 15 分钟资源采样不能替代 24 小时压力或生产工作负载。
- Microsoft Pinyin/等价系统 IME 的真实 composition 仍是唯一保留的功能验收项。
- 自绘控件当前只暴露有限 UIA 信息；后台截图可稳定取证，但不能替代真实 IME 前台交互。
