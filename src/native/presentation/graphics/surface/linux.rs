//! Linux native GPU surface descriptors.

#![cfg(all(unix, not(target_os = "macos")))]
// 在 Rust 2024 下禁止 unsafe 函数体隐式扩大底层操作范围。
#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::c_void;
// 逐窗 surface 与 graphics context 共享同一份原子 logical/drawable 事实。
use std::sync::{Arc, RwLock};

use crate::core::{Errc, Error, Result};

// Wayland logical extent、整数缩放与 drawable extent 的一致快照。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WaylandSurfaceSnapshot {
    // 窗口协议与 UI 使用的逻辑宽度。
    pub(crate) logical_width: i32,
    // 窗口协议与 UI 使用的逻辑高度。
    pub(crate) logical_height: i32,
    // 原生 buffer 使用的物理宽度。
    pub(crate) drawable_width: i32,
    // 原生 buffer 使用的物理高度。
    pub(crate) drawable_height: i32,
    // wl_surface 当前整数 buffer scale。
    pub(crate) scale: i32,
    // 任一 surface 事实变化时递增的代次。
    pub(crate) revision: u64,
    // 客户端窗口在当前模式下需要的物理圆角半径。
    pub(crate) corner_radius: i32,
}

// 锁内只保存不能被拆分发布的 logical extent、scale 与 revision。
#[derive(Debug)]
struct WaylandSurfaceMetricsState {
    // 当前逻辑宽度。
    logical_width: i32,
    // 当前逻辑高度。
    logical_height: i32,
    // 当前整数 buffer scale。
    scale: i32,
    // 当前事实修订号。
    revision: u64,
    // UIX 自绘标题栏是否接管窗口外观。
    client_decorated: bool,
    // 最大化和全屏窗口必须保持方形贴合工作区。
    maximized: bool,
}

// presentation 与 Wayland windowing 共享的窄 surface 元数据 owner。
#[derive(Debug)]
pub(crate) struct WaylandSurfaceMetrics {
    // 单锁保证 graphics 永远读取同一代 logical extent 与 scale。
    state: RwLock<WaylandSurfaceMetricsState>,
}

impl WaylandSurfaceMetrics {
    // 使用已验证的初始逻辑尺寸与 output scale 创建逐窗元数据。
    pub(crate) fn new(logical_width: i32, logical_height: i32, scale: i32) -> Self {
        // 构造路径把无效协议值收敛到可呈现的最小事实。
        let logical_width = logical_width.max(1);
        // 高度采用相同正值约束。
        let logical_height = logical_height.max(1);
        // Wayland core buffer scale 只能是正整数。
        let scale = scale.max(1);
        // 发布初始一致状态。
        Self {
            // 新建锁只包含本窗口的 surface 事实。
            state: RwLock::new(WaylandSurfaceMetricsState {
                // 保存逻辑宽度。
                logical_width,
                // 保存逻辑高度。
                logical_height,
                // 保存整数缩放。
                scale,
                // 初始 surface 使用第零修订。
                revision: 0,
                // 初始仍由 compositor 装饰。
                client_decorated: false,
                // 初始窗口处于还原态。
                maximized: false,
            }),
        }
    }

    // 发布客户端装饰接管事实，供所有图形后端读取同一外观状态。
    pub(crate) fn set_client_decorated(&self, client_decorated: bool) -> Result<()> {
        let mut state = self.state.write().map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "Wayland surface appearance write lock poisoned",
            )
        })?;
        if state.client_decorated != client_decorated {
            state.client_decorated = client_decorated;
            state.revision = state.revision.saturating_add(1);
        }
        Ok(())
    }

    // 发布最大化或全屏状态，使客户端装饰在贴边窗口上关闭圆角。
    pub(crate) fn set_maximized(&self, maximized: bool) -> Result<()> {
        let mut state = self.state.write().map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "Wayland surface mode appearance write lock poisoned",
            )
        })?;
        if state.maximized != maximized {
            state.maximized = maximized;
            state.revision = state.revision.saturating_add(1);
        }
        Ok(())
    }

    // 原子更新逻辑尺寸与 scale，并返回更新后的完整快照。
    pub(crate) fn update(
        &self,
        logical_width: i32,
        logical_height: i32,
        scale: i32,
    ) -> Result<WaylandSurfaceSnapshot> {
        // 拒绝会让窗口与 drawable 失去正 extent 的输入。
        if logical_width <= 0 || logical_height <= 0 || scale <= 0 {
            // 保留完整非法事实供调用方定位协议或生命周期错误。
            return Err(Error::new(
                // 非正 surface 数据属于参数边界错误。
                Errc::InvalidArgument,
                // 错误文本包含三项相关事实。
                format!(
                    "Wayland surface metrics require positive logical extent and scale, got {logical_width}x{logical_height} scale={scale}"
                ),
            ));
        }
        // 一次取得写锁，禁止 logical 与 scale 分开发布。
        let mut state = self.state.write().map_err(|_| {
            // 锁中毒表示共享 surface owner 已损坏。
            Error::new(
                // 使用稳定生命周期错误分类。
                Errc::InvalidState,
                // 指明发生在 Wayland surface 元数据提交边界。
                "Wayland surface metrics write lock poisoned",
            )
        })?;
        // 只有事实变化时才推进修订号。
        if state.logical_width != logical_width
            || state.logical_height != logical_height
            || state.scale != scale
        {
            // 提交新逻辑宽度。
            state.logical_width = logical_width;
            // 提交新逻辑高度。
            state.logical_height = logical_height;
            // 提交新整数缩放。
            state.scale = scale;
            // 推进修订以隔离旧 drawable 事实。
            state.revision = state.revision.saturating_add(1);
        }
        // 从同一写锁 guard 构造完整快照。
        snapshot_from_state(&state)
    }

    // 返回当前同代 logical extent、drawable extent 与 scale。
    pub(crate) fn snapshot(&self) -> Result<WaylandSurfaceSnapshot> {
        // 一次读取锁阻止跨代拼接。
        let state = self.state.read().map_err(|_| {
            // 锁中毒保持 typed 生命周期失败。
            Error::new(
                // 共享 owner 损坏使用 InvalidState。
                Errc::InvalidState,
                // 指明发生在快照读取边界。
                "Wayland surface metrics read lock poisoned",
            )
        })?;
        // 在 guard 生命周期内完成物理尺寸换算。
        snapshot_from_state(&state)
    }
}

impl Default for WaylandSurfaceMetrics {
    // 缺少窗口事实时提供最小的 1x1、scale=1 占位 owner。
    fn default() -> Self {
        // 默认只用于尚未初始化的原生 descriptor。
        Self::new(1, 1, 1)
    }
}

// 从同一锁 guard 计算带溢出检查的物理 drawable 快照。
fn snapshot_from_state(state: &WaylandSurfaceMetricsState) -> Result<WaylandSurfaceSnapshot> {
    // 逻辑宽度乘整数 scale 得到物理宽度。
    let drawable_width = state
        .logical_width
        .checked_mul(state.scale)
        .ok_or_else(|| {
            // 尺寸溢出不得传入 EGL、SHM 或驱动。
            Error::new(
                // 物理 extent 超出 i32 属于资源尺寸不足。
                Errc::InsufficientResources,
                // 保留逻辑宽度和缩放用于诊断。
                format!(
                    "Wayland drawable width overflow: {} * {}",
                    state.logical_width, state.scale
                ),
            )
        })?;
    // 逻辑高度使用相同受检换算。
    let drawable_height = state
        .logical_height
        .checked_mul(state.scale)
        .ok_or_else(|| {
            // 高度溢出也不得形成部分 surface 快照。
            Error::new(
                // 保持同一资源错误分类。
                Errc::InsufficientResources,
                // 保留逻辑高度和缩放用于诊断。
                format!(
                    "Wayland drawable height overflow: {} * {}",
                    state.logical_height, state.scale
                ),
            )
        })?;
    // 返回不可拆分的完整 surface 事实。
    Ok(WaylandSurfaceSnapshot {
        // 复制逻辑宽度。
        logical_width: state.logical_width,
        // 复制逻辑高度。
        logical_height: state.logical_height,
        // 保存受检物理宽度。
        drawable_width,
        // 保存受检物理高度。
        drawable_height,
        // 保存同代整数缩放。
        scale: state.scale,
        // 保存同代修订号。
        revision: state.revision,
        // 统一使用十个逻辑像素，与原生窗口常见圆角尺度一致。
        corner_radius: if state.client_decorated && !state.maximized {
            10_i32.saturating_mul(state.scale)
        } else {
            0
        },
    })
}

#[derive(Clone, Debug)]
pub(crate) struct WaylandSurfaceHandle {
    pub(crate) display: *mut c_void,
    pub(crate) surface: *mut c_void,
    // descriptor 共享窗口持有的逐窗 surface 元数据，而不是复制易过期数值。
    pub(crate) metrics: Arc<WaylandSurfaceMetrics>,
}

impl WaylandSurfaceHandle {
    pub(crate) fn new(
        display: *mut c_void,
        surface: *mut c_void,
        metrics: Arc<WaylandSurfaceMetrics>,
    ) -> Self {
        // 三项共同形成 graphics 构造期间可克隆的原生 descriptor。
        Self {
            // 保存 Wayland display 连接指针。
            display,
            // 保存目标 wl_surface 指针。
            surface,
            // 保存逐窗原子 surface 元数据。
            metrics,
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        !self.display.is_null() && !self.surface.is_null()
    }

    /// 从原生指针按值读取 Wayland surface 描述。
    ///
    /// # Safety
    /// `native_surface` 必须指向调用期间有效、正确对齐且已初始化的 [`WaylandSurfaceHandle`]。
    pub(crate) unsafe fn from_native(native_surface: *mut c_void) -> Result<Self> {
        if native_surface.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "WaylandSurfaceHandle: native surface descriptor is null",
            ));
        }
        // SAFETY：native_surface 由调用方保证来自受信来源，且非空（上文已校验）；
        // 解引用得到的是按值复制的句柄描述，不长期持有原生指针。
        // 共享元数据通过 clone 延长到 graphics owner checked shutdown 结束。
        let handle = unsafe { (&*(native_surface as *const WaylandSurfaceHandle)).clone() };
        if !handle.is_valid() {
            return Err(Error::new(
                Errc::PlatformError,
                "WaylandSurfaceHandle: display or surface pointer is null",
            ));
        }
        Ok(handle)
    }
}

impl Default for WaylandSurfaceHandle {
    // 尚未初始化的窗口 descriptor 保持原生指针为空。
    fn default() -> Self {
        // 创建只供占位的最小元数据 owner。
        Self {
            // 空 display 阻止 descriptor 被 graphics 接纳。
            display: std::ptr::null_mut(),
            // 空 surface 阻止 descriptor 被 graphics 接纳。
            surface: std::ptr::null_mut(),
            // 元数据仍保持结构完整，避免 Option 扩散到窗口 owner。
            metrics: Arc::new(WaylandSurfaceMetrics::default()),
        }
    }
}
