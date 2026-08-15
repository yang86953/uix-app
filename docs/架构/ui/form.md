# form 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 ui 系统的表单模型、字段绑定协议、校验与提交契约。依赖：[component](component.md)、[reactive](reactive.md)、[event](event.md)。导出：`FormModel`、与具体控件无关的字段适配协议和校验结果，供 widgets 组合。
>
> **当前实现线索**：业务模型相关能力位于 `src/ui/form/`（form.rs、model_form.rs 等），表单视觉组件位于 `src/ui/widgets/input/`；重构后业务模型归本模块，视觉组件仍归 widgets。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `FormBuilder` | struct | 声明字段 schema、初值、规则和依赖 |
| `FormModel` | struct | 持有字段真值、激活状态、错误、提交与重置 |
| `Values` | typed snapshot | 向校验器提供整表不可变类型化读取 |
| `Trigger` | enum | 定义 OnSubmit、OnChange、OnBlur 校验边界 |
| `FieldError` | struct | 表达字段当前第一条有效错误 |
| `FormListModel` / `FormListItemId` | struct/value | 管理动态行的稳定身份和值/错误隔离 |
| field adapters | structs | 连接受控组件、`State<T>`、焦点和字段模型 |

## 组件：FormModel

`FormModel` 是字段值和业务校验的唯一真相；Form widget 只负责布局与视觉。字段变化按依赖图对每个受影响字段至多重验一次，循环依赖不得造成重复执行或无限递归。

## 组件：field adapters

适配器把组件 Change/FocusOut、外部 `State<T>`、字段值和 `FocusHandle` 组合起来。类型化滑块适配器只投影 `State<f64>`、范围、步长与标签，校验、重置和提交写回仍由 `FormModel` 统一拥有。提交失败只登记首错的 focus-and-reveal 命令，由目标窗口下一 UI 轮次完成焦点和最近 viewport 的最小滚动。

## 组件：FormListModel

动态列表使用单调且删除后不复用的行身份参与 keyed reconcile。未删除行保留组件身份；移除行的值、错误、handler 和焦点绑定必须一并清理。

## 模块不变量

- validator、reset binding 和 focus binding 不进入组件快照或后台线程。
- 同步校验不执行 I/O；异步业务校验由应用显式编排和取消。
- 校验本身不登记 timer、帧机会或跨窗工作。
