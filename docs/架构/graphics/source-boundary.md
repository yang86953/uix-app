# 图形源码边界

[← 返回架构索引](../../架构.md)

## 当前状态

| 状态 | 范围 | 已核实结论 |
|---|---|---|
| **已实现** | 上层单一源码 | `src/app`、`src/ui`、`src/draw` 对 `crate::native` / `native::` 的生产引用为 0；UI 与 app 不直连 RHI，Drawing 只消费 platform 通用 RHI/Surface 合同。 |
| **已实现** | platform 组合边界 | `src/platform/composition_root.rs` 是 `src/platform` 中唯一允许依赖 concrete native backend 的文件；中立合同、该组合根与 `src/native` concrete adapter 共同构成 platform 层。 |
| **已实现** | 源码边界 | 上层 OS/API 中立性、唯一 concrete 组合根、12 类 pipeline 的单一规范，以及 Vulkan、D3D11、D3D12、OpenGL、Metal 对共享 Surface 生命周期的消费均已落地；这些物理结构只由架构审查维护，不建立项目测试。 |
| **已验收** | Windows D3D11 公开运行 | `0.0.2` 已在真实 Windows x64 主机完成 Vulkan 首选、D3D11 兼容回退与最终 surface 验收；实现边界与复验方式见 [backend 状态矩阵](backend.md#当前实现状态)，发布提交、环境与制品摘要由私有 Gitea `v0.0.2` Release 持有。 |

当前生产依赖顺序只有一条：

```text
UI → Drawing Engine → platform 通用 RHI/Surface 合同 → native 原生 adapter
src/ui → src/draw → src/platform/presentation/rhi
                              ↓ 动态合同实现
              src/native/presentation/graphics/{vulkan,d3d11,d3d12,opengl,metal}

启动装配：src/app → src/platform/composition_root.rs → src/native/factory 与 concrete backend
```

## 物理责任

- 共享上层：`src/app`、`src/ui`、`src/draw` 在所有桌面目标编译同一套生产源码。UI 只建立界面语义；Drawing Engine 唯一定义场景、绘制顺序、FramePlan、12 种 `PipelineKind` 的 canonical spec 与恢复策略。
- 中立契约层：`src/platform/presentation/rhi` 唯一定义资源、pipeline ABI、Device/Surface、提交事务、坐标规则与 `RhiSurfaceLifecycle`。Drawing 的图形下行依赖只从这个入口取得中立协议和 opaque recipe owner；字体发现另经非图形 `platform::services::FontSystemInfo` 窄协议。中立合同不依赖 `src/native`。
- 唯一组合根：`src/platform/composition_root.rs` 是 `src/platform` 内唯一允许依赖 concrete native backend 的文件，只选择、创建、注入并转移对象所有权，不承载上层或绘制流程。
- 原生 adapter 层：API 调用、原生句柄、shader 映射、swapchain/context 和呈现实现分别只在 `src/native/presentation/graphics/vulkan`、`d3d11`、`d3d12`、`opengl`、`metal`。`src/native/factory` 与各 registry 只登记、选择和创建 adapter，不执行上层行为。

`src/platform` 的中立合同与唯一组合根、`src/native` 的 concrete adapter 与内部装配共同构成 platform 层；OS/API 差异只在该层内选择并经同一合同注入，不泄漏到 `src/app`、`src/ui`、`src/draw`。

## 已实现事实

- 三平台生产 registry 都以 Vulkan `priority: 100` 为第一候选；macOS 显式启用的 Metal 为 `90`，Windows 显式启用的 D3D12 为 `40`，D3D11/OpenGL 兼容候选优先级更低。
- GPU recipe 直接构造 GPU-only backend，不能进入 CPU PixelUpload；PixelUpload 是独立 recipe。
- 五个生产 adapter 都消费 platform RHI 的同一 `RhiSurfaceLifecycle`，不拥有私有 generation/recreate 状态机。
- 公开 platform GPU adapter 枚举只把中立选择值交给 native factory；D3D 共用 DXGI，Vulkan 与 Metal 分别调用各自原生设备枚举。OpenGL ES 没有无上下文的可移植枚举协议，保持 typed `NotImplemented`，不伪造设备。

## 禁止依赖

- `src/app`、`src/ui`、`src/draw` 禁止 OS target `cfg` 和 Vulkan/D3D11/D3D12/DXGI/OpenGL/EGL/WGL/Metal 专名或直接 API crate。
- `src/app`、`src/ui`、`src/draw` 对 `crate::native` 的引用数量必须为 0；UI/app 不得直连 RHI，Drawing 不得引用具体 adapter。
- API crate、原生入口和 `GraphicsDevice`/`GraphicsSurface` 实现不得离开对应 native adapter；factory/registry 不得穿透到 adapter 的 context、pipeline 或 raster 内部。
