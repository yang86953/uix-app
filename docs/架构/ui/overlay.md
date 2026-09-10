# overlay 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 ui System 的窗口内浮层栈、层叠、焦点陷阱和进退场。与[组件运行时框架](widget_runtime.md)、[event](event.md)和[animation](animation.md)的协作由 ui System 编排，Module 间不直接持有实例。导出：所有内置与自定义浮层共享的栈契约；公开用法见[使用 · 反馈](../../使用/交互与反馈/反馈.md)。
>
> **当前实现线索**：栈与 entry 位于 `src/ui/overlay/mod.rs`，组件树在 `src/ui/widget_runtime/widget/tree_layout/layout.rs` 重建登记，焦点陷阱位于 `src/ui/widget_runtime/focus_trap.rs`，具体 placement 由各浮层组件持有。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `OverlayStack` | struct | 每窗 entry 栈、层级、命中和 focus trap 查询 |
| `OverlayEntry` | struct | owner、kind、bounds、z-index、modal/dismiss/focus 行为 |
| `OverlayId` | value | 栈内稳定身份 |
| `OverlayKind` | enum | Modal、Drawer、Popover、Tooltip、ContextMenu、Message、Notification、Custom |
| `OverlayBackdropBlur` | value | Theme/显式半径与 mask/独立逻辑区域请求 |
| `OverlayBackdropEffect` | value | UI 聚合后交给 ScenePipeline 的唯一逻辑区域与半径 |
| `FocusTrap` | foundation 机制 | 最上层模态子树内的焦点循环与恢复 |
| `TransitionPlayer` | animation struct | enter/leave 视觉生命周期 |

## 组件：OverlayStack / OverlayEntry

- 每个 WidgetTree 只有一个 OverlayStack；后打开或更高 z-index 的 entry 先命中。
- overlay 命中先递归 owner 的浮层子树，再回退 owner/mask；底层树在 modal 遮罩下不可接收事件。
- bounds 使用窗口 logical 坐标并收敛到当前 surface；placement、绘制、damage 与 hit-test 共用同一几何。
- 组件树重建登记时通过 `WidgetRender::overlay_entry_for_surface` 显式传入当前根表面；默认实现向后兼容旧入口，依赖窗口边界的组件不得只凭触发器 frame 复用上一帧 surface 下的绝对矩形。
- owner 移除、隐藏、换根或 generation 失效时，managed entry 自动清理。

## backdrop blur

- 全部 `OverlayKind` 共用 `OverlayEntry::backdrop_blur`，默认关闭；内置 Modal、Drawer 另提供同名便捷属性，`Custom` 与内置类型不形成私有分支。
- `OverlayBackdropBlur::theme()` 延迟读取当前 Theme 的 `backdrop_blur_radius`（默认 8.0 逻辑像素），`radius` 显式值覆盖 token；小于 0.5px、非有限半径或无效区域不形成效果计划。
- 默认区域使用 entry 的 mask/hit bounds；`region` 可以提供独立窗口逻辑区域。OverlayStack 把同帧多个请求收敛为区域并集与最大半径，ScenePipeline 只执行一次 snapshot/blur 计划。
- `RenderTarget::supports_backdrop_blur()` 是真实能力查询；不支持时保留纯色 mask，不伪报 blur。资源、copy、draw、submit 或 destroy 失败保持 typed failure 并中止当前帧。
- GPU owner 同时保留未模糊 clean snapshot 与由它派生的 effect texture。策略、半径或区域变化时销毁旧派生纹理并从 clean snapshot 重新复制、模糊，不重复 acquire/present，也不对旧 blur 结果累计取样；离场开始以 no-op 半径释放派生效果。
- 普通树、主题或窗口尺寸变化时，GPU-native ScenePipeline 强制完整重建正常树，通过 API-neutral FrameEncoder 先写 retained texture（无 present），再执行新 clean snapshot→effect blur→restore，最后只重放 overlay 并由原帧 `end_frame` 做唯一最终 present；中间阶段不重复 acquire/present，任一 typed failure 都保留 invalidation 并中止当前帧。
backdrop blur 的验收必须同时证明背景被处理、前景保持清晰、逻辑区域与像素裁剪一致，并覆盖不支持能力和任一中间阶段失败；阶段性设备矩阵与结果由 Gitea 持有。

`FloatButton` 等显式窗口 placement 由本模块解释为 logical 客户区锚点；组件保存 placement、有限作者偏移与内容配置，并以同一解析几何驱动绘制、damage、命中和 entry bounds。未显式 placement 的普通布局节点继续服从所属 layout frame，overlay 不夺取容器布局所有权。

## 表面约束与弹层定位

- 弹层使用当前帧的 logical surface、触发器 frame、内容自然尺寸和作者首选方向；首选放不下时比较备选方向，最终矩形收敛到安全客户区。
- 组件内部的多列、树、月历、时间列、色板或页脚只从最终弹层矩形继续细分；各分区不得独立选择方向或越过 surface。
- 候选数量、内容尺寸、surface、锚点或主题几何变化时重新计算；同一打开周期保留旧矩形用于 dirty 合并，新的呈现周期不得复用旧缓存。
- 绘制、箭头、裁剪、键盘显露、滚动上限、命中、damage 和 OverlayStack bounds 必须消费同一最终几何。一个弹层的验证结果不能外推到其他组件；实际覆盖矩阵由 Gitea 持有。

## 焦点

- 模态浮层打开后把焦点移入第一个可聚焦节点，Tab/Shift+Tab 在 trap 内循环。
- 关闭后焦点尽量回到仍有效的触发节点。
- 新 trap 建立时先保存实际原焦点，再进入可聚焦内容。若同一 owner 在关闭态承担触发按钮，它自身也是恢复候选，不能按内容后代排除；打开时 owner 已聚焦也不等于正文已获得焦点。正文无可聚焦节点时才回退到 owner，恢复时仍核对目标有效性。
- 多层浮层只由最上层有效 trap 约束；Escape/外部点击遵守 entry 的 dismiss 配置。
- 离场开始即停止新交互，但可保留视觉节点到动画完成。

Modal 的 `State<bool>` 是受控显隐的唯一事实源；组件自身独占进入、离场、OverlayStack entry、焦点恢复、操作命中与绘制几何。确定与取消只作为实例持有的同步窄回调存在，不引入全局服务或 EventBus；确定按钮和 Enter / Space 形成确认入口，取消按钮、标题栏关闭、可关闭遮罩与 Escape 形成取消入口，任一有效操作都只回调一次、写回 `false` 并立即停止后续交互。

Drawer 遵守同一受控事实源边界：可选 `State<bool>` 只保存业务显隐事实，组件独占 placement、面板尺寸、OverlayStack entry、进退场、布局、命中与输入。外部状态变化由动画帧同步到组件生命周期；用户关闭只写回一次 `false`，离场期间吞掉输入且重复关闭不重启动画。该闭环是组件内同步契约，不引入 EventBus、全局服务或后台线程。

## enter / leave

entry membership 改变时同步 OverlayStack。进出场只在 TransitionPlayer active 时登记动画帧；holding 阶段用 timer deadline 或纯静态状态，不维持固定 16ms tick。

Message/Notification 的每项可以有独立稳定 ID 和 `Entering → Holding → Leaving` 状态，但最终都归所属窗口调度，队列为空时删除 entry 和相关 timer。

Modal 内容保留终态布局；进出场缩放与透明度在组件拥有的视觉边界施加，面板和真实内容子树同步变化，遮罩仍覆盖整个表面。子树变换同时用于绘制、裁剪、语义和命中，不能只缩放面板矩形而让内容保持原大小，也不能在静态缓存中冻结首个采样。退场开始停止新交互，动画完成后再移除视觉节点并恢复仍有效的原焦点。

## 绘制与缓存

全屏 overlay 可能把 damage 提升为 FullComposite。backend 能安全复用干净背景时，可缓存 overlay 打开前的 retained 内容并只重绘浮层；策略/半径/逻辑区域变化把旧、新 effect 区域都送入 damage，并从未模糊 clean snapshot 重新派生。普通树、尺寸、主题、debug 状态或 overlay membership 变化后必须失效，不能从已含遮罩的帧重新捕获“背景”。

## 不变量

- OverlayStack 不单独创建线程或跨窗 wake。
- `Custom` entry 没有隐式 modal/dismiss/focus 行为，调用方必须显式配置。
- 浮层内容是普通 View 子树，仍使用同一 layout/event/paint/semantics 管线。
- entry、焦点恢复目标和缓存都绑定窗口及 tree generation；关闭或换根后晚到 timer/动画不得复活旧浮层。
