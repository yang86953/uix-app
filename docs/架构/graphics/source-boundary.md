# 图形源码边界

当前生产依赖顺序只有一条：

```text
UI → Drawing Engine → platform 通用 RHI → native 原生 adapter
src/ui、src/app → src/draw → src/platform/presentation/rhi
                            → src/native/presentation/graphics/{vulkan,d3d11,opengl}
```

## 物理责任

- 共享上层：`src/app`、`src/ui`、`src/draw` 在所有桌面目标编译同一套生产源码。UI 只建立界面语义；Drawing Engine 唯一定义场景、绘制顺序、FramePlan、11 种 `PipelineKind` 的 canonical spec 与恢复策略。
- 中立契约层：`src/platform/presentation/rhi` 唯一定义资源、pipeline ABI、Device/Surface、提交事务、坐标规则与 `RhiSurfaceLifecycle`。Drawing 的图形下行依赖只从这个入口取得中立协议和 opaque recipe owner；字体发现另经非图形 `platform::services::FontSystemInfo` 窄协议。
- 原生 adapter 层：API 调用、原生句柄、shader 映射、swapchain/context 和呈现实现分别只在 `src/native/presentation/graphics/vulkan`、`d3d11`、`opengl`。`src/native/factory` 与各 registry 只选择、创建和注入 adapter，不执行原生图形命令。

## 已锁定事实

- 三平台生产 registry 都以 Vulkan `priority: 100` 为第一候选；D3D11/OpenGL 兼容候选优先级更低。
- GPU recipe 直接构造 GPU-only backend，不能进入 CPU PixelUpload；PixelUpload 是独立 recipe。
- 三个 API 的 parity harness 都直接消费 Drawing 的同一份 11-pipeline canonical scenes、samples 和 tolerance，没有私有规范副本。
- 三个 adapter 都消费 platform RHI 的同一 `RhiSurfaceLifecycle`，不拥有私有 generation/recreate 状态机。
- 公开 platform GPU adapter 枚举只把中立选择值交给 native factory；DXGI 调用位于 D3D11 adapter。

## 禁止依赖

- `src/app`、`src/ui`、`src/draw` 禁止 OS target `cfg` 和 Vulkan/D3D11/DXGI/OpenGL/EGL/WGL 专名或直接 API crate。
- `src/ui`、`src/draw` 禁止 `crate::native`；Drawing 不得引用具体 adapter。
- API crate、原生入口和 `GraphicsDevice`/`GraphicsSurface` 实现不得离开对应 native adapter；factory/registry 不得穿透到 adapter 的 context、pipeline 或 raster 内部。

## 当前风险与非图形旧债

- 真实 Windows D3D11 尚未执行；本阶段只完成源码门禁及交叉编译检查，不能替代 Windows 驱动、窗口、resize 与呈现验收。
- `src/app` 仍通过精确 allowlist 使用 legacy `crate::native` 的 `Platform`、窗口/事件/输入协议、平台创建入口和可选 agent transport。它们没有 OS/API 条件分支，不影响上层一套图形源码，但其 platform contract 物理迁移尚未完成。
