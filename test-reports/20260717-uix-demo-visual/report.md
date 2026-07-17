# UIX Demo 视觉 QA 报告

- 日期：2026-07-17
- 环境：Microsoft Windows 11 专业版 10.0.26200；真实 `UIX Demo` 原生窗口；Vulkan 图形后端
- 结论：**PASS**
- 未关闭 P0/P1：**0**

## 覆盖范围

本轮以真实窗口截图为主要视觉 oracle，覆盖全部 12 个 Demo 页面：

- 桌面窗口 `1200 × 800`：12 页顶部与底部，共 24 个场景。
- 紧凑窗口 `900 × 640`：首页、布局、输入、数据展示、反馈、框架能力、覆盖清单的顶部与底部，共 14 个场景。
- 关键状态：紧凑侧栏底部、浅色 Modal、紧凑 Modal、深色首页、深色输入顶部/底部、深色数据底部、深色 Modal，共 8 个场景。
- 额外应用控制证据：通过精确 HWND 和 Windows Graphics Capture 获取 1 张窗口截图。

自动化场景共 46 个，最终证据目录共 47 张截图。逐张检查裁切、重叠、缺字、对齐、间距、密度、层级、对比度、滚动与状态一致性。

## 发现与修复

| 严重度 | 视觉问题 | 修复与复测结果 | 证据 |
| --- | --- | --- | --- |
| P1 | Alert、Steps、日期/时间、Empty、Result、Tag、Table、Upload 等示例出现缺字方框或不稳定 Unicode 图标 | 统一改用 Lucide 图标绘制；浅色、深色和紧凑状态均未再出现 tofu 字形 | [基线导航底部](evidence/desktop-1200x800-04-navigation-bottom.png) · [最终导航底部](after/desktop-1200x800-04-navigation-bottom.png) · [最终反馈顶部](after/desktop-1200x800-07-feedback-top.png) |
| P1 | Table 未约束高度并覆盖后续 SelectableList；页面滚动动作还可能被嵌套 Table 截获 | 为 Table 增加公开固定视口尺寸能力；Demo 明确设置高度；语义滚动精确命中页面 ScrollView | [基线数据底部](evidence/desktop-1200x800-06-data-bottom.png) · [最终数据底部](after/desktop-1200x800-06-data-bottom.png) |
| P1 | Form 示例只显示标签，实际输入区域为空 | FormItem 挂载真实 Input 子组件；桌面和紧凑布局均显示完整三行表单 | [基线输入底部](evidence/desktop-1200x800-05-input-bottom.png) · [最终输入底部](after/desktop-1200x800-05-input-bottom.png) · [最终紧凑输入底部](after/compact-900x640-05-input-bottom.png) |
| P1 | `900 × 640` 下侧栏导航被底部版本信息/状态栏遮挡，无法完整查看 | 侧栏改为受约束的独立纵向 ScrollView，并增加结构回归测试 | [基线紧凑首页底部](evidence/compact-900x640-00-home-bottom.png) · [最终侧栏底部](after/state-compact-sidebar-bottom.png) |
| P1 | Modal 内容行把取消/确认按钮拉伸到整块内容高度 | 内容行改为居中对齐；桌面、紧凑、浅色、深色四种状态复测正常 | [最终浅色 Modal](after/state-light-feedback-modal.png) · [最终紧凑 Modal](after/state-light-feedback-modal-compact.png) · [最终深色 Modal](after/state-dark-feedback-modal.png) |
| P1 | SelectableList 使用硬编码深色背景，浅色主题对比错误；文件图标显示为原始 `file` 文本 | 全部颜色改用主题 token，图标改为 Lucide；浅色/深色数据页均正常 | [基线数据底部](evidence/desktop-1200x800-06-data-bottom.png) · [最终数据底部](after/desktop-1200x800-06-data-bottom.png) · [最终深色数据底部](after/state-dark-data-bottom.png) |
| P1 | Affix、BackTop、FloatButton 示例为空白，无法展示框架能力 | Affix 增加真实子节点与吸顶状态，BackTop 增加可见态，FloatButton 增加可选的正常布局占位并保留默认 overlay 语义 | [基线通用底部](evidence/desktop-1200x800-02-general-bottom.png) · [最终通用底部](after/desktop-1200x800-02-general-bottom.png) · [基线布局底部](evidence/desktop-1200x800-03-layout-bottom.png) · [最终布局底部](after/desktop-1200x800-03-layout-bottom.png) |
| P1 | 深色主题下多种输入组件仍使用硬编码白色容器背景 | Cascader、DatePicker、DateRangePicker、InputNumber、Mentions、TimePicker 改用 `color_bg_container` | [最终深色输入顶部](after/state-dark-input-top.png) · [最终深色输入底部](after/state-dark-input-bottom.png) |
| P1 | 初始帧和页面底部截图可能在呈现尚未稳定时采集，降低视觉 oracle 可信度 | 截图流程等待首帧与目标 revision 呈现；覆盖 12 页滚动、resize、主题切换和 Modal 开关 | [最终桌面首页](after/desktop-1200x800-00-home-top.png) · [精确窗口 WGC](after/toolkit-exact-window.png) |

## Must Pass 复核

- [x] 无阻塞阅读或操作的裁切与重叠。
- [x] 无缺字方框、原始图标名或明显乱码。
- [x] 关键控件在浅色/深色主题下具备可用对比度。
- [x] Modal 操作区在桌面与紧凑窗口中保持正常尺寸和层级。
- [x] 页面纵向滚动、侧栏独立滚动和嵌套 Table 滚动目标一致。
- [x] 既有紧凑布局采用最小内容宽度并提供横向滚动；该产品约束不作为裁切缺陷，语义回归确认滚动范围存在。

## 验证结果

| 验证 | 结果 |
| --- | --- |
| `cargo test --features agent-control,test-harness --test agent_gui_windows visual::real_demo_captures_all_pages_for_visual_review -- --ignored --exact --nocapture` | PASS；46 个真实窗口场景完成 |
| `cargo test --features test-harness --all-targets` | PASS；库测试 2085 通过、15 ignored；Demo 34 通过；其余集成与 Markdown 清单测试通过 |
| `ai-computer-toolkit doctor --pretty` | PASS；桌面、窗口、UIA、WGC 等本机控制 provider 可用 |
| 精确 HWND 后台截图 | PASS；`Windows.Graphics.Capture`，1202 × 808，硬件设备，前台窗口保持不变 |
| `cargo fmt --all -- --check` / `git diff --check` | PASS |
| 四文档严格检查器 | 仅报告 `docs/使用指南/**/*.md` 的 `unexpected_active_doc`；这是仓库 `AGENTS.md` 明确声明的允许结构，无其他错误 |

## 边界

- 本轮不采用固定像素差阈值：Demo 的计时文字、动画与平台字体栅格会产生合法差异。最终判定结合真实截图逐张复核、组件/布局断言和语义交互结果。
- 本轮只覆盖 Windows 原生窗口与 Vulkan 路径；macOS、Linux 和其他图形后端不在本次视觉结论范围内。
- 基线截图保存在 `evidence/`，最终截图保存在 `after/`。
