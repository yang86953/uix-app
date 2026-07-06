# 基础设施系统

> 错误、几何、日志、诊断。最底层，无平台依赖，Fail Fast。

**几何**：Point / Size / Rect / EdgeInsets 全域复用（布局、命中、脏区、present damage），禁止重复定义（#38、#56、#70）。

**错误**：结构化 Error + Result；生产禁止 unwrap/expect；平台错误统一映射。

**日志 / 诊断**：级别 + Sink；帧指标与启动失败走日志；诊断用于 IO/网络，UI 热路径保持轻量。
