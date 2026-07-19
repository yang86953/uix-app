# Agent Bridge (开发预览)

双门禁启用：`agent-control` feature + `.enable_agent_control()`。启用后发布本机端点，供同用户进程通过 JSON Lines 协议枚举窗口、读语义树、执行动作。空闲无轮询，敏感值不导出。

`perform` 支持两类动作：

- 语义目标动作：`invoke`、`focus`、`set_value`、`insert_text`、`select`、`toggle`、`increment`、`decrement`、`scroll`。
- 窗口坐标动作：`press_key`、`click_at`、`pointer_move`、`pointer_down`、`pointer_up`。后三者用于真实 hover / pressed 视觉验收；当前只表达左键，调用方必须在 `pointer_down` 后保证发送匹配的 `pointer_up`。

```json
{"type":"perform","window_id":1,"generation":1,"action":{"kind":"pointer_move","x":320,"y":180}}
{"type":"perform","window_id":1,"generation":1,"action":{"kind":"pointer_down","x":320,"y":180}}
{"type":"perform","window_id":1,"generation":1,"action":{"kind":"pointer_up","x":320,"y":180}}
```

详见[架构](../架构.md#大模型控制协议)；缺口 → [`AGENT-R1`](../进度/平台与硬件.md#agent-附能力)。
