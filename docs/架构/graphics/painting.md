# painting 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 graphics 系统的目标 `painting` 模块，权威持有组件绘制入口、canonical 操作、DisplayList 与帧编码。依赖：[geometry](geometry.md)。导出：UI 使用的 `PaintContext` 和 renderer/scene 使用的命令序列。

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

保持 painter order；共享不可变内容可 copy-on-write 复用，但优化不得重排目标相关操作。

## 组件：FrameEncoder

选择 canonical op、Picture 或受控 CPU segment；不能语义等价执行时返回 typed error。输出保留 UI 绘制语义，不包含 GPU handle、pipeline、barrier 或 swapchain 操作；GPU 资源和 pass 计划由[backend](backend.md)中的通用 GPU Renderer 统一生成。
