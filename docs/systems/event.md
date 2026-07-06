# 事件系统

> 输入 → 系统 / 语义 / 自定义 三层；业务只绑定后两层。

| 层 | 消费方 |
|----|--------|
| 系统（指针、键盘、窗口、剪贴板、IME…） | 组件 dispatch |
| 语义（Click、Change、FileDrop、ContextMenu…） | HandlerTable |
| 自定义（typed payload，#6） | HandlerTable |

SystemEvent 在 app 边界由 UiEvent 转换；逻辑像素（#70）。Click payload：按键、位置、修饰键（#36）。语义可 register_semantic 扩展（#66）。

## 传播 · HandlerTable

Bubble 默认，Capture opt-in（#4、#25）。多 handler 顺序执行（#7）；stop 阻后续 handler 与语义冒泡（#68）；preventDefault（#92）。

HandlerTable 挂 ComponentId（#10）；闭包 `'static` + AppState/Handle（#26）；when（#91）、once（#90）；rebuild 一律重注册（#62）。

剪贴板/IME（#71、#75）：单行、多行、完整 IME。FileDrop（#95）、ContextMenu（#98）→ 浮层。

派发：平台输入 → SystemEvent → 命中/焦点链 → dispatch → Bubble → HandlerTable → 标脏。测试：FakePlatform + 语义断言 + paint 快照（#40）。
