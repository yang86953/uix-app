# UIX Demo 全面回归测试报告

结果：Partial。当前 Windows 实机 demo 的全部 11 个页面、关键交互与本轮修复路径均已回归通过；Microsoft Pinyin 真实 composition 会话仍未取得可判定的前台证据，因此不把输入法生产验收标为完成。

环境：Windows 11、Rust 1.96.0、当前工作树；测试时间 2026-07-12（UTC+08:00）。通过 Windows 实际 `uix-demo` 窗口完成操作与截图，使用默认 Auto 图形路径。测试控制层的数值/按键参数解析已补强并单独通过其工作区全量测试。

## 覆盖与结果

| 场景 | 结果 | 直接证据 |
| --- | --- | --- |
| 页面导航与初始渲染 | 通过 | 首页、应用能力、通用、布局、导航、输入、数据展示、反馈、图表、其他、覆盖清单均在实际窗口中切换并完成渲染；基线截图见 [首页](evidence/01-home-baseline.png)、[运行时](evidence/02-runtime-page.png)。 |
| 运行时 State、Timer、主题 | 通过 | `+1` 后局部计数更新；全局 timer 连续递增；明/暗主题切换均正常，见 [State/Timer](evidence/03-runtime-state-timer.png)、[暗色主题](evidence/04-runtime-dark.png)。 |
| `InputNumber` 跨 reconcile | 通过 | 焦点内按 `Up` 后输入 `42`，值为 `142`；等待超过全局 timer 的刷新周期后仍保留。对应单元回归 `uncontrolled_value_survives_reconcile` 已进入全量测试。 |
| 多窗口与系统关闭 | 通过 | 打开主题副窗口后，`Alt+F4` 与标题栏关闭均在 1.2 秒内移除副窗口；主题在主/副窗之间同步。 |
| Transfer / Upload | 通过 | 选择源列表的 A 项后右移，目标列表保留 A/C；点击 Upload 产生 `upload_1.png`、`upload_2.png` 的模拟条目。 |
| 反馈 Overlay | 通过 | Modal 打开 → Escape 关闭 → 重开，以及 Drawer 打开均正常，未出现裁切或无法重开的回归。 |
| 窗口尺寸事件 | 通过 | 主窗口最大化后客户区完整绘制，随后还原到原尺寸，无黑边或崩溃。 |
| CLI demo | 通过 | `cargo run --bin uix-demo -- --cli` 退出 0，输出“全部演示完成”。 |
| 自动化与文档门禁 | 通过 | `cargo test --all-targets`：lib **1183/1183**、demo **23/23**；`cargo test --doc`：**3 passed / 24 ignored**；Clippy 静态检查、`python scripts/check_docs_links.py`、`git diff --check` 均退出 0。Clippy 仅保留既有非阻断 warning。 |

## 发现、修复与回归

### BUG-20260712-001 [High] `InputNumber` 在根视图刷新时回退默认值

全局 timer 导致 root reconcile 时，未显式 `.value(...)` 的 `InputNumber` 会被新建节点的默认 `0` 覆盖。组件现在区分受控值与未配置值：仅在下一节点显式配置 `.value(...)` 时采用新值，否则保留当前值并按新边界夹紧；新增 `uncontrolled_value_survives_reconcile` 回归。

### BUG-20260712-002 [High] `Alt+F4` 不能关闭主题副窗口

`WM_SYSKEYDOWN` 原先与普通按键一样返回 `0`，阻断 `DefWindowProcW` 将 `Alt+F4` 转成 `WM_SYSCOMMAND/WM_CLOSE`。现在继续投递 UI 键事件，但系统键上/下行同时交回默认窗口过程；真实副窗口的 `Alt+F4` 与标题栏关闭均已复测通过。

## 测试控制能力补强

测试过程中发现控制工具会把 `--arg text=42`、`--arg key=4` 解析为数值，无法可靠表达自绘控件的文本/按键参数。已在外部本地工具工作区修正这两类参数为字符串，并通过其格式、定向、工作区测试及 Clippy。该工具目录不是本仓 Git 工作树，因此不纳入本仓提交。

## 已知限制

- Windows Microsoft Pinyin（或等价 IME）的候选、提交和 Tab 焦点遍历仍需要真实前台 composition 证据；沿用 [进度 · 未关闭缺陷](../../docs/进度.md) 中 BUG-002 的待验证状态。
- 本报告只证明当前 Windows 主机与默认图形路径；不同 GPU/驱动、DPI 和 Linux/macOS 仍需单独实机验收。
