//! 平台中立窗口协议 — 窗口创建、属性、生命周期与帧回调。

use crate::core::WindowId;
use crate::core::error::{Error, Result};
use crate::core::geometry::Point;
use crate::platform::presentation::IPresenter;
use crate::platform::windowing::WindowSurfaceRole;
use crate::platform::windowing::event::{FrameRequestToken, PointerActivationId};
use crate::platform::windowing::{WindowCapabilities, WindowResizeEdge};

/// 原生单次帧请求相对当前提交的生效阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFrameRequestPhase {
    /// 当前组装帧提交后回调才可生效；显示节拍 API 使用此阶段。
    AfterPresent,
}

/// 交给窗口后端的一次性帧回调请求。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFrameRequest {
    pub token: FrameRequestToken,
    pub phase: NativeFrameRequestPhase,
}

/// 合成器对窗口能否贡献可见像素的当前认知。
///
/// `Unknown` 表示后端没有精确的逐窗查询，调用方必须保留后端特有的恢复机制。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WindowOcclusionState {
    #[default]
    Unknown,
    Visible,
    Occluded,
}

impl NativeFrameRequest {
    pub const fn after_present(token: FrameRequestToken) -> Self {
        Self {
            token,
            phase: NativeFrameRequestPhase::AfterPresent,
        }
    }
}

/// 平台窗口的逻辑属性与模式操作。
pub trait IWindowProperties {
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    /// 设置正的 logical 客户区尺寸；已有最小/最大约束时必须落在其内。
    fn set_size(&mut self, w: i32, h: i32) -> Result<()>;
    /// 设置正的 logical 客户区最小尺寸，且不得超过已登记的最大尺寸。
    fn set_minimum_size(&mut self, w: i32, h: i32) -> Result<()>;
    /// 设置正的 logical 客户区最大尺寸，且不得小于已登记的最小尺寸。
    fn set_maximum_size(&mut self, w: i32, h: i32) -> Result<()>;
    fn position(&self) -> Point;
    fn set_position(&mut self, x: i32, y: i32) -> Result<()>;
    fn set_resizable(&mut self, resizable: bool) -> Result<()>;
    fn is_maximized(&self) -> bool;
    fn is_minimized(&self) -> bool;
    fn maximize(&mut self) -> Result<()>;
    fn minimize(&mut self) -> Result<()>;
    fn restore(&mut self) -> Result<()>;
    /// 显示或隐藏系统非客户区标题栏，同时保留后端可用的窗口缩放边框。
    fn set_system_title_bar_visible(&mut self, visible: bool) -> Result<()>;
    fn set_borderless(&mut self, borderless: bool) -> Result<()>;
    /// 切换全屏；退出时恢复进入前的位置与窗口样式。
    fn set_fullscreen(&mut self, fullscreen: bool) -> Result<()>;
    fn is_fullscreen(&self) -> bool;
    fn set_always_on_top(&mut self, on: bool) -> Result<()>;
    /// 设置窗口整体不透明度，取值必须为有限的 `0.0..=1.0`。
    fn set_window_opacity(&mut self, opacity: f32) -> Result<()>;
    fn start_text_input(&mut self) -> Result<()>;
    fn stop_text_input(&mut self) -> Result<()>;
    fn enable_file_drop(&mut self, enable: bool) -> Result<()>;
}

/// 暴露平台窗口原生句柄的窄协议。
pub trait INativeHandle {
    fn native_window(&self) -> *mut std::ffi::c_void;
}

/// 创建平台窗口的中立能力协议。
pub trait IWindowManager {
    fn create_window(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<Box<dyn PlatformWindow>, Error>;

    /// 创建带显式 surface role 的窗口；非 Wayland 平台只接受普通 toplevel。
    fn create_window_with_role(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
        role: &WindowSurfaceRole,
    ) -> Result<Box<dyn PlatformWindow>, Error> {
        match role {
            WindowSurfaceRole::Toplevel => self.create_window(title, width, height),
            WindowSurfaceRole::DesktopLayer(_) => Err(Error::new(
                crate::core::Errc::NotImplemented,
                "desktop layer surfaces require a Wayland layer-shell compositor",
            )),
        }
    }
}

/// 平台窗口 — 可见性、标题、层级与呈现器访问。
pub trait PlatformWindow {
    fn window_id(&self) -> WindowId;
    /// 返回当前窗口实例在调用时真实具备的只读可选能力集合。
    fn capabilities(&self) -> WindowCapabilities;
    fn show(&mut self) -> Result<()>;
    fn hide(&mut self) -> Result<()>;
    fn close(&mut self) -> Result<()>;
    /// 请求走平台正常关闭事件，使应用先释放图形与会话资源。
    fn request_close(&mut self) -> Result<()>;
    /// 将当前鼠标手势及其不可解释激活身份移交给窗口管理器。
    fn begin_move_drag(
        // 激活身份只用于关联当前原生 PointerDown，平台可选择忽略。
        &mut self,
        // 未携带身份表示动作并非由可验证的原生指针激活产生。
        pointer_activation: Option<PointerActivationId>,
        // 返回平台请求提交结果，正常的过期授权应安全忽略。
    ) -> Result<()>;
    /// 将当前鼠标手势移交给窗口管理器并从指定边或角调整大小。
    fn begin_resize_drag(
        // 原生后端把公共方向映射为各自的窗口系统协议值。
        &mut self,
        // 保留调用 UI 热区声明的精确方向。
        edge: WindowResizeEdge,
        // 激活身份只用于关联当前原生 PointerDown，平台可选择忽略。
        pointer_activation: Option<PointerActivationId>,
        // 返回平台请求提交结果，正常的过期授权应安全忽略。
    ) -> Result<()>;
    /// 在当前指针位置显示窗口管理器提供的原生系统菜单。
    fn show_system_menu(&mut self) -> Result<()>;
    fn is_visible(&self) -> bool;

    /// 返回窗口系统当前确认的逻辑客户区；默认使用中立属性快照。
    fn client_logical_extent(&self) -> (i32, i32) {
        (self.properties().width(), self.properties().height())
    }

    /// 返回后端可查询到的最新精确合成器可见性。
    ///
    /// 此状态有意区别于 `is_visible`：窗口可以已显示但被其他窗口完全遮挡。
    fn occlusion_state(&self) -> WindowOcclusionState {
        WindowOcclusionState::Unknown
    }

    fn set_title(&mut self, title: &str) -> Result<()>;
    fn center_on_screen(&mut self) -> Result<()>;
    fn raise(&mut self) -> Result<()>;
    fn lower(&mut self) -> Result<()>;
    fn set_window_icon(&mut self, icon_path: &str) -> Result<()>;
    fn flash_window(&mut self) -> Result<()>;
    fn resize_notify(&mut self, width: i32, height: i32) -> Result<()>;
    fn properties(&self) -> &dyn IWindowProperties;
    fn properties_mut(&mut self) -> &mut dyn IWindowProperties;
    fn presenter(&mut self) -> &mut dyn IPresenter;
    fn native_handle(&self) -> &dyn INativeHandle;

    /// 安排一次原生单次回调；`Ok(false)` 表示不支持该阶段，调度器必须回退。
    fn request_native_frame(&mut self, request: NativeFrameRequest) -> Result<bool>;

    /// 确认与 `token` 关联的帧已经提交。
    ///
    /// 需要在呈现后开始原生等待的后端使用此钩子；无需动作的后端也须显式声明。
    fn native_frame_presented(&mut self, token: FrameRequestToken) -> Result<()>;

    /// 失效仍在途的原生请求；不能取消 OS 对象的后端至少必须抑制该令牌的交付。
    fn cancel_native_frame(&mut self, token: FrameRequestToken) -> Result<()>;

    fn native_surface_ptr(&self) -> *mut std::ffi::c_void;
}
