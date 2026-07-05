# View 与响应式系统

> 系统职责：声明式 **View** 如何描述 UI，**State** 如何驱动更新与 diff。

---

## 1. 定位

| 原则 | 说明 |
|------|------|
| 唯一入口 | 业务**仅**通过 View DSL 创建 UI（#21） |
| 声明式 | State 变 → 重建 View 树 |
| 细粒度 diff | 同 key 同类型 → 字段级比较（#60） |

---

## 2. View 组成

声明式节点树、根构建器、组合器：column、row、button、label、scroll、grid 等。

---

## 3. 构建流程

View 构建 → 捕获 State 依赖 → 生成组件树 → 绑定 orphan State / Effect。

---

## 4. Reconciler（#49、#60、#62）

| 步骤 | 行为 |
|------|------|
| 匹配 | 按 **key**（无 key 则同层索引） |
| 类型不同 | 卸载旧节点，新建 ComponentId |
| 类型相同 | 比较 Style、文本、静态 props |
| Style 变 | 写回 + paint invalidate |
| handler | **一律重新注册**（闭包不比相等） |
| 子树 | 递归 |
| 移除 | unmount + 释放 HandlerTable |

---

## 5. 响应式原语

| 概念 | 角色 |
|------|------|
| State | 变更 → paint invalidate（#24） |
| Computed | 依赖追踪；**保留**（#78） |
| Effect | 帧末副作用；**禁止**直接改 UI，须经 State（#79） |

Handler 须 static；业务数据经 AppState + ComponentHandle（#32）。

Layout 失效不直接 present。

---

## 6. 与组件树

View reconciler 产出 / 更新 **Generational ComponentId** 组件树（#35），供布局、事件、绘制消费。

---

## 7. 落地要求

| 项 | 要求 |
|----|------|
| View DSL | `column`、`row`、`button`、`dynamic_label` 等为业务侧首选入口 |
| State | `State<T>`、Computed、Effect 与 reconciler / HandlerTable 完整联动 |
| Reconciler | 同 key 同类型字段级 diff，handler rebuild 重新注册 |
| View-only 入口 | 业务代码仅通过 View DSL 创建 UI；底层 Widget API 面向框架和高级组件 |

---

## 8. 相关决策

#21、#24、#31、#35、#49、#60–#62、#78–#79 — [decisions.md](../decisions.md)
