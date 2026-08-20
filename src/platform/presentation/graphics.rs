//! GPU adapter 的公开描述值。

/// 可显式枚举的图形 API。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicsBackend {
    /// 启用 `d3d11` feature 时可显式选择 Direct3D 11。
    #[cfg(feature = "d3d11")]
    Direct3D11,
    /// 启用 `vulkan` feature 时可显式选择 Vulkan。
    #[cfg(feature = "vulkan")]
    Vulkan,
    /// 启用 `d3d12` feature 时可显式选择 Direct3D 12。
    #[cfg(feature = "d3d12")]
    Direct3D12,
    /// 启用 `metal` feature 时可显式选择 Metal。
    #[cfg(feature = "metal")]
    Metal,
    /// 启用 `opengles` feature 时可显式选择 OpenGL ES。
    #[cfg(feature = "opengles")]
    OpenGlEs,
}

impl GraphicsBackend {
    /// 将公开具体 API 转换为 crate-private registry 身份。
    pub(crate) const fn into_native(self) -> crate::native::present::GraphicsApi {
        match self {
            // 公开 D3D11 变体只在对应实现参与构建时存在。
            #[cfg(feature = "d3d11")]
            Self::Direct3D11 => crate::native::present::GraphicsApi::D3d11,
            // 公开 Vulkan 变体只在对应实现参与构建时存在。
            #[cfg(feature = "vulkan")]
            Self::Vulkan => crate::native::present::GraphicsApi::Vulkan,
            // 公开 D3D12 变体只在对应实现参与构建时存在。
            #[cfg(feature = "d3d12")]
            Self::Direct3D12 => crate::native::present::GraphicsApi::D3d12,
            // 公开 Metal 变体只在对应实现参与构建时存在。
            #[cfg(feature = "metal")]
            Self::Metal => crate::native::present::GraphicsApi::Metal,
            // 公开 OpenGL ES 变体只在对应实现参与构建时存在。
            #[cfg(feature = "opengles")]
            Self::OpenGlEs => crate::native::present::GraphicsApi::OpenGlEs,
        }
    }
}

/// GPU 的物理实现类别。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuDeviceType {
    /// 与系统内存共享资源的集成式 GPU。
    Integrated,
    /// 拥有独立显存的离散式 GPU。
    Discrete,
    /// 由虚拟化环境提供的 GPU。
    Virtual,
    /// 通过 CPU 执行图形工作的软件设备。
    Software,
    /// 平台无法可靠归类的设备。
    Unknown,
}

/// 一次枚举返回的 owned adapter 描述。
///
/// 该值不是稳定 ID，也不持有原生 adapter 或 driver 资源。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuAdapterInfo {
    backend: GraphicsBackend,
    device_type: GpuDeviceType,
    name: Option<String>,
    vendor_id: Option<u32>,
    device_id: Option<u32>,
    driver: Option<String>,
}

impl GpuAdapterInfo {
    // 目前只有 Windows D3D11 的 DXGI 枚举器构造 adapter 快照。
    #[cfg(all(windows, feature = "d3d11"))]
    pub(crate) fn new(
        backend: GraphicsBackend,
        device_type: GpuDeviceType,
        name: Option<String>,
        vendor_id: Option<u32>,
        device_id: Option<u32>,
        driver: Option<String>,
    ) -> Self {
        Self {
            backend,
            device_type,
            name,
            vendor_id,
            device_id,
            driver,
        }
    }

    /// 返回枚举该 adapter 时使用的图形 API。
    pub fn backend(&self) -> GraphicsBackend {
        self.backend
    }

    /// 返回平台报告的物理设备类别。
    pub fn device_type(&self) -> GpuDeviceType {
        self.device_type
    }

    /// 返回平台提供的 adapter 显示名称。
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// 返回平台提供的 PCI 厂商标识。
    pub fn vendor_id(&self) -> Option<u32> {
        self.vendor_id
    }

    /// 返回平台提供的 PCI 设备标识。
    pub fn device_id(&self) -> Option<u32> {
        self.device_id
    }

    /// 返回平台提供的驱动描述。
    pub fn driver(&self) -> Option<&str> {
        self.driver.as_deref()
    }
}
