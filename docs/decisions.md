# 设计决策索引

> **唯一决策台账**（#1–#100，2026-07-05 定稿）。锚点 `#d{N}`。正文 → [`systems/`](systems/) · [Main.md](Main.md)

## 实现顺序（[#50](#d50)）

1. Theme（120 chromatic + 13 neutral + TypographyScale）  
2. Style（StyleSet / ColorValue / TypographyToken）  
3. Event（System / Semantic / Custom）  
4. HandlerTable  
5. Lifecycle  
6. Button  
7. 全量重写内置 Widget + demo（[#58](#d58)、[#80](#d80)）  

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
| <a id="d10"></a>10 | 绑定存储 | **HandlerTable 挂 ComponentId** |
| <a id="d11"></a>11 | 12 主色命名 | **Ant Design 同款**（Red … Magenta） |
| <a id="d12"></a>12 | 衍生色算法 | **移植 Ant Design 官方色板算法** |
| <a id="d13"></a>13 | 中性色引用 | **NeutralRole** 15 角色 → 13 阶映射 |
| <a id="d14"></a>14 | StyleSet 五态与优先级 | **五态** normal/hover/pressed/focused/disabled；**disabled > focused > pressed > hover > normal** |
| <a id="d15"></a>15 | StyleSet 继承 | **差异字段 + 继承 normal** |
| <a id="d16"></a>16 | Button 变体 | **StyleSet 预设**，无 variant enum |
| <a id="d17"></a>17 | Theme 范围 | **Light/Dark + 可换种子**；**[#57](#d57) 两档 API** |
| <a id="d18"></a>18 | 焦点模型 | **单焦点 + tab_index 焦点链** |
| <a id="d19"></a>19 | 视口检测 | **与祖先 clip/scroll 求交** |
| <a id="d20"></a>20 | 组件 authoring | **`component!` 宏** |
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
| <a id="d31"></a>31 | View 更新 | **声明式重建 + diff** 复用 ComponentId |
| <a id="d32"></a>32 | 业务数据 | **AppState + ComponentHandle** |
| <a id="d33"></a>33 | Theme 作用域 | **App 全局**；**多窗共享**（[#93](#d93)–[#94](#d94)） |
| <a id="d34"></a>34 | danger 预设 | **Primary::RED** |
| <a id="d35"></a>35 | ComponentId | **Generational ID** |
| <a id="d36"></a>36 | Click payload | **`{ button, pos, modifiers }`** |
| <a id="d37"></a>37 | disabled 视觉 | **opacity + 中性色** |
| <a id="d38"></a>38 | measure 约束 | **`Constraints { min, max, definite }`** |
| <a id="d39"></a>39 | 预设命名 | **`StyleSet::button_*()`** |
| <a id="d40"></a>40 | 组件测试 | **FakePlatform + 语义断言 + paint snapshot** |
| <a id="d42"></a>42 | 字体 | **Theme TypographyScale** |
| <a id="d43"></a>43 | 边框 v1 | **`border_width: EdgeInsets` + border_color** |
| <a id="d44"></a>44 | 阴影 v1 | **Style.shadow 可选** |
| <a id="d45"></a>45 | 滚动 | **ScrollContainer 消化 Wheel** |
| <a id="d46"></a>46 | i18n v1 | **不做**，事件预留 |
| <a id="d47"></a>47 | button_ghost | **透明底 + Primary 边框/文字** |
| <a id="d48"></a>48 | 默认 Theme | **内置 DefaultTheme** |
| <a id="d49"></a>49 | View diff | **`.key()` + [#60](#d60) 细粒度 diff** |
| <a id="d50"></a>50 | 实现顺序 | 见上文 |
| <a id="d51"></a>51 | TypographyScale | **Ant Design ~10 档** + **TypographyToken** |
| <a id="d52"></a>52 | NeutralRole | **15 角色**，Light/Dark 各一套映射 |
| <a id="d53"></a>53 | v1 布局 | **Flex + Grid** |
| <a id="d55"></a>55 | AppState | **App 单例持有** + Handle |
| <a id="d56"></a>56 | border_width | **EdgeInsets 四边** |
| <a id="d57"></a>57 | 品牌主题 | **`with_brand_primary` / `from_primaries([12])`** |
| <a id="d58"></a>58 | Widget 落地 | **Big Bang 全量重写** |
| <a id="d59"></a>59 | 默认引擎 | **GPU 优先，失败回退 CPU** |
| <a id="d60"></a>60 | View diff 粒度 | **Style/文本/handler 字段比较** |
| <a id="d61"></a>61 | ComponentHandle | **只读配置** + invalidate/emit |
| <a id="d62"></a>62 | Handler diff | **rebuild 一律重新注册** |
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
| <a id="d76"></a>76 | OS 主题监听 | **运行中自动切换 Theme** |
| <a id="d77"></a>77 | button() 默认 | **`button_default()`**；链式 `.primary()` 等 |
| <a id="d78"></a>78 | Computed | **保留** |
| <a id="d79"></a>79 | Effect | **保留**；禁止 Effect 内直接改 UI |
| <a id="d80"></a>80 | demo | **与 Button 同时重写** |
| <a id="d81"></a>81 | Flex 对齐 | **Style**：align_items / justify_content / gap |
| <a id="d82"></a>82 | 离屏缓存 | **深度启发式 Picture**（[#86](#d86)–[#87](#d87)） |
| <a id="d83"></a>83 | 动画 | **AnimationRegistry** 驱动动画帧 |
| <a id="d84"></a>84 | Grid 轨道 | **Style grid_template_*** |
| <a id="d86"></a>86 | Picture 条件 | **子树深度** |
| <a id="d87"></a>87 | 深度阈值 | **≥ 4 层** |
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

新决策追加 **#101+**。
