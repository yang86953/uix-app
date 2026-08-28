//! D3D11 swapchain 创建、image 身份、窄提交与 HRESULT 分类边界。

// 引入原生窗口句柄的无类型表示。
use std::ffi::c_void;

// 引入统一错误、结果与最终呈现 damage。
use crate::core::{Errc, Error, PresentDamage, Result};
// 引入跨后端共享的 present image 与保留性证明。
use crate::platform::presentation::{PresentCoherency, PresentImage, PresentTestResult};
// 引入已经通过 Surface capability 与范围门禁的呈现输入。
use crate::platform::presentation::rhi::ValidatedRhiPresent;
// HRESULT 分类归 D3D11/D3D12 共享的 DXGI 组件唯一持有。
use crate::native::presentation::graphics::dxgi::d3d_hresult_code;
// 引入 Windows COM 接口转换能力。
use ::windows::core::Interface;
// 引入 Windows 基础状态、矩形与窗口句柄。
use ::windows::Win32::Foundation::{
    DXGI_STATUS_MODE_CHANGE_IN_PROGRESS, DXGI_STATUS_OCCLUDED, FALSE, HWND, RECT, TRUE,
};
// 引入 D3D11 device 接口，交换链只借用它完成创建。
use ::windows::Win32::Graphics::Direct3D11::ID3D11Device;
// 引入 swapchain descriptor 使用的共享 DXGI 格式。
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_DESC,
    DXGI_MODE_SCALING_UNSPECIFIED, DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED, DXGI_RATIONAL,
    DXGI_SAMPLE_DESC,
};
// 引入 legacy 与 flip-model swapchain 的 DXGI 接口和值。
use ::windows::Win32::Graphics::Dxgi::{
    DXGI_PRESENT, DXGI_PRESENT_PARAMETERS, DXGI_PRESENT_TEST, DXGI_SCALING_STRETCH,
    DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT,
    DXGI_SWAP_EFFECT_DISCARD, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
    IDXGIAdapter, IDXGIDevice, IDXGIFactory, IDXGIFactory2, IDXGIOutput, IDXGISwapChain,
    IDXGISwapChain3,
};

// 集中保存实际 D3D11 swapchain 与上层 present 能力之间的冻结事实。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct D3d11SwapChainContract {
    // 保存实际创建的交换链缓冲数量。
    pub(crate) buffer_count: u32,
    // 保存 DXGI 交换效果，供 descriptor 与一致性测试共同消费。
    pub(crate) swap_effect: DXGI_SWAP_EFFECT,
    // 保存 GraphicsContext 对外声明的跨帧保留性证明。
    pub(crate) present_coherency: PresentCoherency,
}

// 返回具备真实 image 身份的 flip-model 交换链事实。
pub(crate) const fn tracked_swap_chain_contract() -> D3d11SwapChainContract {
    // SwapChain3 + FLIP_SEQUENTIAL 证明双 image 历史和窄提交能力。
    D3d11SwapChainContract {
        // 使用 flip model 的最小双缓冲数量。
        buffer_count: 2,
        // 使用可保留每个 back buffer 内容的交换效果。
        swap_effect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        // 上层必须按真实 image index 修复错过的历史 damage。
        present_coherency: PresentCoherency::TrackedSwapchain,
    }
}

// 返回不具备 image 身份时的 legacy 全帧交换链事实。
pub(crate) const fn legacy_swap_chain_contract() -> D3d11SwapChainContract {
    // DISCARD 不证明 backbuffer 内容保留，因此只能完整提交。
    D3d11SwapChainContract {
        // 保留现有双缓冲创建参数。
        buffer_count: 2,
        // 保留已验证的 legacy bitblt DISCARD 交换效果。
        swap_effect: DXGI_SWAP_EFFECT_DISCARD,
        // DISCARD 无法为上层提供可证明的窄 present coherency。
        present_coherency: PresentCoherency::FullOnly,
    }
}

// 表示构造期已经冻结的 D3D11 swapchain 实例形态。
pub(crate) enum D3d11SwapChain {
    // 保存同时具备 Present1 和真实 back-buffer index 的生产主路径。
    Tracked(IDXGISwapChain3),
    // 保存创建期能力不足时的 DISCARD 全帧回退。
    Legacy(IDXGISwapChain),
}

// 为冻结 swapchain 形态提供不泄漏 DXGI 接口的窄生命周期操作。
impl D3d11SwapChain {
    // 返回本实例在创建期冻结的能力事实。
    pub(crate) const fn contract(&self) -> D3d11SwapChainContract {
        // 交换链形态一旦构造就不在运行期漂移。
        match self {
            // SwapChain3 主路径使用跟踪式双 image 契约。
            Self::Tracked(_) => tracked_swap_chain_contract(),
            // legacy 路径保持完整提交契约。
            Self::Legacy(_) => legacy_swap_chain_contract(),
        }
    }

    // 返回本帧当前可写 back buffer 的稳定身份。
    pub(crate) fn present_image(&self) -> Option<PresentImage> {
        // 只有 SwapChain3 能提供可信的当前 image index。
        match self {
            // 从原生交换链读取当前 back buffer index。
            Self::Tracked(swap_chain) => Some(PresentImage::new(
                // SAFETY: swapchain 由 owner-thread context 唯一持有。
                unsafe { swap_chain.GetCurrentBackBufferIndex() as usize },
                // image 总数来自同一冻结创建契约。
                tracked_swap_chain_contract().buffer_count as usize,
            )),
            // legacy DISCARD 不提供 per-image 历史证明。
            Self::Legacy(_) => None,
        }
    }

    // 获取当前 D3D11 可写 back buffer。
    ///
    /// # Safety
    /// 调用者必须保证 index 小于当前 swapchain 缓冲数，且 swapchain 在调用期间存活（本实例持有）。
    pub(crate) unsafe fn get_buffer<T>(&self, index: u32) -> ::windows::core::Result<T>
    where
        // 目标接口必须满足 Windows COM 类型约束。
        T: Interface,
    {
        // 两种形态都通过共同 IDXGISwapChain 基接口读取 buffer。
        match self {
            // flip-model 仍由继承的 GetBuffer 提供当前 D3D11 buffer 0。
            // SAFETY: swapchain 由本实例持有且存活；index 由调用方限制在当前缓冲数内。
            Self::Tracked(swap_chain) => unsafe { swap_chain.GetBuffer(index) },
            // legacy 路径沿用原生 GetBuffer。
            // SAFETY: swapchain 由本实例持有且存活；index 由调用方限制在当前缓冲数内。
            Self::Legacy(swap_chain) => unsafe { swap_chain.GetBuffer(index) },
        }
    }

    // 在释放所有 back-buffer 引用后重建当前交换链尺寸。
    ///
    /// # Safety
    /// 调用者必须保证所有 back-buffer 引用（RTV/视图）已释放且 GPU 不再使用旧缓冲，尺寸为正。
    pub(crate) unsafe fn resize_buffers(
        &self,
        width: u32,
        height: u32,
    ) -> ::windows::core::Result<()> {
        // 两种形态都保留构造期的 buffer count 与 flags。
        match self {
            // flip-model 使用继承的 ResizeBuffers，并由 surface 代际隔离旧历史。
            // SAFETY: 调用方已释放 back-buffer 引用并等待 GPU；尺寸已验证为正；0 表示保持原缓冲数。
            Self::Tracked(swap_chain) => unsafe {
                swap_chain.ResizeBuffers(
                    0,
                    width,
                    height,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
            },
            // legacy 路径保持相同 resize 参数。
            // SAFETY: 调用方已释放 back-buffer 引用并等待 GPU；尺寸已验证为正；0 表示保持原缓冲数。
            Self::Legacy(swap_chain) => unsafe {
                swap_chain.ResizeBuffers(
                    0,
                    width,
                    height,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
            },
        }
    }

    // 提交本帧，并仅在跟踪式主路径消费经过验证的 dirty rect。
    pub(crate) fn present(&self, present: &ValidatedRhiPresent) -> Result<()> {
        // 按冻结形态选择 Present1 或 legacy Present。
        match self {
            // 主路径必须让最终绘制与 compositor 消费同一 damage 集合。
            Self::Tracked(swap_chain) => {
                // 只把共享门禁已发布的矩形机械投影为 DXGI RECT。
                let mut dirty_rects = native_dirty_rects(present.damage());
                // 构造只在本次调用期间借用 dirty rect 数组的参数。
                let parameters = DXGI_PRESENT_PARAMETERS {
                    // 零表示应用更新了完整帧。
                    DirtyRectsCount: dirty_rects.len() as u32,
                    // 非空时传入稳定数组首地址，空时传入 null。
                    pDirtyRects: if dirty_rects.is_empty() {
                        // 全帧提交不提供矩形指针。
                        std::ptr::null_mut()
                    } else {
                        // 数组在 Present1 返回前保持存活且不会扩容。
                        dirty_rects.as_mut_ptr()
                    },
                    // 本任务不把 scroll 语义重复编码到 native adapter。
                    pScrollRect: std::ptr::null_mut(),
                    // 本任务不提交 compositor scroll offset。
                    pScrollOffset: std::ptr::null_mut(),
                };
                // 将 Present1 HRESULT 交给统一 typed error 分类。
                map_dxgi_present_result(unsafe {
                    // SAFETY: 参数内指针在同步 Present1 调用期间保持有效。
                    swap_chain.Present1(1, DXGI_PRESENT(0), &parameters)
                })
            }
            // FullOnly 回退路径只会收到共享门禁发布的完整 damage。
            Self::Legacy(swap_chain) => map_dxgi_present_result(unsafe {
                // SAFETY: swapchain 只在其 owner UI thread 使用。
                swap_chain.Present(1, DXGI_PRESENT(0))
            }),
        }
    }

    // 不提交帧数据地探测遮挡状态。
    pub(crate) fn test_present(&self) -> Result<PresentTestResult> {
        // 两种交换链都从共同 Present(TEST) 状态边界探测。
        let result = match self {
            // flip-model 使用继承的 Present 进行无数据探测。
            // SAFETY: swapchain 存活且只在其 owner UI thread 上调用；TEST 标志不提交帧数据。
            Self::Tracked(swap_chain) => unsafe { swap_chain.Present(0, DXGI_PRESENT_TEST) },
            // legacy 路径维持既有探测。
            // SAFETY: swapchain 存活且只在其 owner UI thread 上调用；TEST 标志不提交帧数据。
            Self::Legacy(swap_chain) => unsafe { swap_chain.Present(0, DXGI_PRESENT_TEST) },
        };
        // 统一保留 Occluded 与 typed failure。
        map_dxgi_present_test_result(result)
    }
}

// 创建具备 SwapChain3 image 身份的 flip-model 主路径，失败时由调用方回退。
fn create_tracked_swap_chain(
    device: &ID3D11Device,
    adapter: &IDXGIAdapter,
    hwnd: *mut c_void,
    width: i32,
    height: i32,
) -> Result<D3d11SwapChain> {
    // 获取支持 CreateSwapChainForHwnd 的 DXGI 1.2 factory。
    // SAFETY: adapter 存活；GetParent 返回的接口由 windows crate 类型接管。
    let factory: IDXGIFactory2 = unsafe { adapter.GetParent() }
        .map_err(|error| d3d_error("IDXGIAdapter::GetParent<IDXGIFactory2>", error))?;
    // 构造 FLIP_SEQUENTIAL 双缓冲 descriptor。
    let descriptor = tracked_swap_chain_desc(width, height);
    // 创建 HWND flip-model swapchain。
    // SAFETY: device/adapter 存活；hwnd 为有效非空窗口句柄；descriptor 为栈上完整初始化的描述；输出参数传 None。
    let swap_chain = unsafe {
        factory.CreateSwapChainForHwnd(device, HWND(hwnd), &descriptor, None, None::<&IDXGIOutput>)
    }
    .map_err(|error| d3d_error("IDXGIFactory2::CreateSwapChainForHwnd", error))?;
    // 只有 SwapChain3 能把真实 image index 提供给上层 damage history。
    let swap_chain: IDXGISwapChain3 = swap_chain
        .cast()
        .map_err(|error| d3d_error("IDXGISwapChain1::cast<IDXGISwapChain3>", error))?;
    // 冻结为具备 tracked coherency 的主路径实例。
    Ok(D3d11SwapChain::Tracked(swap_chain))
}

// 创建保持旧行为的 DISCARD 全帧交换链。
fn create_legacy_swap_chain(
    device: &ID3D11Device,
    adapter: &IDXGIAdapter,
    hwnd: *mut c_void,
    width: i32,
    height: i32,
) -> Result<D3d11SwapChain> {
    // 获取所有受支持 DXGI 版本都具备的基础 factory。
    // SAFETY: adapter 存活；GetParent 返回的接口由 windows crate 类型接管。
    let factory: IDXGIFactory = unsafe { adapter.GetParent() }
        .map_err(|error| d3d_error("IDXGIAdapter::GetParent<IDXGIFactory>", error))?;
    // 构造已经验证的 bitblt DISCARD descriptor。
    let descriptor = legacy_swap_chain_desc(hwnd, width, height);
    // 接收 legacy CreateSwapChain 返回值。
    let mut swap_chain = None;
    // 执行基础 factory 创建并保留 HRESULT 分类。
    // SAFETY: factory 存活；descriptor 为栈上完整初始化的描述；输出指针指向栈上 Option。
    map_dxgi_operation_result("IDXGIFactory::CreateSwapChain", unsafe {
        factory.CreateSwapChain(device, &descriptor, &mut swap_chain)
    })?;
    // 拒绝成功 HRESULT 却没有接口对象的异常状态。
    let swap_chain = swap_chain.ok_or_else(|| {
        // 使用稳定平台错误供创建回退诊断。
        Error::new(
            Errc::PlatformError,
            "D3d11Context: IDXGIFactory::CreateSwapChain returned no swapchain",
        )
    })?;
    // 冻结为 FullOnly legacy 实例。
    Ok(D3d11SwapChain::Legacy(swap_chain))
}

// 在一个 device 创建边界内选择主路径或 legacy 回退。
pub(crate) fn create_swap_chain(
    device: &ID3D11Device,
    hwnd: *mut c_void,
    width: i32,
    height: i32,
) -> Result<D3d11SwapChain> {
    // 从 D3D11 device 取得其实际 DXGI adapter。
    let dxgi_device: IDXGIDevice = device
        .cast()
        .map_err(|error| d3d_error("ID3D11Device::cast<IDXGIDevice>", error))?;
    // 读取与 device 同源的 adapter，禁止另选显卡创建 swapchain。
    // SAFETY: dxgi_device 存活；GetAdapter 返回的接口由 windows crate 类型接管。
    let adapter = unsafe { dxgi_device.GetAdapter() }
        .map_err(|error| d3d_error("IDXGIDevice::GetAdapter", error))?;
    // 优先创建已批准的 FLIP_SEQUENTIAL + SwapChain3 主路径。
    match create_tracked_swap_chain(device, &adapter, hwnd, width, height) {
        // 主路径成功后能力立即冻结。
        Ok(swap_chain) => Ok(swap_chain),
        // 任何接口或创建缺口只在构造边界回退一次。
        Err(error) => {
            // 记录主路径失败原因，保留可诊断兼容性事实。
            tracing::warn!(
                "D3d11Context: tracked flip swapchain unavailable; falling back to legacy DISCARD: {}",
                error.what()
            );
            // 使用同一 device 和 adapter 创建 FullOnly 回退。
            create_legacy_swap_chain(device, &adapter, hwnd, width, height)
        }
    }
}

// 构造 flip-model HWND swapchain descriptor。
pub(crate) fn tracked_swap_chain_desc(width: i32, height: i32) -> DXGI_SWAP_CHAIN_DESC1 {
    // 从唯一主路径事实读取创建参数。
    let contract = tracked_swap_chain_contract();
    // 返回 DXGI 1.2 HWND descriptor。
    DXGI_SWAP_CHAIN_DESC1 {
        // 使用实际 drawable 宽度。
        Width: width.max(1) as u32,
        // 使用实际 drawable 高度。
        Height: height.max(1) as u32,
        // 保持 UIX 的 premultiplied BGRA8 surface 格式。
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        // 当前窗口交换链不启用立体呈现。
        Stereo: FALSE,
        // flip model 禁止 swapchain MSAA，固定单采样。
        SampleDesc: DXGI_SAMPLE_DESC {
            // 单采样满足 flip-model 约束。
            Count: 1,
            // 单采样不使用质量等级。
            Quality: 0,
        },
        // back buffer 只作为最终 render target。
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        // 缓冲数量来自跟踪式契约。
        BufferCount: contract.buffer_count,
        // HWND surface 随客户区缩放。
        Scaling: DXGI_SCALING_STRETCH,
        // 交换效果与 per-image preservation 证明同源。
        SwapEffect: contract.swap_effect,
        // 普通 HWND swapchain 忽略 alpha mode。
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        // 当前契约不启用 tearing 或其它可选 flags。
        Flags: 0,
    }
}

// 构造 legacy bitblt swapchain descriptor。
pub(crate) fn legacy_swap_chain_desc(
    hwnd: *mut c_void,
    width: i32,
    height: i32,
) -> DXGI_SWAP_CHAIN_DESC {
    // 从唯一回退事实读取创建参数。
    let contract = legacy_swap_chain_contract();
    // 返回原有 DXGI descriptor。
    DXGI_SWAP_CHAIN_DESC {
        // 保存窗口模式的显示信息。
        BufferDesc: DXGI_MODE_DESC {
            // 使用实际 drawable 宽度。
            Width: width.max(1) as u32,
            // 使用实际 drawable 高度。
            Height: height.max(1) as u32,
            // 沿用稳定的 60/1 nominal refresh 描述。
            RefreshRate: DXGI_RATIONAL {
                // 刷新率分子。
                Numerator: 60,
                // 刷新率分母。
                Denominator: 1,
            },
            // 保持 BGRA8 surface 格式。
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            // 不指定扫描线顺序。
            ScanlineOrdering: DXGI_MODE_SCANLINE_ORDER_UNSPECIFIED,
            // 不指定显示模式缩放。
            Scaling: DXGI_MODE_SCALING_UNSPECIFIED,
        },
        // legacy 路径同样固定单采样。
        SampleDesc: DXGI_SAMPLE_DESC {
            // 使用一个 sample。
            Count: 1,
            // 不使用质量等级。
            Quality: 0,
        },
        // back buffer 只作为 render target。
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        // 缓冲数量与回退契约保持同步。
        BufferCount: contract.buffer_count,
        // 绑定调用方 HWND。
        OutputWindow: HWND(hwnd),
        // 使用窗口模式。
        Windowed: TRUE,
        // 交换效果与 FullOnly 事实保持同步。
        SwapEffect: contract.swap_effect,
        // 保持无额外 flags。
        Flags: 0,
    }
}

// 把共享门禁已经规范化的 damage 机械转换为 Present1 RECT 数组。
fn native_dirty_rects(damage: &PresentDamage) -> Vec<RECT> {
    // Full damage 使用空数组编码为 DXGI 完整提交。
    let PresentDamage::Partial(rects) = damage else {
        // 零个 dirty rect 是 Present1 的完整帧信号。
        return Vec::new();
    };
    // 只做元组到 Win32 RECT 的同值投影，不再解释范围规则。
    rects
        // 逐个借用共享物理矩形。
        .iter()
        // 映射为 DXGI left/top/right/bottom 表示。
        .map(|&(x, y, width, height)| RECT {
            // 横向起点保持共享左上原点。
            left: x,
            // 纵向起点保持共享左上原点。
            top: y,
            // 右边界由已验证的正宽度无损生成。
            right: x + width,
            // 下边界由已验证的正高度无损生成。
            bottom: y + height,
        })
        // 固定为同步 Present1 调用期间存活的连续数组。
        .collect()
}

// 将 Windows COM error 转为统一 UIX Error。
pub(super) fn d3d_error(operation: &str, err: ::windows::core::Error) -> Error {
    // 保留操作名、原生消息和 typed error code。
    Error::new(
        // 从 HRESULT 选择恢复分类。
        d3d_hresult_code(err.code()),
        // 保存稳定的 D3D11 context 诊断前缀。
        format!("D3d11Context: {operation} failed: {err}"),
    )
}

// 映射最终 Present 或 Present1 的返回状态。
pub(crate) fn map_dxgi_present_result(result: ::windows::core::HRESULT) -> Result<()> {
    // DXGI 的 occluded 状态是成功码形态，必须先于 is_err 单独识别。
    if result == DXGI_STATUS_OCCLUDED {
        // 返回可恢复遮挡错误，禁止提交 damage history。
        return Err(Error::new(
            Errc::GraphicsOccluded,
            format!("D3d11Context: swapchain present reported occlusion: {result:?}"),
        ));
    }
    // 模式切换中的成功状态未提交当前帧，按 SurfaceLost 执行一次受控重建。
    if result == DXGI_STATUS_MODE_CHANGE_IN_PROGRESS {
        return Err(Error::new(
            Errc::GraphicsSurfaceLost,
            format!("D3d11Context: swapchain present reported mode change: {result:?}"),
        ));
    }
    // 其它返回值进入共同 HRESULT 分类。
    map_dxgi_operation_result("swapchain present", result)
}

// 映射无帧数据的 Present(TEST) 结果。
pub(crate) fn map_dxgi_present_test_result(
    result: ::windows::core::HRESULT,
) -> Result<PresentTestResult> {
    // 遮挡仍存在时返回显式 probe 状态，而不是错误。
    if result == DXGI_STATUS_OCCLUDED {
        // 调度器据此维持 idle。
        return Ok(PresentTestResult::Occluded);
    }
    // 其它失败保持 typed error。
    map_dxgi_operation_result("IDXGISwapChain::Present(DXGI_PRESENT_TEST)", result)?;
    // 成功表示 surface 已恢复可呈现。
    Ok(PresentTestResult::Presentable)
}

// 映射 ResizeBuffers 的原生结果。
pub(crate) fn map_dxgi_resize_result(result: ::windows::core::HRESULT) -> Result<()> {
    // 复用共同 HRESULT 分类。
    map_dxgi_operation_result("IDXGISwapChain::ResizeBuffers", result)
}

// 把 D3D11 设备移除查询统一纳入已有 HRESULT 分类边界。
pub(crate) fn map_dxgi_device_removed_reason(result: ::windows::core::HRESULT) -> Result<()> {
    // 健康设备返回 S_OK，已移除或重置设备返回 GraphicsDeviceLost。
    map_dxgi_operation_result("ID3D11Device::GetDeviceRemovedReason", result)
}

// 映射普通 DXGI 操作的 HRESULT。
fn map_dxgi_operation_result(operation: &str, result: ::windows::core::HRESULT) -> Result<()> {
    // 只有失败 HRESULT 进入错误通道。
    if result.is_err() {
        // 保留统一错误分类和原始十六进制值。
        return Err(Error::new(
            d3d_hresult_code(result),
            format!("D3d11Context: {operation} failed: {result:?}"),
        ));
    }
    // 成功或非错误状态完成操作。
    Ok(())
}
