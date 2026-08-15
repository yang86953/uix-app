# view 模块

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统的声明式 View、组合器和 reconcile 输入。依赖：[component](component.md)、[reactive](reactive.md)。导出：`View`、`ViewNode`、combinator 与组件 DSL；theme 等上下文通过通用构建上下文扩展。
>
> **当前实现线索**：相关实现暂位于 `src/ui/view/`、`src/ui/macros/` 和 `src/ui/component/` 上下文文件；重构后以本模块边界为准。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `View` | trait | 一次性构建 ViewNode |
| `ViewNode` | struct | 组件类型、key、子树、handler 与 Provider 上下文 |
| `ViewAdapter` | internal adapter | ViewNode 到 WidgetNode/patch 的桥 |
| combinators | functions/builders | 声明组件、样式、事件、动画与子树 |
| `component!` 等宏 | macro | 生成组件样板和能力实现 |

## 组件：ViewNode

ViewNode 是声明快照，不是持久组件。reconcile 以父范围内“同类型 + 同 key”保留 ComponentId 和运行态；类型或 key 变化才销毁重建。

`userSelect` 属于树结构元数据而非视觉 `Style`。`ViewNode` 保存声明值，adapter 写入 `WidgetNode`，`WidgetTree` 再按祖先边界解析实际值并同步到文字组件；reconcile 改变策略时必须重算既有子树并清理失效选择。

## 组件：ViewAdapter

adapter 把声明属性分类为结构、Layout、Paint 或 Composite patch；同 key reconcile 不应覆盖组件用户运行态。

## 模块不变量

View build 可捕获 State/Provider 依赖，但不执行 present、平台 I/O 或后台任务；业务闭包不进入可快照组件 struct。
