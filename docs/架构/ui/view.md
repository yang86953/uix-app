# view 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 ui System 的声明式 View、组合器和 reconcile 输入。与[组件运行时框架](widget_runtime.md)及[reactive](reactive.md)的协作由 ui System 编排，Module 间不直接持有实例。导出：`View`、`ViewNode`、combinator 与组件 DSL；theme 等上下文通过通用构建上下文扩展。
>
> **当前实现线索**：相关实现暂位于 `src/ui/view/`、`src/ui/macros/` 和 `src/ui/widget_runtime/` 上下文文件；重构后以本模块边界为准。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `View` | trait | 一次性构建 ViewNode |
| `ViewNode` | struct | 组件类型、key、子树、handler 与 Provider 上下文 |
| `ViewAdapter` | internal adapter | ViewNode 到 WidgetNode/patch 的桥 |
| combinators | functions/builders | 声明组件、样式、事件、动画与子树；`scoped` 声明节点级重建粒度的子树作用域 |
| `widget!` 等宏 | macro | 生成 widget 样板和能力实现 |

## 组件：ViewNode

ViewNode 是声明快照，不是持久组件。reconcile 以父范围内“同类型 + 同 key”保留 WidgetId 和运行态；类型或 key 变化才销毁重建。

ViewNode 保存 owned 声明值、稳定 key 和 side-table 登记签名，不拥有 `WidgetNode`、平台对象或可变 handler。相同父范围内重复 key 属于歧义输入，必须在改写现有树前返回 typed error，不能按遍历顺序任意接管旧身份。

`userSelect` 属于树结构元数据而非视觉 `Style`。`ViewNode` 保存声明值，adapter 写入 `WidgetNode`，`WidgetTree` 再按祖先边界解析实际值并同步到文字组件；reconcile 改变策略时必须重算既有子树并清理失效选择。

`position` 与四边 inset 同样属于树结构元数据。`ViewNode` 保存完整声明，adapter 在首次挂载和同 key reconcile 时交接给 `WidgetTree`；树级布局 Module 负责正常流分类、absolute 包含块、fixed 根视口和 sticky 滚动约束，绘制、脏区与命中复用同一定位视觉变换。fixed 只提升合成坐标路径，不转移组件生命周期所有权。

## 组件：ViewAdapter

adapter 把声明属性分类为结构、Layout、Paint 或 Composite patch；同 key reconcile 不应覆盖组件用户运行态。

## 模块不变量

目标中的[Lisp 动态界面](../app/extensions.md#7-状态迁移与动态界面)通过受控挂载位提交 owned 声明，继续由 ui System 校验并协调；ui 不解释 Lisp 或编译器 IR。扩展代控制回调有效性，稳定逻辑 key 控制 widget 复用，两者不混用。挂载位与事务回执是待实现能力，不扩大当前 View API 的成功或原子性承诺。

- View build 可捕获 State/Provider 依赖，但不执行 present、平台 I/O、阻塞业务 I/O 或启动后台任务；失败发生在预检阶段时不得部分改写持久树。
- 业务闭包只进入所属树管理的 side table，ViewNode/组件快照只保存签名或稳定登记身份；节点销毁、换根和 generation 变化同步释放登记。
- adapter 输出 patch 只表示声明差异已分类；协调发布、布局、绘制和呈现各自由后续结果建立。
