# 数据系统

← [Main](../Main.md) · 系统 **#11** · 功能域：`data`

> 配置持久化；不参与 UI 热路径（符合核心理念 #105 冷路径边界）。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| API | [SettingsService](#settingsservice) | #64 |
| 格式 | [JSON 格式](#json-格式) | — |
| App 集成 | [App 集成](#app-集成) | #64 #48 |
| 约定 key | [Key 约定](#key-约定) | #74 #57 |
| 约束 | [约束](#约束) | #88 #105 |

**关联**：[application](application.md) · [theme-style](theme-style.md) · [demand-driven](demand-driven.md)

---

## SettingsService

`src/data/settings/settings.rs` — 扁平字符串 KV 存储。

```rust
struct SettingsService {
    path: Option<String>,
    values: HashMap<String, String>,
    dirty: bool,
}
```

| API | 行为 |
|-----|------|
| `load(path)` | 设置路径；文件不存在 = 空 map（OK） |
| `save()` | dirty 才写盘；无 path 则 no-op |
| `get(key)` | `Option<&str>` |
| `get_or(key, default)` | 带默认值 |
| `set(key, val)` | 标记 dirty |
| `remove` / `clear` | 删除 |
| `all()` | 全部 KV 迭代 |
| `loaded_path()` / `dirty()` / `count()` |  introspection |

错误：`core::Error`（`Errc::FormatError` 等）；不 panic。

---

## JSON 格式

- **扁平对象**：`{"key": "value", ...}`
- 值类型 v1 **均为 string**
- 自定义 parser（无 serde 依赖）
- save 时 pretty-print（2 空格缩进）

示例：

```json
{
  "theme_mode": "dark",
  "brand_primary": "#1677ff"
}
```

---

<a id="app-集成"></a>

## App 集成

> **启动 vs 运行中**（[#74](../decisions.md#d74) · [#125](../decisions.md#d125)）
>
> | 阶段 | 行为 |
> |------|------|
> | **启动** | 无 Settings → DefaultTheme；`theme_mode="system"` 或显式配置可读 OS 初始明暗 |
> | **运行中** | 默认不跟 OS；须 `.follow_system_theme(true)` 才自动跟 OS 切换 |

设计（#64）— opt-in 路径见 [roadmap · 实现进度总览](../roadmap.md#实现进度总览)：

```text
App builder（opt-in）
    → App::settings(path)        // 配置持久化路径
    → run() 进入 GUI/CLI 前 load 一次，并注册 SettingsService 到 DI
    → 读 theme_mode / brand_primary
    → 构造 Theme::antd_* / with_brand_primary
    → App::theme(theme)

运行中变更
    → settings.set(...)
    → settings.save()            // 非自动；业务显式调用
```

| 规则 | 说明 |
|------|------|
| 可选注入 | App builder 或 DI Container 持有 |
| 默认不自动 load/save | 不参与 UI 热路径 |
| 缺文件 | DefaultTheme（#48） |
| 单线程 | GUI 主线程（#88） |

> **实现注记**：Settings 类型已就绪；`App::settings(path)` 已提供 opt-in load 集成，`run()` 进入 GUI/CLI 模式前加载一次并注册 `SettingsService` 到 App DI；默认仍不自动 load/save，运行中保存由业务显式调用。

---

## Key 约定

> **theme_mode 仅影响启动**（#74）；运行中跟 OS 须 App `.follow_system_theme(true)`（#125），与 Settings 写入无关。

| Key | 值示例 | 解析 |
|-----|--------|------|
| `theme_mode` | `"light"` / `"dark"` / `"system"` | 启动 Theme；`"system"` 读 OS 初始模式（#74）；运行中跟 OS 须 App `.follow_system_theme(true)`（#125） |
| `brand_primary` | `"#1677ff"` | `with_brand_primary` |
| 自定义 | 任意 string | 业务自行解析 |

Theme 相关 key 变更后应触发 palette-only invalidate（#9），不强制 layout。

---

## 约束

| 约束 | 原因 |
|------|------|
| 不参与 UI 热路径 | data 域职责边界（AGENTS.md） |
| 不自动 load/save | 避免隐式 IO；测试可控（#64） |
| v1 值均 string | 简单 KV；复杂结构由 App 序列化进 string |
| 只用 core::Error | 跨域错误统一 |

---

## 与 DI 的关系

SettingsService 典型注册：

```rust
App::new()
    .singleton(SettingsService::new())
    // run 前: resolve_mut::<SettingsService>().load("settings.json")?
```

Container 见 [application · CLI 与 DI](application.md#cli-与-di)。

---

## data 域边界

```text
data/
└── settings/
    ├── mod.rs
    └── settings.rs      SettingsService

允许依赖: core
禁止依赖: ui, app, draw, native
```

配置读取应在 app 启动阶段完成；运行中写盘由业务在合适时机（如退出、设置页确认）触发 `save()`。**不**参与主循环热路径、不触发 layout/render（#105）。

---

## 源码模块

```text
data/
└── settings/
    ├── mod.rs
    └── settings.rs      SettingsService（扁平 JSON KV）
```

详见 [roadmap · 源码目录详表](../roadmap.md#源码目录详表)。
