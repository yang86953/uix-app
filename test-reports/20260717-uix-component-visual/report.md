# UIX 逐组件视觉质量报告

- 日期：2026-07-17
- 环境：Microsoft Windows 11 专业版 10.0.26200；真实 `UIX Demo` 原生窗口；Vulkan pixel-upload；NVIDIA GeForce RTX 4070 Ti SUPER
- 结论：**PASS（Windows 范围）**
- 组件库存：**88 / 88**
- 声明状态：**412 / 412**
- 证据映射：**588 / 588 PASS**（176 个全局主题/尺寸状态 + 412 个组件适用状态）
- 原始 PNG：**316**（88 浅色桌面 + 88 深色紧凑 + 140 独立交互状态）
- 未关闭 P0/P1：**0**

## 验收口径

库存由公开导出与 `WidgetComponent` 声明生成并做精确集合比对，额外纳入公开构建器/Provider：`FloatButtonBackTop`、`NavGroup`、`Navigation`、`ConfigProvider`、`LocaleProvider`。每个库存项必须具备隔离 Demo 场景、非零可见目标、浅色桌面和深色紧凑证据；hover、pressed、focus、open、scroll、selection、sorting、focus trap 等交互状态按适用性生成独立截图。声明 focus 的单体组件必须暴露语义 `focus`，`NavGroup` / `Navigation` 构建器则必须在其可见子树内暴露真实可聚焦 `NavItem`，禁止用点击截图替代 focus。

真实窗口场景由 Agent Bridge 完成页面切换、指针移动/按下/释放、语义动作、键盘、滚动、resize 与主题切换；每次动作等待对应 `presented_revision` 后截图。视觉判定同时使用语义/交互断言、组件回归单测、21 张联系表人工复核和必要的原图放大复核，不以文件名存在代替状态成立。

完整状态到原始截图的映射见 [`component-visual-matrix.csv`](component-visual-matrix.csv)。21 张联系表归档在 `contact-sheets/`；原始 316 张 PNG 可通过报告中的真实窗口命令确定性再生到 `target/debug-captures/uix-component-visual/`。

## 发现与修复

| 严重度 | 问题 | 修复与复测 |
| --- | --- | --- |
| P0 | `uix-demo` 真实 Windows GUI 测试线程在完整逐组件场景下栈溢出 | GUI worker 使用显式 8 MiB 栈；完整 88 组件矩阵复测通过 |
| P1 | Carousel 空白、Message/Notification 在截图前过期、ResultView 第三个变体裁出舞台、Table/Tree 数据不足以证明滚动 | 补齐真实数据/持久反馈/固定视口与响应式宽度；浅色、深色和交互联系表复核通过 |
| P1 | Tooltip 与独立 FocusTrap 不布局子树；独立 FocusTrap 未接入 Tab 作用域 | 补齐子树 frame 布局、最近激活 trap 的 Tab 回环路由及回归单测；真实焦点循环截图通过 |
| P1 | Drawer/Modal 的中心点击未命中触发器，旧 open 文件实际仍为 closed；Drawer/Modal Escape 无法稳定到达叠层 owner | 使用真实触发区域，Escape 优先路由顶层 overlay，验证 Drawer 子控件焦点与 Modal `Cancel → Confirm → Cancel` 回环 |
| P1 | ScrollView 滚轮方向与 Windows/Table/Tree 不一致；Tree 未暴露真实语义 scroll offset | 统一滚动方向并导出 Tree viewport offset；真实滚动截图与单测通过 |
| P1 | Popover、Table、Upload 缺语义焦点/可见焦点；Carousel 子项布局后动态 `tab_index` 被节点初值掩盖 | 补齐键盘焦点状态和边框；节点同时读取组件运行时焦点能力；严格 focus 门槛通过 |
| P1 | Popover 的 focus 曾由点击打开伪装，随后 open 又被第二次点击关闭 | 删除点击回退；Popover 支持 FocusIn/Out、Enter/Space 与 Escape；最终 focus 为闭合蓝框，open 为真实弹层 |
| P2 | Upload 清单把未实现的鼠标 hover/pressed 当作适用状态 | 改为真实 `drop-zone / drag / file-list / pending / uploading / done / error / focus`，不触发或伪造文件选择 |

## 视觉复核

- [x] 88 个组件在浅色桌面、深色紧凑下均有完整可读画面。
- [x] 无阻塞阅读/操作的裁切、重叠、缺字方框、乱码或错误主题底色。
- [x] ResultView success/warning/error 三列在两种尺寸完整出现。
- [x] Drawer、Modal、Popconfirm、Popover、Tooltip 的 open/overlay/placement 证据是真实状态。
- [x] Carousel、Table、Tree、ScrollView 的 changed/selected/sorted/expanded/scrolled 证据可区分。
- [x] Drawer、Modal、FocusTrap、Popover、Table、Upload 的焦点状态可见且经过语义/键盘断言。
- [x] ConfigProvider、LocaleProvider 同时展示继承、覆盖、空渲染与语言 fallback。

## 验证命令

```bash
cargo test --features "test-harness agent-control" --test agent_gui_windows real_demo_captures_every_component_and_applicable_visual_state -- --ignored --nocapture
cargo test --features "test-harness agent-control" --test agent_gui_windows build_component_visual_contact_sheets -- --ignored --nocapture
```

| 门禁 | 结果 |
| --- | --- |
| 真实窗口逐组件矩阵 | PASS；88 个组件、412 个适用状态、316 张原始 PNG、588 行状态证据 |
| 视觉联系表生成与人工复核 | PASS；21 张联系表逐张检查，关键修复后二次复核 |
| `cargo test --all-targets --features "test-harness agent-control"` | PASS；lib 2098 passed / 15 ignored，Demo 38 passed，其他集成与 Markdown 清单测试通过；需桌面的 GUI 用例保持显式 ignored，并由上面两条命令单独执行 |
| `cargo check --all-targets --features "test-harness agent-control"` | PASS |
| `cargo test --doc --features "test-harness agent-control"` | PASS；5 passed / 24 ignored |
| `cargo clippy --all-targets --features "test-harness agent-control"` | PASS；退出 0，保留仓库既有非阻断 warning；`-D warnings` 不是当前仓库可通过的基线 |
| `cargo fmt --all -- --check` / `git diff --check` | PASS |
| `python scripts/check_docs_links.py` / `python -m unittest scripts.test_check_docs_links` | PASS；36 个 Markdown 文件及 3 个检查器单测通过 |

## 边界

- 本报告只对 Windows 原生窗口、Vulkan pixel-upload 和当前 NVIDIA 设备给出视觉 PASS；macOS、Wayland、AMD、Intel 仍是环境矩阵缺口。
- 动画、计时文字与平台字体栅格使固定全图像素 diff 不适合作为唯一 oracle；结论来自状态断言、真实 present、逐图人工复核和回归测试的组合。
- `agent-control` 是显式启用的本机开发预览能力；测试不扩大其传输、安全或产品授权边界。
