# theme 模块

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统的主题令牌、组件配置、本地化与子树上下文。依赖：[view](view.md)、[graphics/geometry](../graphics/geometry.md)。导出：组件解析视觉值、行为默认值和内置文案的统一 Provider 契约。
>
> **当前实现线索**：相关实现暂分布于 `src/ui/theme/`（traits.rs 同目录）、`src/ui/component/config.rs`、`src/ui/component/provider_context.rs` 和 `src/ui/component/locale.rs`；重构后应以本模块提供统一上下文。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `ThemePrimitives` | struct | 保存品牌、语义、背景、文字和边框基色 |
| `DesignTokens` / `Theme` | struct | 提供运行时语义视觉令牌 |
| `ThemeTokens` / `TokenProvider` | trait | 定义组件读取令牌的最小契约 |
| `TokenPatch` | struct | 只覆写显式字段，其余委托父主题 |
| `ComponentConfig` | struct | 保存组件行为与尺寸默认值 |
| `ConfigProvider` | View component | 覆写子树中的组件配置或 token patch |
| `Locale` / `LocaleProvider` | struct/View component | 提供内置组件文案与子树本地化覆写 |
| `ProviderContext` | internal value | 绑定节点构建、测量、绘制和语义阶段的同一上下文 |

## 组件：ThemePrimitives / DesignTokens

Primitives 通过稳定推导规则生成文本、背景、边框、状态、间距、圆角和阴影等 `DesignTokens`。色板按需生成；graphics 只消费解析后的颜色、字号、圆角与阴影，不知道 Theme 或组件类型。

## 组件：ConfigProvider / TokenPatch

优先级为组件局部属性 > 最近 Provider > App 根上下文 > 框架预设。Provider 只覆盖显式字段；配置变化按字段分类为 reconcile、Layout 或 Paint，不能无条件重建整棵树。

## 组件：LocaleProvider

节点捕获最近 Locale，measure、render、事件提示和无障碍名称读取同一上下文，避免显示文案与语义文案不一致。系统语言变化只有在应用显式接受后才更新根上下文。

## 组件：ProviderContext

`ProviderContext` 是构建时向子树传递的不可变快照，不是全局可变单例。后续组件阶段恢复同一上下文，节点移除后释放引用；Provider 不创建独立组件树或工作循环。

## 模块不变量

- Theme、组件配置和 Locale 属于 ui；graphics 与 platform 不反向依赖 Provider。
- 设置持久化属于 data，theme 只持有当前运行时上下文。
- Provider 继承只有一套优先级和一份有效值，不建立 manager/config 的平行真相。
