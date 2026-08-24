# widget_runtime 模块（组件运行时框架）

[← 返回架构索引](../../架构.md)

> **接口**：声明 ui System 的持久组件树、基础组件能力和树级运行态。基础依赖：[core/identity](../core/identity.md)、[core/geometry](../core/geometry.md)。导出：由 ui System 编排给 view、reactive、layout、event 等 Module 的组件身份、树生命周期和阶段挂载点；不授予兄弟 Module 实例依赖。
>
> **术语与命名（SMC-07 语义基线）**：「component」在本仓库代码与架构文档中只指 SMC
> 三层中的 **Component**——单一职责、窄契约、独立替换边界明确的原子代码单位
> UI 实体一律称为 widget：`Widget` 契约、`WidgetId` 身份、`WidgetHandle`、widgets 库与
> UIX Lang `Widget` 声明均不属于该层。ui System 的 widget 运行时框架使用独立的
> `widget_runtime` Module 名称，不得把该 Module 与 SMC Component 层混同；迁移记录见
> [Gitea Issue #4](http://100.79.245.29:3000/admin/uix-app/issues/4)。
>
> **当前实现线索**：相关实现已收敛于 `src/ui/widget_runtime/`（traits.rs、managers/ 及各 handle 文件）；重构后保持本模块边界稳定。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `WidgetTree` | struct | 持有单窗组件树、索引、side table、dirty 与生命周期 |
| `WidgetNode` | internal struct | 组件身份、父子关系、frame、可见性和能力入口 |
| `Widget` | trait | 组件基础身份、类型和生命周期契约 |
| `WidgetStateStore` | internal state store | 按窗口、静态声明和实际实例路径拥有 UIX widget 私有状态槽 |
| phase extension slots | internal registry | 允许 layout、event、painting 等模块挂接各自的小型能力 |
| `WidgetManagers` | internal struct | 聚合焦点、交互、文本、拖拽等树级运行态 |
| `WidgetHandle` / `FocusHandle` | handles | 跨作用域引用组件并向所属窗口投递受控操作 |

## 组件：WidgetTree

`WidgetTree` 是单个窗口 UI 线程独占的持久运行态。它拥有节点和 side table，并提供 reconcile、layout、event、paint、semantics 等阶段的受控挂载点；具体算法由对应模块扩展，widget_runtime 不反向依赖这些上层模块。后台线程不得直接持有或访问树内可变对象。

协调分为“预检”和“发布”两段。View 捕获、身份检查和不触碰现有树的展开属于预检；预检失败只丢弃候选资源。首次改写节点结构或树级附件后即进入发布段，最外层事务成功结束是唯一对外线性化点。发布段发生 panic 时不得继续使用半提交树，`WidgetTree` 永久进入 fail-stop，只允许所属窗口执行受控关闭并创建全新的树。

fail-stop 树拒绝 reconcile、动态 renderer、event、timer、Effect、animation、layout、semantics 和 paint，不向调用方暴露部分结构。它不尝试撤销用户生命周期或 Effect 已经产生的外部副作用；框架只保证未提交回执回滚，并在关闭时释放 State 租约、Effect、handler、renderer side table、动画 owner 和节点资源。

## 组件：WidgetStateStore

`WidgetStateStore` 由单个 `WidgetTree` 唯一拥有，只接受 UIX 编译器生成的窄作用域值与字段编号，不取得 For、View 展开或协调流程所有权。静态 `Widget` 调用先形成声明基座；调用位于 For 时，编译器再以完整嵌套业务 key 或位置路径派生实际实例作用域，并在对应迭代内建立 props、private state、computed 与事件适配器。同一作用域在成功 reconcile 中复用状态槽；节点标记不再承载的作用域在最外层协调事务结束时清理，未提交捕获通过回执回滚。

## 组件：Widget

`Widget` 只定义阶段无关的基础契约；layout、event、painting、animation 等模块各自定义小型扩展能力，并通过受控 slot 挂接，避免 widget_runtime 反向引用所有阶段类型或形成巨型接口。业务闭包、虚拟列表 renderer 等不可快照对象按 `WidgetId` 存入由树托管的 side table，不进入组件值本身。

## 组件：WidgetManagers

焦点、hover/pressed、拖拽、文本覆写等都是树级横切状态，不形成独立业务模块。节点隐藏、移除或 generation 失效时，相关 capture、focus、IME、overlay、handler、timer 和 manager 覆写必须一起清理。

## 组件：WidgetHandle / FocusHandle

handle 只保存带 generation 的目标身份和所属窗口投递能力，不借出 `WidgetTree`。跨线程调用成功表示操作已进入目标队列，不表示 UI 回调已经执行或画面已经提交。

## 模块不变量

- 声明式 View 不是第二棵持久树；运行态只有 `WidgetTree` 一份。
- 同父级“同类型 + 同 key”保留组件身份，类型或 key 改变才执行销毁与重建。
- present 成功前不消费相应 dirty；节点销毁前完成所有树级引用清理。
- fail-stop 状态不可重置；恢复只能由窗口 owner 丢弃旧树并建立新的独占运行态。
