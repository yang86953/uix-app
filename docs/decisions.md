# 设计决策索引

> **唯一决策台账**：共 **100 项**（#1–#100），2026-07-05 定稿。  
> 设计正文见 [`systems/`](systems/) 各文档；总目录见 [Main.md](Main.md)。  
> **后写入决策覆盖先前冲突项**——见下方「废止关系」。

阅读约定：本文件记录**定稿设计取舍**。源码若与本文件冲突，按较新的决策重构源码。

---

## 废止关系（冲突以右为准）

| 早期 # | 原决策 | 被谁废止 |
|--------|--------|----------|
| 41 | v1 仅 Flex | **#53** Flex + Grid |
| 14 | disabled > pressed > hover | **#54** 增加 focused 层 |
| #33 单窗隐含 | Theme/App 单窗 | **#93–#94** v1 多窗共享 Theme |
| RepaintBoundary | 显式离屏组件 | **#82** 深度启发式自动 Picture |
| Lifecycle::tick | 组件 impl tick | **#85** 仅 AnimationRegistry |
| Event::wants_frame | 组件声明要帧 | **#85** 同上 |

---

## 实现顺序（#50）

1. Theme（120 chromatic + 13 neutral + TypographyScale）  
2. Style（StyleSet / ColorValue / TypographyToken）  
3. Event（System / Semantic / Custom）  
4. HandlerTable  
5. Lifecycle  
6. Button  
7. 全量重写内置 Widget + demo（#58、#80）  

---

## 分类索引

| 分类 | 相关决策 |
|------|----------|
| 主题 / 样式 | #1–#3、#9、#11–#17、#27–#28、#34、#37、#39、#42–#44、#47–#48、#51–#57、#63、#65、#74、#76、#81、#84 |
| 事件 / Handler | #4–#7、#10、#25–#26、#36、#40、#61–#62、#66、#68、#71、#90–#92、#95、#98 |
| View / 响应式 | #21、#24、#31–#32、#35、#49、#60、#78–#79 |
| 组件 / 生命周期 | #8、#18–#20、#22–#23、#29–#30、#58、#69、#72、#77、#83、#85 |
| 布局 / 滚动 | #19、#38、#41、#45、#53、#67、#73、#81、#84 |
| 渲染 / 动画 | #59、#70、#82、#85–#87 |
| 应用 / 平台 / 数据 | #55、#59、#64、#70、#74、#76、#88–#89、#93–#94 |
| 浮层 / 无障碍 | #96–#100 |

---

## 决策全表

| # | 议题 | 决策 |
|---|------|------|
| 1 | 主色在衍生色中的位置 | **第 6 阶**；chromatic **12×10=120** |
| 2 | 中性色 | **13 阶**白→黑，独立 scale |
| 3 | Custom 色标记 | **debug lint / clippy 警告** |
| 4 | 事件传播 | **Bubble 默认**，Capture opt-in |
| 5 | 语义事件 | **可扩展注册表** + 内置常用事件 |
| 6 | CustomEvent payload | **typed struct + TypeId / 宏** |
| 7 | 多 handler | **顺序执行，可 stop**（#68 扩展 stop 语义） |
| 8 | Active/Inactive | **v1 就要**（Focus + 视口） |
| 9 | 主题切换重绘 | **只 invalidate 用 Palette 的组件** |
| 10 | 绑定存储 | **HandlerTable 挂 ComponentId** |
| 11 | 12 主色命名 | **Ant Design 同款**（Red … Magenta） |
| 12 | 衍生色算法 | **移植 Ant Design 官方色板算法** |
| 13 | 中性色引用 | **NeutralRole** 15 角色 → 13 阶映射 |
| 14 | StyleSet 优先级 | **disabled > focused > pressed > hover > normal** |
| 15 | StyleSet 继承 | **差异字段 + 继承 normal** |
| 16 | Button 变体 | **StyleSet 预设**，无 variant enum |
| 17 | Theme 范围 | **Light/Dark + 可换种子**；**#57 两档 API** |
| 18 | 焦点模型 | **单焦点 + tab_index 焦点链** |
| 19 | 视口检测 | **与祖先 clip/scroll 求交** |
| 20 | 组件 authoring | **`component!` 宏** |
| 21 | 使用入口 | **只用 View DSL** |
| 22 | Button 尺寸 | **StyleSet 预设** + TypographyToken |
| 23 | Ripple | **v1 不做** |
| 24 | 响应式 State | **`State<T>` 绑定** → auto invalidate |
| 25 | 语义事件冒泡 | **默认 Bubble** |
| 26 | Handler 生命周期 | **闭包 `'static`** + AppState/Handle |
| 27 | 组件侧 Custom | **仅 debug lint** |
| 28 | 默认品牌主色 | **Primary::BLUE（8）** |
| 29 | Layout 方法名 | **`measure(constraints) -> Size`** |
| 30 | Button icon/loading | **v1 纯文本** |
| 31 | View 更新 | **声明式重建 + diff** 复用 ComponentId |
| 32 | 业务数据 | **AppState + ComponentHandle** |
| 33 | Theme 作用域 | **App 全局**；**#94 多窗共享** |
| 34 | danger 预设 | **Primary::RED** |
| 35 | ComponentId | **Generational ID** |
| 36 | Click payload | **`{ button, pos, modifiers }`** |
| 37 | disabled 视觉 | **opacity + 中性色** |
| 38 | measure 约束 | **`Constraints { min, max, definite }`** |
| 39 | 预设命名 | **`StyleSet::button_*()`** |
| 40 | 组件测试 | **FakePlatform + 语义断言 + paint snapshot** |
| 41 | 布局 v1 | ~~仅 Flex~~ → **#53** |
| 42 | 字体 | **Theme TypographyScale** |
| 43 | 边框 v1 | **`border_width: EdgeInsets` + border_color** |
| 44 | 阴影 v1 | **Style.shadow 可选** |
| 45 | 滚动 | **ScrollContainer 消化 Wheel** |
| 46 | i18n v1 | **不做**，事件预留 |
| 47 | button_ghost | **透明底 + Primary 边框/文字** |
| 48 | 默认 Theme | **内置 DefaultTheme** |
| 49 | View diff | **`.key()` + #60 细粒度 diff** |
| 50 | 实现顺序 | 见上文 |
| 51 | TypographyScale | **Ant Design ~10 档** + **TypographyToken** |
| 52 | NeutralRole | **15 角色**，Light/Dark 各一套映射 |
| 53 | v1 布局 | **Flex + Grid** |
| 54 | StyleSet focused | **focused 层** + 优先级见 #14 |
| 55 | AppState | **App 单例持有** + Handle |
| 56 | border_width | **EdgeInsets 四边** |
| 57 | 品牌主题 | **`with_brand_primary` / `from_primaries([12])`** |
| 58 | Widget 落地 | **Big Bang 全量重写** |
| 59 | 默认引擎 | **GPU 优先，失败回退 CPU** |
| 60 | View diff 粒度 | **Style/文本/handler 字段比较** |
| 61 | ComponentHandle | **只读配置** + invalidate/emit |
| 62 | Handler diff | **rebuild 一律重新注册** |
| 63 | focused 时机 | **仅 :focus-visible** |
| 64 | Settings | **App 可选注入**，默认不自动存盘 |
| 65 | font_family | **系统字体栈** |
| 66 | 语义注册表 | **SemanticKind enum + register_semantic!** |
| 67 | Grid DSL | **`grid(...)` 组合器** |
| 68 | handler stop | **stop 后续 handler + 语义冒泡** |
| 69 | prelude | **精选高频符号** |
| 70 | HiDPI | **scale_factor**，逻辑 px |
| 71 | 剪贴板 v1 | **SystemEvent → 语义事件** |
| 72 | Handle 可读 | **仅配置字段** |
| 73 | Scroll DSL | **`scroll(...).vertical().horizontal()`** |
| 74 | Light/Dark | **默认跟 OS**；App 可显式覆盖 |
| 75 | 文本输入 v1 | **单行 + 多行 + 完整 IME** |
| 76 | OS 主题监听 | **运行中自动切换 Theme** |
| 77 | button() 默认 | **`button_default()`**；链式 `.primary()` 等 |
| 78 | Computed | **保留** |
| 79 | Effect | **保留**；禁止 Effect 内直接改 UI |
| 80 | demo | **与 Button 同时重写** |
| 81 | Flex 对齐 | **Style**：align_items / justify_content / gap |
| 82 | 离屏缓存 | **深度启发式 Picture**；无 RepaintBoundary |
| 83 | 动画 | **AnimationRegistry** |
| 84 | Grid 轨道 | **Style grid_template_*** |
| 85 | tick / wants_frame | **废除**；仅 Registry |
| 86 | Picture 条件 | **子树深度** |
| 87 | 深度阈值 | **≥ 4 层** |
| 88 | AppState 线程 | **仅主线程** |
| 89 | 错误 UI | **可选 Toast**；默认 log |
| 90 | once_click | **首次触发后移除** |
| 91 | `.when()` | **每次派发前求值** |
| 92 | preventDefault | **语义层 default_prevented** |
| 93 | 多窗口 | **v1 多窗**；共享 AppState + Theme |
| 94 | 多窗 Theme | **全局共享** |
| 95 | 拖放 | **SemanticEvent::FileDrop** |
| 96 | Tooltip | **v1 完整组件** |
| 97 | Modal | **Overlay + focus trap** |
| 98 | 右键菜单 | **SemanticEvent::ContextMenu** |
| 99 | 无障碍 | **v2** |
| 100 | 浮层 | **`OverlayStack` 统一管理** |

---

## 待讨论

（暂无开放项。新决策追加为 **#101+**，并更新废止关系与本表。）
