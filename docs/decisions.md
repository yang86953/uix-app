# 设计决策索引

← [Main](Main.md) · 按需查阅

> **唯一决策台账**（#1–#100 定稿；#101–#159 术语收敛、核心理念、豁免治理与实现边界；新决策 #160+）。锚点 `#d{N}`。系统正文 → [`systems/`](systems/) · 完整索引 → [Main.md](Main.md)

## 实现顺序（[#50](#d50)）

1. Theme（120 chromatic + 13 neutral + TypographyScale）  
2. Style（StyleSet / ColorValue / TypographyToken）  
3. Event（System / Semantic / Custom）  
4. HandlerTable  
5. Lifecycle  
6. Button  
7. 全量重写内置 Widget + demo（[#58](#d58)、[#80](#d80)）  

## 按域索引

| 域 | 代表决策 |
|----|----------|
| **主题** | [#1](#d1)–[#17](#d17)、[#9](#d9)、[#28](#d28)、[#33](#d33)、[#42](#d42)–[#44](#d44)、[#48](#d48)、[#51](#d51)–[#52](#d52)、[#57](#d57)、[#65](#d65)、[#74](#d74)、[#76](#d76) 等 |
| **组件** | [#8](#d8)、[#20](#d20)、[#22](#d22)、[#30](#d30)、[#35](#d35)、[#39](#d39)、[#47](#d47)、[#58](#d58)、[#77](#d77)、[#83](#d83)、[#101](#d101)、[#102](#d102) [#146](#d146) [#151](#d151) [#152](#d152) 等 |
| **View / 响应式** | [#21](#d21)、[#24](#d24)、[#31](#d31)、[#49](#d49)、[#60](#d60)、[#62](#d62)、[#78](#d78)、[#79](#d79)、[#123](#d123)、[#135](#d135) [#138](#d138) [#142](#d142) [#143](#d143) [#149](#d149) [#150](#d150) [#153](#d153) [#155](#d155)–[#156](#d156) [#159](#d159) |
| **事件** | [#4](#d4)–[#7](#d7)、[#10](#d10)、[#18](#d18)–[#19](#d19)、[#25](#d25)–[#27](#d27)、[#36](#d36)、[#46](#d46)、[#66](#d66)、[#68](#d68)、[#71](#d71)、[#75](#d75)、[#90](#d90)–[#92](#d92)、[#95](#d95)、[#98](#d98) [#147](#d147) 等 |
| **布局** | [#29](#d29)、[#38](#d38)、[#45](#d45)、[#53](#d53)、[#56](#d56)、[#67](#d67)、[#73](#d73)、[#81](#d81)、[#84](#d84)、[#103](#d103)、[#104](#d104) |
| **渲染** | [#59](#d59)、[#70](#d70)、[#82](#d82)、[#86](#d86)–[#87](#d87)、[#122](#d122) [#129](#d129) [#136](#d136) |
| **应用** | [#32](#d32)、[#55](#d55)、[#61](#d61)、[#64](#d64)、[#72](#d72)、[#88](#d88)、[#89](#d89)、[#93](#d93)–[#94](#d94)、[#132](#d132)–[#134](#d134) [#137](#d137) [#140](#d140)–[#141](#d141) [#144](#d144)–[#145](#d145) [#148](#d148) [#149](#d149) [#153](#d153) [#155](#d155) |
| **平台 / 测试** | [#40](#d40)、[#59](#d59) [#139](#d139) |
| **浮层** | [#96](#d96)–[#100](#d100) |
| **术语收敛** | [#101](#d101)–[#104](#d104) |
| **按需零闲置** | [#105](#d105)–[#158](#d158) |
| **工程** | [#154](#d154) [#157](#d157) [#158](#d158) |
| **预留** | [#41](#d41)、[#54](#d54)、[#85](#d85) |

## 决策全表

| # | 议题 | 决策 |
|---|------|------|
| <a id="d1"></a>1 | 主色在衍生色中的位置 | **第 6 阶**；chromatic **12×10=120** |
| <a id="d2"></a>2 | 中性色 | **13 阶**白→黑，独立 scale |
| <a id="d3"></a>3 | Custom 色标记 | **debug lint / clippy 警告** |
| <a id="d4"></a>4 | 事件传播 | **Bubble 默认**，Capture opt-in |
| <a id="d5"></a>5 | 语义事件 | **可扩展注册表** + 内置常用事件 |
| <a id="d6"></a>6 | CustomEvent payload | **typed struct + TypeId / 宏** |
| <a id="d7"></a>7 | 多 handler | **顺序执行，可 stop**（[#68](#d68) 扩展 stop 语义） |
| <a id="d8"></a>8 | Active/Inactive | **v1 就要**（Focus + 视口） |
| <a id="d9"></a>9 | 主题切换重绘 | **只 invalidate 用 Palette 的组件** |
| <a id="d10"></a>10 | 绑定存储 | **HandlerTable 挂 ComponentId** → 见 [#101](#d101) |
| <a id="d11"></a>11 | 12 主色命名 | **Ant Design 同款**（Red … Magenta） |
| <a id="d12"></a>12 | 衍生色算法 | **移植 Ant Design 官方色板算法** |
| <a id="d13"></a>13 | 中性色引用 | **NeutralRole** 15 角色 → 13 阶映射 |
| <a id="d14"></a>14 | StyleSet 五态与优先级 | **五态** normal/hover/pressed/focused/disabled；**disabled > focused > pressed > hover > normal** |
| <a id="d15"></a>15 | StyleSet 继承 | **差异字段 + 继承 normal** |
| <a id="d16"></a>16 | Button 变体 | **StyleSet 预设**，无 variant enum |
| <a id="d17"></a>17 | Theme 范围 | **Light/Dark + 可换种子**；**[#57](#d57) 两档 API** |
| <a id="d18"></a>18 | 焦点模型 | **单焦点 + tab_index 焦点链** |
| <a id="d19"></a>19 | 视口检测 | **与祖先 clip/scroll 求交** |
| <a id="d20"></a>20 | 组件 authoring | **`component!` 宏** → [#102](#d102) |
| <a id="d21"></a>21 | 使用入口 | **只用 View DSL** |
| <a id="d22"></a>22 | Button 尺寸 | **StyleSet 预设** + TypographyToken |
| <a id="d23"></a>23 | Ripple | **v1 不做** |
| <a id="d24"></a>24 | 响应式 State | **`State<T>` 绑定** → auto invalidate |
| <a id="d25"></a>25 | 语义事件冒泡 | **默认 Bubble** |
| <a id="d26"></a>26 | Handler 生命周期 | **闭包 `'static`** + AppState/Handle |
| <a id="d27"></a>27 | 组件侧 Custom | **仅 debug lint** |
| <a id="d28"></a>28 | 默认品牌主色 | **Primary::BLUE（8）** |
| <a id="d29"></a>29 | Layout 方法名 | **`measure(constraints) -> Size`** |
| <a id="d30"></a>30 | Button icon/loading | **v1 纯文本** |
| <a id="d31"></a>31 | View 更新 | **声明式重建 + diff** 复用 ComponentId → 见 [#101](#d101) |
| <a id="d32"></a>32 | 业务数据 | **AppState + ComponentHandle** |
| <a id="d33"></a>33 | Theme 作用域 | **App 全局**；**多窗共享**（[#93](#d93)–[#94](#d94)） |
| <a id="d34"></a>34 | danger 预设 | **Primary::RED** |
| <a id="d35"></a>35 | ComponentId | **Generational ID** → 见 [#101](#d101) |
| <a id="d36"></a>36 | Click payload | **`{ button, pos, modifiers }`** |
| <a id="d37"></a>37 | disabled 视觉 | **opacity + 中性色** |
| <a id="d38"></a>38 | measure 约束 | **`Constraints { min, max, definite }`** |
| <a id="d39"></a>39 | 预设命名 | **`StyleSet::button_*()`** |
| <a id="d40"></a>40 | 组件测试 | **FakePlatform + 语义断言 + paint snapshot** |
| <a id="d41"></a>41 | （预留） | **编号保留，未定稿** |
| <a id="d42"></a>42 | 字体 | **Theme TypographyScale** |
| <a id="d43"></a>43 | 边框 v1 | **`border_width: EdgeInsets` + border_color** |
| <a id="d44"></a>44 | 阴影 v1 | **Style.shadow 可选** |
| <a id="d45"></a>45 | 滚动 | **ScrollView 消化 Wheel**（旧称 ScrollContainer，见 [#104](#d104)） |
| <a id="d46"></a>46 | i18n v1 | **不做**，事件预留 |
| <a id="d47"></a>47 | button_ghost | **透明底 + Primary 边框/文字** |
| <a id="d48"></a>48 | 默认 Theme | **内置 DefaultTheme** |
| <a id="d49"></a>49 | View diff | **`.key()` + [#60](#d60) 细粒度 diff** |
| <a id="d50"></a>50 | 实现顺序 | 见上文 |
| <a id="d51"></a>51 | TypographyScale | **Ant Design ~10 档** + **TypographyToken** |
| <a id="d52"></a>52 | NeutralRole | **15 角色**，Light/Dark 各一套映射 |
| <a id="d53"></a>53 | v1 布局 | **Flex + Grid** |
| <a id="d54"></a>54 | （预留） | **编号保留，未定稿** |
| <a id="d55"></a>55 | AppState | **App 单例持有** + Handle |
| <a id="d56"></a>56 | border_width | **EdgeInsets 四边** |
| <a id="d57"></a>57 | 品牌主题 | **`with_brand_primary` / `from_primaries([12])`** |
| <a id="d58"></a>58 | Widget 落地 | **Big Bang 全量重写** |
| <a id="d59"></a>59 | 默认引擎 | **GPU 优先，失败回退 CPU** |
| <a id="d60"></a>60 | View diff 粒度 | **Style/文本/handler 字段比较**（handler 比较用于 #123 智能重绑判定） |
| <a id="d61"></a>61 | ComponentHandle | **只读配置** + invalidate/emit |
| <a id="d62"></a>62 | Handler diff | **rebuild 时智能重绑** — 仅 handler 变更时 clear+重注册；未变则保留（[#123](#d123) 修订） |
| <a id="d63"></a>63 | focused 时机 | **仅 :focus-visible** |
| <a id="d64"></a>64 | Settings | **App 可选注入**，默认不自动存盘 |
| <a id="d65"></a>65 | font_family | **系统字体栈** |
| <a id="d66"></a>66 | 语义注册表 | **SemanticKind enum + register_semantic!** |
| <a id="d67"></a>67 | Grid DSL | **`grid(...)` 组合器** |
| <a id="d68"></a>68 | handler stop | **stop 后续 handler + 语义冒泡** |
| <a id="d69"></a>69 | prelude | **精选高频符号** |
| <a id="d70"></a>70 | HiDPI | **scale_factor**，逻辑 px |
| <a id="d71"></a>71 | 剪贴板 v1 | **SystemEvent → 语义事件** |
| <a id="d72"></a>72 | Handle 可读 | **仅配置字段** |
| <a id="d73"></a>73 | Scroll DSL | **`scroll(...).vertical().horizontal()`** |
| <a id="d74"></a>74 | Light/Dark | **默认跟 OS**；App 可显式覆盖 |
| <a id="d75"></a>75 | 文本输入 v1 | **单行 + 多行 + 完整 IME** |
| <a id="d76"></a>76 | OS 主题监听 | 运行中切换 Theme；**跟 OS** 须 App opt-in `.follow_system_theme(true)`（[#125](#d125)）；否则 App 显式 `.theme()` |
| <a id="d77"></a>77 | button() 默认 | **`button_default()`**；链式 `.primary()` 等 |
| <a id="d78"></a>78 | Computed | **保留** |
| <a id="d79"></a>79 | Effect | **保留**；禁止 Effect 内直接改 UI |
| <a id="d80"></a>80 | demo | **与 Button 同时重写** |
| <a id="d81"></a>81 | Flex 对齐 | **Style**：align_items / justify_content / gap |
| <a id="d82"></a>82 | 离屏缓存 | **PicturePolicy 自适应 Picture**（[#122](#d122) [#129](#d129)） |
| <a id="d83"></a>83 | 动画 | **AnimationRegistry** 驱动动画帧 |
| <a id="d84"></a>84 | Grid 轨道 | **Style grid_template_*** |
| <a id="d85"></a>85 | （预留） | **编号保留，未定稿** |
| <a id="d86"></a>86 | Picture 条件 | 子树代价（[#129](#d129) 修订）；须 PicturePolicy=Eligible（[#122](#d122)） |
| <a id="d87"></a>87 | 深度阈值 | **≥ 4 层** — 已由 [#129](#d129) 取代；保留编号供历史引用 |
| <a id="d88"></a>88 | AppState 线程 | **仅主线程** |
| <a id="d89"></a>89 | 错误 UI | **可选 Toast**；默认 log |
| <a id="d90"></a>90 | once_click | **首次触发后移除** |
| <a id="d91"></a>91 | `.when()` | **每次派发前求值** |
| <a id="d92"></a>92 | preventDefault | **语义层 default_prevented** |
| <a id="d93"></a>93 | 多窗口 | **v1 多窗**；共享 AppState + Theme |
| <a id="d94"></a>94 | 多窗 Theme | **全局共享** |
| <a id="d95"></a>95 | 拖放 | **SemanticEvent::FileDrop** |
| <a id="d96"></a>96 | Tooltip | **v1 完整组件** |
| <a id="d97"></a>97 | Modal | **Overlay + focus trap** |
| <a id="d98"></a>98 | 右键菜单 | **SemanticEvent::ContextMenu** |
| <a id="d99"></a>99 | 无障碍 | **v2** |
| <a id="d100"></a>100 | 浮层 | **`OverlayStack` 统一管理** |
| <a id="d101"></a>101 | 组件 ID 命名 | **ComponentId**（Generational）已落地；源码 `WidgetId` / `NodeId` 为 `core::ComponentId` 别名 |
| <a id="d102"></a>102 | authoring 宏 | 设计态 **`component!`**；当前实现 **`define_widget!` + `impl_widget_component!`**，重构对齐 [#20](#d20) |
| <a id="d103"></a>103 | measure API | **`measure(constraints)`** 已接；`preferred_size(engine)` 保留为兼容桥，语义等同 [#29](#d29) |
| <a id="d104"></a>104 | 滚动容器 | 统一称 **ScrollView**；决策 [#45](#d45) 中 ScrollContainer 指 ScrollView 实现 |
| <a id="d105"></a>105 | **核心理念（最高规则）** | **用最少资源，做最好效果** — Demand-Driven Zero Idle Work；**统领**六域依赖与一切设计；冲突以本规则为准；细则 [`demand-driven.md`](systems/demand-driven.md) |
| <a id="d106"></a>106 | 主循环三态 | **DeepIdle** / **RegisteredActive** / **Active**；DeepIdle → blocking `wait_event`；**无 LightIdle**；禁止定时 wake 跑 layout/render/tick Effect |
| <a id="d107"></a>107 | 滚动失效 | Scroll 优先 **`Invalidation::Composite`** + `scroll_region` memmove |
| <a id="d108"></a>108 | Picture 策略 | **PicturePolicy 自动推断**（[#122](#d122) 修订）；废弃调用方维护黑名单 |
| <a id="d109"></a>109 | PointerMove | **边界感知窄路径**：drag/capture 全 dispatch；hover 框内不 hit_test；否则 hit_test 且 target 变才 dispatch |
| <a id="d110"></a>110 | 多窗零闲置 | **每窗独立** DeepIdle/RegisteredActive/Active；A 窗 Active 不要求 B 窗 wake |
| <a id="d111"></a>111 | RegisteredActive | 周期工作由**框架**在组件/IME 生命周期内 **自动 register/unregister**（[#124](#d124)）；unregister → DeepIdle |
| <a id="d112"></a>112 | 运行中 Theme | 默认 **不**跟 OS；App **opt-in** `.follow_system_theme(true)` 后框架自动处理 ThemeChanged（[#125](#d125)） |
| <a id="d113"></a>113 | 零闲置豁免 | 无法框架托管的定时/轮询 → **`decisions.md` #160+** 公开条目 + 测试证明；默认不豁免；当前无豁免见 [#158](#d158) |
| <a id="d114"></a>114 | Picture 启用 | **PicturePolicy 自动推断** + 自适应阈值（[#122](#d122) [#129](#d129) [#136](#d136)）；修订旧「黑名单+深度4」 |
| <a id="d115"></a>115 | ActiveWorkRegistry | 框架内部注册表（**App 不可访问** [#124](#d124)）；`next_deadline` / `drain_due` |
| <a id="d116"></a>116 | 多窗单 loop | **WindowSession** 每窗独立树+引擎+三态+Registry；**单** `run_app_loop`；UiEvent 带 **window_id** 路由 |
| <a id="d117"></a>117 | 唤醒调度 | DeepIdle → blocking `wait_event`；有 register → app 层 **`wait_until`**（`wait_timeout(remaining)` [#127](#d127)） |
| <a id="d118"></a>118 | 帧内合并 | Active 帧末 **一次** reconcile；同帧 State/标脏 **coalesce**；Effect 在 reconcile 前 tick |
| <a id="d119"></a>119 | Handle invalidate | `ComponentHandle::invalidate` 默认 **窄 Paint**；Layout 仅结构/约束变更时 |
| <a id="d120"></a>120 | when 谓词 | `HandlerOptions.when` 须 **O(1) 纯函数**；禁止 I/O、alloc、读 State |
| <a id="d121"></a>121 | ContinuousPointerMove | **`WantsContinuousPointerMove`** trait；默认 false；框架不自动推断，仅 opt-in |
| <a id="d122"></a>122 | PicturePolicy 自动推断 | 框架按 widget **能力元数据** 自动 `Never`/`Eligible`；**调用方不维护名单**；新 widget 默认 Never |
| <a id="d123"></a>123 | Handler 智能重绑 | reconcile 时 **仅 handler 变更** 才 `clear_component`+重注册；判定细则 [#135](#d135) |
| <a id="d124"></a>124 | Registry 框架独占 | **禁止** App 直接访问 Registry；App 经 **#132 Timer API** 或内置组件 **间接** register |
| <a id="d125"></a>125 | follow_system_theme | App builder **opt-in** `.follow_system_theme(bool)`，默认 **false**；**true** 时框架监听 ThemeChanged→切换+全窗 palette invalidate |
| <a id="d126"></a>126 | RegisteredActive 脏区 | 关联 paint = `dirty_bounds()` / 目标 widget frame / IME caret rect 并集；框架自动 push Invalidation |
| <a id="d127"></a>127 | wait_until 实现 | app 层 `min(next_deadline)-now` → **单次** `wait_timeout(remaining)`；不要求 native 新 API |
| <a id="d128"></a>128 | 多窗 Theme 广播 | `.theme()` 或 follow_system_theme 切换时，框架 **遍历 WindowSession** 逐树 palette-only invalidate |
| <a id="d129"></a>129 | Picture 自适应阈值 | 替代固定深度4：`node_count≥8` 且 `est_pixels≥65536` 且 PicturePolicy=Eligible（框架常量，非 App 配置） |
| <a id="d130"></a>130 | 开发者零维护 | 调用方 **State/View/opt-in/Timer API**；Picture/Registry/invalidate 由框架自动 |
| <a id="d131"></a>131 | 定时约束 | **禁止**裸 Registry、轮询 Effect、固定 interval 主循环探活；**允许** #132 Timer API 与 async→State |
| <a id="d132"></a>132 | App Timer API | **`run_after` / `run_interval` → TimerHandle`**；框架内 register；**主线程**回调；cancel/drop 自动 unregister |
| <a id="d133"></a>133 | post_to_ui | **`App::post_to_ui(FnOnce)`** / **`AppHandle::post_to_ui`**：跨线程投递；帧内调度见 [#137](#d137) |
| <a id="d134"></a>134 | AppHandle 生命周期 | Timer 绑定 WindowSession；**窗关闭 / run 结束** 自动 cancel 该 session 全部 AppTimer；`AppHandle` drop 不 cancel 已注册 Timer |
| <a id="d135"></a>135 | Handler 变更判定 | 比较 **SemanticKind 集合** + 各条目 **handler_generation**（View build 递增）；**不**比较闭包指针 |
| <a id="d136"></a>136 | PicturePolicy 合并 | `effective = min(metadata, runtime)` — **任一为 Never → Never**；Eligible 须再满足 #129 阈值 |
| <a id="d137"></a>137 | MainThreadQueue | 每 **WindowSession** FIFO；Active 帧内 **UiEvent → drain_due → drain_queue**；入队设 pending **不** register |
| <a id="d138"></a>138 | handler_generation 作者化 | View build **自动** bump；`.on_*` 宏维护；**调用方不可配**；指纹细则 [#142](#d142) |
| <a id="d139"></a>139 | 测试时钟分层 | **`FakeTimer`** = `ITimer` 平台层；**`TestClock`** = App `drain_due` / `wait_until` 注入时钟；二者不混用 |
| <a id="d140"></a>140 | on_start | **`.on_start(FnOnce(AppHandle))`** — 每 WindowSession 创建后、首帧前调用一次；运行中 Timer/post_to_ui 的 **canonical** 注入点 |
| <a id="d141"></a>141 | 多窗 post_to_ui | `AppHandle` 含 **`window_id`**；投递 **仅** 入该 session 队列；**禁止**跨窗；session 已销毁则丢弃闭包 |
| <a id="d142"></a>142 | capture 指纹字段 | hash 仅含 State 身份（[#143](#d143)）+ Copy 值 + AppHandle.window_id；**不含**闭包指针 / 堆地址 / State **value generation** |
| <a id="d143"></a>143 | StateSlotId | `State::new` 分配全局单调 **slot id**；`clone` 共享；指纹用 slot id + `TypeId`；**不用** `generation()` |
| <a id="d144"></a>144 | open_window | **`AppHandle::open_window`** → 新 WindowSession + **新 AppHandle**；可选 `.on_window_start`；不阻塞主 loop |
| <a id="d145"></a>145 | AppState 注册 | **不替代** `State<T>`；mount 时框架自动 register；快照 [#146](#d146) |
| <a id="d146"></a>146 | ComponentConfigSnapshot | mount 时从 widget **静态配置**提取快照；authoring [#151](#d151) |
| <a id="d147"></a>147 | Handle emit | `ComponentHandle::emit` → `WidgetTree::dispatch_semantic`；**同** OS 语义冒泡路径；仅主线程 |
| <a id="d148"></a>148 | open_window 接线 | 副窗 **独立** `ViewAdapter::build` + `WindowSession.tree`；不共享 WidgetTree / reconcile 状态 |
| <a id="d149"></a>149 | update_view | **`AppHandle::update_view(Fn → ViewNode)`** — 仅本 session；帧末 **一次** reconcile（#118） |
| <a id="d150"></a>150 | State 跨窗标脏 | `State::set` **fan-out** 至全部 paint bind；每 bind 仅 wake **所属** WindowSession |
| <a id="d151"></a>151 | SnapshotSource | 自定义 widget **`SnapshotSource` trait**；默认 `component!` 自动提取 `pub` 配置字段 |
| <a id="d152"></a>152 | snapshot(skip) | 字段属性 **`#[snapshot(skip)]`** 排除快照；可作用于 `pub`；与 #151 自动提取 compose |
| <a id="d153"></a>153 | reconcile 合并 | `pending_root` **优先**于 `view_factory`；State 批次 + `update_view` **同帧一次** reconcile |
| <a id="d154"></a>154 | 实现路线图 | 分阶段落地顺序；**#105 三态/Registry 优先**；详见 [roadmap · 分阶段路线图](roadmap.md#分阶段路线图) |
| <a id="d155"></a>155 | view_factory 生命周期 | 每 `WindowSession` **创建时**从 `App::root` 或 `open_window` 根闭包生成 **`Arc<dyn Fn() -> ViewNode + Send + Sync>`**；**会话内不可变**；`State` 批次 reconcile **始终**调用该 factory |
| <a id="d156"></a>156 | update_view 与 factory | `update_view` **仅**写 `pending_root`；**不**替换 `view_factory`；`take()` 消费后下一帧 State  reconcile 仍走原 factory |
| <a id="d157"></a>157 | P0 落地清单 | 按文件路径的 P0 接线表；详见 [roadmap · P0 落地清单](roadmap.md#p0-落地清单) |
| <a id="d158"></a>158 | 零闲置豁免台账 | 当前 **无豁免**；新增豁免必须追加公开条目（当前从 **#160+** 起），并写明触发源、wake 频率、允许工作范围、无法 register 的理由与测试边界 |
| <a id="d159"></a>159 | Handler capture 自动收集边界 | 任意 Rust handler 闭包 **不做运行时自动捕获探测**；稳定复用仅通过显式 capture API，或未来宏 / DSL 在语法层生成 `capture_fingerprint`；禁止执行 handler 做 probe，避免业务副作用、错误事件语义与零闲置破坏 |

新决策追加 **#160+**（含豁免条目）。
