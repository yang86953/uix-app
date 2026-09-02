//! Wayland 客户端装饰阴影的可选平台适配。

use wayland_client::{
    QueueHandle,
    globals::GlobalList,
    protocol::{wl_shm, wl_surface},
};
use wayland_protocols_plasma::shadow::client::{
    org_kde_kwin_shadow::OrgKdeKwinShadow, org_kde_kwin_shadow_manager::OrgKdeKwinShadowManager,
};

use crate::core::WindowId;
use crate::native::windowing::shared::shadow_profile as profile;

use super::compat::{Main, ProxyContext, WaylandDispatchState};
use super::shm_buffer::{ShmBuffer, create_argb_buffer};

// 持有协议对象和八块 SHM buffer，确保 compositor 使用期间资源不失效。
pub(crate) struct WaylandClientShadow {
    manager: Main<OrgKdeKwinShadowManager>,
    // compositor 只在 wl_surface commit 应用 shadow 状态并通知渲染端，
    // 因此 shadow 对象在每次启用时重建，禁用后不保留。
    shadow: Option<Main<OrgKdeKwinShadow>>,
    buffers: Vec<ShmBuffer>,
    surface: Main<wl_surface::WlSurface>,
    // tile 生成时的输出缩放，供启用时换算逐边协议偏移。
    scale: i32,
    enabled: bool,
}

impl WaylandClientShadow {
    // 协议缺失或阴影资源不可用时安全回退，不影响标准 Wayland 窗口创建。
    pub(crate) fn try_create(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WaylandDispatchState>,
        context: ProxyContext,
        shm: &Main<wl_shm::WlShm>,
        surface: &Main<wl_surface::WlSurface>,
        window_id: WindowId,
        scale: i32,
    ) -> Option<Self> {
        // KDE shadow 是可选协议：compositor 不提供时按无阴影降级，不产生
        // 用户可见错误。
        let manager = globals
            .bind::<OrgKdeKwinShadowManager, _, _>(queue_handle, 1..=2, ())
            .ok()
            .map(|proxy| Main::new(proxy, context))?;
        // tile 尺寸跟随各边外扩：边带保持 1px 维度由 compositor 拉伸，
        // 角部 tile 覆盖两轴外扩的完整矩形，接缝两侧取同一剖面场。
        let top = profile::top_extent() * scale;
        let side = profile::side_extent() * scale;
        let bottom = profile::bottom_extent() * scale;
        let specs = [
            ("left", side, 1),
            ("top-left", side, top),
            ("top", 1, top),
            ("top-right", side, top),
            ("right", side, 1),
            ("bottom-right", side, bottom),
            ("bottom", 1, bottom),
            ("bottom-left", side, bottom),
        ];
        let mut buffers = Vec::with_capacity(specs.len());
        for (label, width, height) in specs {
            let mut buffer = create_argb_buffer(
                shm,
                width,
                height,
                &format!("shadow-{}-{label}", window_id.raw()),
            )
            .inspect_err(|error| tracing::warn!("shadow buffer creation failed: {error}"))
            .ok()?;
            let pixels = shadow_pixels(label, width, height, scale);
            buffer
                .write_pixels(&pixels)
                .inspect_err(|error| tracing::warn!("shadow buffer write failed: {error}"))
                .ok()?;
            buffers.push(buffer);
        }
        Some(Self {
            manager,
            shadow: None,
            buffers,
            surface: surface.clone(),
            scale,
            enabled: false,
        })
    }

    // 把统一客户端装饰状态映射为平台阴影协议提交。
    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        if self.enabled == enabled {
            return;
        }
        if enabled {
            // 每次启用都重建 shadow 对象：服务端只在 wl_surface commit
            // 应用 pending shadow 并广播 shadowChanged，复用旧对象时
            // 协议级 commit 不触发渲染端刷新，阴影会停留在空状态。
            let shadow = self.manager.create_shadow(&self.surface);
            shadow.attach_left(&self.buffers[0].buffer);
            shadow.attach_top_left(&self.buffers[1].buffer);
            shadow.attach_top(&self.buffers[2].buffer);
            shadow.attach_top_right(&self.buffers[3].buffer);
            shadow.attach_right(&self.buffers[4].buffer);
            shadow.attach_bottom_right(&self.buffers[5].buffer);
            shadow.attach_bottom(&self.buffers[6].buffer);
            shadow.attach_bottom_left(&self.buffers[7].buffer);
            // 逐边偏移与 tile 尺寸同源：方向性阴影顶部短、底部长。
            let top = (profile::top_extent() * self.scale) as f64;
            let side = (profile::side_extent() * self.scale) as f64;
            let bottom = (profile::bottom_extent() * self.scale) as f64;
            shadow.set_left_offset(side);
            shadow.set_top_offset(top);
            shadow.set_right_offset(side);
            shadow.set_bottom_offset(bottom);
            shadow.commit();
            self.surface.commit();
            self.shadow = Some(shadow);
        } else {
            self.manager.unset(&self.surface);
            self.surface.commit();
            self.shadow = None;
        }
        self.enabled = enabled;
    }
}

// 缺口补画共享外观事实入口：把输出缩放与物理圆角换算为逐角补画参数。
pub(super) fn notch_fill(scale: i32, corner_radius: i32) -> ([f32; 4], i32) {
    profile::notch_fill(corner_radius, scale)
}

// 为八个方向生成同一有向投影场的连续 premultiplied 黑色阴影。
//
// 纹理坐标约定（KWin 九宫格 quad 以 patch 起点 = 远离窗口一侧采样）：
// 边带与角部 tile 的索引 0 一律对应最外缘、末尾索引对应贴窗边。边带
// tile 只承载一维剖面（compositor 沿另一轴拉伸），角部 tile 在两轴外
// 扩矩形内取双带剖面的无缝混合；所有 tile 共享 `shadow_profile` 的同
// 一衰减场，接缝两侧取值一致。距离按输出缩放换算为 scale=1 剖面坐标。
fn shadow_pixels(label: &str, width: i32, height: i32, scale: i32) -> Vec<u32> {
    let scale = scale.max(1) as f32;
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let alpha = match label {
                "left" => profile::side_profile((width as f32 - x as f32 - 0.5) / scale),
                "right" => profile::side_profile((x as f32 + 0.5) / scale),
                "top" => profile::top_profile((height as f32 - y as f32 - 0.5) / scale),
                "bottom" => profile::bottom_profile((y as f32 + 0.5) / scale),
                _ => corner_tile_alpha(label, x, y, width, height, scale),
            };
            pixels.push(((alpha.clamp(0.0, 1.0) * 255.0).round() as u32) << 24);
        }
    }
    pixels
}

// 角部 tile 逐像素取值：索引 0 = 最外角，末行末列 = 贴窗角；左右/上下
// 镜像按 tile 方位映射回剖面坐标，距离按输出缩放换算。
fn corner_tile_alpha(label: &str, x: i32, y: i32, width: i32, height: i32, scale: f32) -> f32 {
    let left = label.ends_with("left");
    let top = label.starts_with("top");
    let sx = (if left {
        width as f32 - x as f32 - 0.5
    } else {
        x as f32 + 0.5
    }) / scale;
    let sy = (if top {
        height as f32 - y as f32 - 0.5
    } else {
        y as f32 + 0.5
    }) / scale;
    let horizontal = profile::side_profile(sx);
    let vertical = if top {
        profile::top_profile(sy)
    } else {
        profile::bottom_profile(sy)
    };
    profile::corner_alpha(horizontal, vertical, sx, sy, !top)
}
