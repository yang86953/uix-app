# 数据系统

> 配置持久化；不参与 UI 热路径。

**SettingsService**：扁平字符串 KV，最小 JSON 持久化，脏标记后写入。load/save/get/set/remove。

App **可选**注入（#64）；**默认不**自动 load/save。持久化 theme_mode、brand 等 key，App 解析为 Theme；缺文件用 DefaultTheme。v1 值均为字符串；单线程 GUI。
