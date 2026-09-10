# painting 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 graphics System 的 `painting` Module，权威持有组件绘制入口、canonical 操作、DisplayList 与帧编码。几何值由 graphics System 从 [geometry](geometry.md) 注入；Module 间不直接持有实例。导出：UI 使用的 `PaintContext` 和 graphics System 编排用的命令序列。

> **当前实现线索**：主要位于 `src/draw/painting/`（paint_context/、recorder/、encoder/）；未来路径重构不改变单一录制管线。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `PaintContext` | struct | 组件生产绘制入口 |
| `PaintOp` / `PaintPass` | enum | canonical 绘制操作与阶段 |
| `DisplayList` | struct | 有序、共享、可重放操作序列 |
| `CommandRecorder` | component | 状态机与操作规范化 |
| `FrameEncoder` / `FrameCommand` | struct/enum | backend-neutral 帧 IR |
| `Canvas2D` | internal interface | canonical backend 执行语义，不是原生 RHI |

## 组件：PaintContext

显式录制 save/restore、clip、transform、opacity、blend 和已解析主题值；UI/Demo 不取得 raw framebuffer、backend 或 native graphics。

## 组件：DisplayList

保持 painter order；seal 后为不可变值，共享内容可 copy-on-write 复用，但优化不得重排目标相关操作。`CommandRecorder` 独占未完成的状态栈，录制结束时 save/restore、clip 与 pass 必须平衡；不完整序列返回 typed error，不能交给 scene 或 backend 猜测修复。

## 组件：FrameEncoder

选择 canonical op、Picture 或受控 CPU segment；不能语义等价执行时返回 typed error。输出保留 UI 绘制语义，不包含 GPU handle、pipeline、barrier 或 swapchain 操作；GPU 资源和 pass 计划由[backend](backend.md)中的通用 GPU Renderer 统一生成。

## 所有权与不变量

- 每次录制由一个窗口的一次 paint 阶段独占；`PaintContext` 和未 seal recorder 不跨线程、不跨帧保存，也不延长组件或窗口生命周期。
- DisplayList 只保存 owned 值、稳定资源句柄和必要 generation，不保存组件、原生对象或 backend 可变引用。
- 坐标、变换、opacity 与路径输入必须有限；无效值在录制边界返回 typed error，不能传播为未定义 backend 行为。
- 编码成功只说明生成了可执行帧输入，不表示 backend 已执行、present 已成功或 damage 可以消费。
