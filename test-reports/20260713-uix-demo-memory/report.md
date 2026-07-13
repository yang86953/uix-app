# uix-demo 内存复测

日期：2026-07-13；平台：Windows；构建：`cargo build --bin uix-demo`（debug）；窗口：1200×800。

## 结论

高内存不是随运行时间单调增长的泄漏，也不是单一图形 API 驱动问题。`FrameRecordingCanvas` 在每个 glyph、圆角和阴影操作后都把整张 1200×800 RGBA scratch（约 3.66 MiB）复制进 `FrameEncoder`；首屏约 400 个 retained full-frame payload 使私有内存稳定在约 1.49 GiB。

修复后每个 CPU segment 只保留 alpha 可见边界的紧凑 tile，并显式记录局部 source 与窗口 destination。25 秒复测中私有内存稳定在约 195 MiB，工作集约 174 MiB；相对修复前分别下降约 86.9% 和 88.1%。

| 25 秒采样 | 修复前 | 修复后 |
|---|---:|---:|
| Working Set | 1463.6 MiB | 173.5 MiB |
| Private Bytes | 1486.8 MiB | 195.4 MiB |
| Virtual Memory | 7101.3 MiB | 5808.6 MiB |
| CPU Time | 24.94 s | 24.58 s |

## 隔离证据

修复前在相同 debug 构建与窗口尺寸下运行 8 秒：D3D11、D3D12、OpenGL ES 的 Private Bytes 分别约为 1489.2、1522.8、1557.8 MiB，排除了单一 backend 的特有分配。修复前后 25 秒曲线均趋于稳定；本次修复针对 retained payload 峰值，不宣称解决 demo 的 16 ms 动画定时器带来的持续单核负载。

## 自动化回归

- `recording_canvas_retains_compact_cpu_segment_tiles`：1200×800 recorder 中 3×2 半透明操作只保留 6 个像素，`src=(0,0,3,2)`、`dst=(701,503,3,2)`，并验证 reference 合成边界。
- `recording_canvas_emits_native_cpu_and_picture_commands_in_painter_order`：相邻 native / CPU / Picture 命令顺序保持不变。

剩余验证：切换多页并运行 15 分钟，确认不同组件组合和长时运行的资源稳定性。
