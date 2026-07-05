# 数据系统

> 系统职责：**配置持久化**；不参与 UI 热路径。

---

## 1. 定位

侧向依赖基础设施错误类型；不依赖界面 / 渲染；v1 扁平 KV，无 ORM。

输入：应用选择加载的配置路径与字符串键值。输出：内存配置快照与显式保存结果。

---

## 2. SettingsService

| 项 | 说明 |
|----|------|
| 内存 | 扁平字符串键值 |
| 持久化 | 最小 JSON |
| 脏标记 | 变更后才写入 |

能力：加载、保存、get/set/remove、已加载路径。

---

## 3. 与 App 集成（#64）

- 框架**不强制**  
- App **可选**注入  
- **默认不**自动 load/save  

---

## 4. 与主题 / locale

持久化 theme_mode、brand 等字符串 key；App 解析为 Theme。locale v2 同理。缺文件用 DefaultTheme。

---

## 5. 约束

键名建议命名空间；v1 值均为字符串；单线程 GUI。

---

## 6. 落地要求

| 项 | 要求 |
|----|------|
| SettingsService | 保持轻量扁平 KV 与 JSON 保存，不引入 ORM |
| App 集成 | 可选注入，默认不自动 load/save，由 App 明确决定何时读写 |
| 主题 / locale | 以字符串 key 持久化，由 App 解析为 Theme / locale 对象 |

---

## 7. 相关决策

#64 — [decisions.md](../decisions.md)
