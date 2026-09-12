# theme 模块

[← 返回架构索引](../../架构.md)

本模块拥有通用样式、主题值读取和不可变覆盖。设计系统定义字段名、默认值和派生规则；graphics 只消费解析后的颜色、文字和几何值。

| 契约 | 职责 |
| --- | --- |
| `ThemeTokens::value(key)` | 按命名空间键读取 `TokenValue`，未知键返回 `None` |
| `TokenValue` | 颜色、数值、静态字符串、布尔值与阴影层列表 |
| `Theme` / `TokenProvider` | 持有可共享的运行时主题提供器 |
| `TokenPatch` | 覆盖明确登记的键，未提及值委托原主题 |
| `ColorValue::token` / `TypographyToken::Token` | 保留运行时键与缺失时的回退值 |
| `Style` / `StyleDiff` | 通用视觉声明、字段合并与差异 |
| `StyleScope` / `WidgetTokenOverrides` | 子树主题与按组件类型选择的通用令牌覆盖 |

`Theme::light()` / `dark()` 只给出中性明暗、前景与背景；字体、选择和滚动条等基础值通过 `uix.*` 键读取并有独立回退值。`DesignTokens`、`ThemePrimitives`、Ant Design 字段、按钮样式、`WidgetConfig`、`Locale` 由 `uix-widgets` 拥有，框架不引用该包。

第三方提供器只需实现 `ThemeTokens`。`layout_fingerprint()` 是可选优化：仅当两个指纹都存在且相等时，可以确认主题切换不改变几何；未声明时保守重布局。库自定义键不需要修改框架 trait、编译器或 LSP。

## 上下文

`ProviderContext` 是由类型索引的不可变快照。`ContextProvider<T>`、`with_context`、`use_context` 按值类型继承；内层同类型覆盖外层，不同类型互不影响。值要求 `Clone + PartialEq + Send + Sync + 'static`，读取的缺省值由该类型自己的 `Default` 决定。

节点保留构建时的快照，协调、测量、绘制、事件及动态内容使用所属节点的上下文。作用域结束或 panic 展开都恢复调用方；窗口间不使用隐式全局配置。`App::context(value)` 设置窗口根上下文；组件库的 `.config()` / `.locale()` 扩展通过这个通用入口安装自己的值。

## 绘制边界

`Style` 不执行文件 I/O。`theme/painting` 解析样式，按背景色、背景图层、边框的顺序调用 graphics；整体透明度包裹表面。图片解码、字体、缓存和代际句柄仍由 graphics resources 拥有。主题不持有平台窗口或持久化服务。
