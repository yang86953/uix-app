//! GPU adapter 的公开描述值。

/// 可显式枚举的图形 API。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphicsBackend {
    Vulkan,
    Direct3D12,
    Metal,
    OpenGlEs,
}

impl GraphicsBackend {
    pub(crate) const fn into_native(self) -> crate::native::present::GraphicsBackend {
        use crate::native::present::GraphicsBackend as NativeGraphicsBackend;

        match self {
            Self::Vulkan => NativeGraphicsBackend::Vulkan,
            Self::Direct3D12 => NativeGraphicsBackend::D3d12,
            Self::Metal => NativeGraphicsBackend::Metal,
            Self::OpenGlEs => NativeGraphicsBackend::OpenGlEs,
        }
    }
}

/// GPU 的物理实现类别。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuDeviceType {
    Integrated,
    Discrete,
    Virtual,
    Software,
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

    pub fn backend(&self) -> GraphicsBackend {
        self.backend
    }

    pub fn device_type(&self) -> GpuDeviceType {
        self.device_type
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn vendor_id(&self) -> Option<u32> {
        self.vendor_id
    }

    pub fn device_id(&self) -> Option<u32> {
        self.device_id
    }

    pub fn driver(&self) -> Option<&str> {
        self.driver.as_deref()
    }
}
