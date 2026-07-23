//! 可跨线程保存的系统硬件描述值。

use std::num::NonZeroUsize;

use crate::core::Rect;

/// 操作系统描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsInfo {
    name: String,
    version: Option<String>,
    build: Option<String>,
}

impl OsInfo {
    pub(crate) fn new(name: String, version: Option<String>, build: Option<String>) -> Self {
        Self {
            name,
            version,
            build,
        }
    }

    /// 非空的系统名称。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 系统版本；平台未提供时为 `None`。
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// 系统 build 标识；平台未提供时为 `None`。
    pub fn build(&self) -> Option<&str> {
        self.build.as_deref()
    }
}

/// CPU 描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuInfo {
    architecture: String,
    logical_cores: NonZeroUsize,
    vendor: Option<String>,
    model: Option<String>,
}

impl CpuInfo {
    pub(crate) fn new(
        architecture: String,
        logical_cores: NonZeroUsize,
        vendor: Option<String>,
        model: Option<String>,
    ) -> Self {
        Self {
            architecture,
            logical_cores,
            vendor,
            model,
        }
    }

    /// 非空的 CPU 架构名称。
    pub fn architecture(&self) -> &str {
        &self.architecture
    }

    /// 当前进程可用的逻辑处理器数量。
    pub fn logical_cores(&self) -> NonZeroUsize {
        self.logical_cores
    }

    /// CPU 厂商；平台未提供时为 `None`。
    pub fn vendor(&self) -> Option<&str> {
        self.vendor.as_deref()
    }

    /// CPU 型号；平台未提供时为 `None`。
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }
}

/// 系统物理内存描述。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryInfo {
    total_bytes: u64,
    available_bytes: u64,
}

impl MemoryInfo {
    pub(crate) fn new(total_bytes: u64, available_bytes: u64) -> Self {
        Self {
            total_bytes,
            available_bytes,
        }
    }

    /// 系统物理内存总量（字节）。
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// 当前可用物理内存（字节）。
    pub fn available_bytes(&self) -> u64 {
        self.available_bytes
    }
}

/// 单个显示器的即时描述。
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfo {
    bounds: Rect,
    scale: f32,
    is_primary: bool,
    name: Option<String>,
    refresh_rate_millihertz: Option<u32>,
}

impl DisplayInfo {
    pub(crate) fn new(
        bounds: Rect,
        scale: f32,
        is_primary: bool,
        name: Option<String>,
        refresh_rate_millihertz: Option<u32>,
    ) -> Self {
        Self {
            bounds,
            scale,
            is_primary,
            name,
            refresh_rate_millihertz,
        }
    }

    /// 显示器逻辑坐标边界；多显示器布局允许负坐标。
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// 逻辑坐标到物理像素的有限正比例。
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// 是否为系统主显示器。
    pub fn is_primary(&self) -> bool {
        self.is_primary
    }

    /// 系统提供的显示器名称。
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// 刷新率，单位为 millihertz。
    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        self.refresh_rate_millihertz
    }
}
