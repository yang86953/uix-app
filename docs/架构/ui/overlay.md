# overlay 模块

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统的窗口内浮层栈、层叠、焦点陷阱和进退场。依赖：[component](component.md)、[event](event.md)、[animation](animation.md)。导出：所有内置与自定义浮层共享的栈契约；公开用法见[使用 · 反馈](../../使用/反馈.md)。
>
> **当前实现线索**：栈与 entry 位于 `src/ui/overlay/mod.rs`，组件树在 `src/ui/component/widget/tree_layout/layout.rs` 重建登记，焦点陷阱位于 `src/ui/component/focus_trap.rs`，具体 placement 由各浮层组件持有。

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
- 当前普通树内容变化仍会使 overlay 快照进入安全阻塞并退化为整树 mask-only 重绘；主题/窗口变化后在 overlay 保持期间完成“正常树提交→新 clean snapshot→blur→overlay”的单 present 事务，仍属于 #806 后续实现，不能把本批的 effect 配置失效等同于该事务已闭合。

`FloatButton` 的显式 `Placement` 由本模块解释为窗口 logical 客户区锚点；组件只保存 placement、有限作者偏移与内容配置，并用同一个私有几何结果驱动 control、description、badge、tooltip、damage、命中和 entry bounds。未显式 placement、普通布局占位及 `FloatButtonGroup` 子按钮继续服从所属布局 frame，避免 overlay 模块夺取容器布局所有权。

`f41fede8` 为该表面契约建立 Popover/Popconfirm 回归：触发器 frame 不变而 surface 缩小时，布局阶段的 OverlayStack bounds 与随后绘制共同消费新表面；旧缓存横坐标 150px/200px 分别收敛为 20px/60px。两项聚焦契约、布局 24 项、反馈门面 4 项与完整库 151 项测试通过。

`5fefe2ed` 把同一契约扩展到共享提示气泡原语：Tooltip 与 Slider 的提示登记显式接收当前 surface，作者方向与主轴反向候选按越界量择优，再将最终尺寸和坐标约束到窗口内；绘制箭头使用解析后的真实方向，文字裁在最终气泡中。两项原语契约、两项组件集成契约与完整库 155 项测试通过。

`d288e6c5` 进一步闭合 Select：布局从 `WidgetTree` 根 frame、绘制从 `PaintContext`、登记从 `overlay_entry_for_surface` 取得同帧 surface，共享解析器统一横向收敛、上下翻转与可用高度缩减；自定义选项布局、滚动、绘制、dirty、命中和 bounds 均复用最终矩形，过滤前后方向不同的 dirty 同时覆盖两侧。四项聚焦契约与完整库 159 项测试通过。

`c386f992` 以同一原则闭合 Cascader，但把多列作为组件私有约束处理：共享解析器先把自然总宽限制到 surface，再按可见列数等分最终列宽；纵向翻转或缩高后的实际视口同时驱动列滚动、搜索滚动和键盘显露。登记与绘制记录同帧 surface，事件、绘制裁剪、dirty、命中与 bounds 复用相对弹层缓存。四项聚焦契约与完整库 163 项测试通过。

`250ba978` 将同一约束延伸到 TreeSelect：组件缓存同帧 surface、触发器绝对锚点、相对弹层与跨帧绝对脏区；树节点展开或过滤改变可见行数时，仍以同一表面和锚点重算。事件命中、虚拟滚动、键盘显露、绘制裁剪、dirty、OverlayStack bounds 统一使用翻转或缩高后的实际视口，关闭动画和表面变化也保留旧弹层覆盖。五项聚焦契约与完整库 168 项测试通过。

`5117f4d7` 将同一约束延伸到 AutoComplete，并区分全新呈现与同一打开周期内的连续输入：只有从非呈现状态打开时才清空缓存；过滤改变候选数时，以同一 surface 和绝对锚点重算当前弹层，OverlayStack bounds 随当前行数缩短，dirty 继续合并过滤前区域。事件命中、虚拟滚动、键盘显露和绘制裁剪共同使用实际视口。六项聚焦契约与完整库 174 项测试通过。

`ed758bb3` 将同一约束延伸到 Mentions，并把呈现周期绑定到光标前的有效活动 `@` 查询：从非建议状态进入查询时清空缓存，同一查询内连续输入过滤则保留旧脏区。当前 surface、绝对触发锚点、相对弹层与历史绝对脏区由绘制、dirty、命中、虚拟滚动、键盘显露和 OverlayStack bounds 共同消费；登记只覆盖当前实际弹层，不再并入输入框。六项聚焦契约与完整库 180 项测试通过。日期类选择弹层仍需由对应测试覆盖，不能仅因 Select/Cascader/TreeSelect/AutoComplete/Mentions 已闭合就视为全部 overlay placement 完成。

`677e5a7d` 将同一约束延伸到 DatePicker：表面解析器在触发器上下保留 2px 间隙，横向收敛宽度与起点，纵向优先完整向下、其次完整向上、最后选择较大空间缩高。组件缓存同帧 surface、绝对触发锚点、相对月历与当前打开周期的历史绝对脏区；共享月历则从最终矩形派生标题栏、星期栏、六行日期、导航和字体比例，使绘制与命中在 78px 受限高度下仍一致。OverlayStack 只登记当前月历，dirty 合并 resize 前后区域。五项聚焦契约与完整库 185 项测试通过。DateRangePicker、TimePicker、ColorPicker 仍需由对应测试覆盖，不能据此视为日期时间颜色类 overlay placement 全部完成。

`5bbfd02f` 将约束延伸到 DateRangePicker 的“月历 + 可选预设页脚”组合面板：解析器先确定唯一的表面内组合矩形，再按同一纵向比例派生月历、间隙、页脚上留白和预设行，使日期、月份导航与预设命中始终对应绘制分区。三项预设的 332px 自然面板缩到 78px 时，日期和第二个预设仍可提交；OverlayStack 只登记当前组合面板，dirty 合并 resize 前后的历史区域。迁移后删除无消费者的自然尺寸过渡入口。六项聚焦契约与完整库 191 项测试通过。TimePicker、ColorPicker 仍需由对应测试覆盖。

`fa3a6595` 将约束延伸到 TimePicker 的小时/分钟双列滚动面板：解析器先确定唯一的表面内视口，列宽随最终面板收敛，32px 行高保持不变；滚动上限、当前值显露、指针/滚轮/键盘、绘制和命中都使用实际视口。78px 视口下分钟最大偏移由错误的 1720px 修正为 1842px，第 59 分钟可达，23:59 两列高亮仍可见；OverlayStack 只登记当前面板，dirty 合并 resize 前后的历史区域。七项聚焦契约与完整库 198 项测试通过。ColorPicker 仍需由对应测试覆盖。

`1e5cb989` 将约束延伸到 ColorPicker 的固定 8 列、24 色预设面板：解析器以 208×88px 为自然尺寸并保留 4px 触发间隙，横向收敛宽度与起点，纵向翻转或按较大空间缩高；共享 `ColorPanelGeometry` 从最终矩形同步派生内边距、色块槽位与命中映射。底边场景可翻到上方并缩为 76px，100px 窄表面内仍保持绘制与点击一致；关闭动画继续沿用同一受限几何。OverlayStack 只登记当前面板，dirty 合并 surface 改变前后的历史区域，重新打开的新呈现周期会重置旧损伤缓存。七项聚焦契约与完整库 205 项测试通过。日期、时间与颜色选择弹层这一子序列至此闭合，但不代表 Flex、表格组合、可变行高或真窗测试已经覆盖。

## 焦点

- 模态浮层打开后把焦点移入第一个可聚焦节点，Tab/Shift+Tab 在 trap 内循环。
- 关闭后焦点尽量回到仍有效的触发节点。
- 多层浮层只由最上层有效 trap 约束；Escape/外部点击遵守 entry 的 dismiss 配置。
- 离场开始即停止新交互，但可保留视觉节点到动画完成。

Modal 的 `State<bool>` 是受控显隐的唯一事实源；组件自身独占进入、离场、OverlayStack entry、焦点恢复、操作命中与绘制几何。确定与取消只作为实例持有的同步窄回调存在，不引入全局服务或 EventBus；确定按钮和 Enter / Space 形成确认入口，取消按钮、标题栏关闭、可关闭遮罩与 Escape 形成取消入口，任一有效操作都只回调一次、写回 `false` 并立即停止后续交互。

Drawer 遵守同一受控事实源边界：可选 `State<bool>` 只保存业务显隐事实，组件独占 placement、面板尺寸、OverlayStack entry、进退场、布局、命中与输入。外部状态变化由动画帧同步到组件生命周期；用户关闭只写回一次 `false`，离场期间吞掉输入且重复关闭不重启动画。该闭环是组件内同步契约，不引入 EventBus、全局服务或后台线程。

## enter / leave

entry membership 改变时同步 OverlayStack。进出场只在 TransitionPlayer active 时登记动画帧；holding 阶段用 timer deadline 或纯静态状态，不维持固定 16ms tick。

Message/Notification 的每项可以有独立稳定 ID 和 `Entering → Holding → Leaving` 状态，但最终都归所属窗口调度，队列为空时删除 entry 和相关 timer。

## 绘制与缓存

全屏 overlay 可能把 damage 提升为 FullComposite。backend 能安全复用干净背景时，可缓存 overlay 打开前的 retained 内容并只重绘浮层；策略/半径/逻辑区域变化把旧、新 effect 区域都送入 damage，并从未模糊 clean snapshot 重新派生。普通树、尺寸、主题、debug 状态或 overlay membership 变化后必须失效，不能从已含遮罩的帧重新捕获“背景”。

## 不变量

- OverlayStack 不单独创建线程或跨窗 wake。
- `Custom` entry 没有隐式 modal/dismiss/focus 行为，调用方必须显式配置。
- 浮层内容是普通 View 子树，仍使用同一 layout/event/paint/semantics 管线。
