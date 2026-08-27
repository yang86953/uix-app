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

use super::compat::{Main, ProxyContext, WaylandDispatchState};
use super::shm_buffer::{ShmBuffer, create_argb_buffer};

const SHADOW_SIZE: i32 = 24;
const SHADOW_MAX_ALPHA: f32 = 0.28;

// 持有协议对象和八块 SHM buffer，确保 compositor 使用期间资源不失效。
pub(crate) struct WaylandClientShadow {
    manager: Main<OrgKdeKwinShadowManager>,
    // compositor 只在 wl_surface commit 应用 shadow 状态并通知渲染端，
    // 因此 shadow 对象在每次启用时重建，禁用后不保留。
    shadow: Option<Main<OrgKdeKwinShadow>>,
    buffers: Vec<ShmBuffer>,
    surface: Main<wl_surface::WlSurface>,
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
    ) -> Option<Self> {
        let manager = globals
            .bind::<OrgKdeKwinShadowManager, _, _>(queue_handle, 1..=2, ())
            .ok()
            .map(|proxy| Main::new(proxy, context))?;
        let specs = [
            ("left", SHADOW_SIZE, 1),
            ("top-left", SHADOW_SIZE, SHADOW_SIZE),
            ("top", 1, SHADOW_SIZE),
            ("top-right", SHADOW_SIZE, SHADOW_SIZE),
            ("right", SHADOW_SIZE, 1),
            ("bottom-right", SHADOW_SIZE, SHADOW_SIZE),
            ("bottom", 1, SHADOW_SIZE),
            ("bottom-left", SHADOW_SIZE, SHADOW_SIZE),
        ];
        let mut buffers = Vec::with_capacity(specs.len());
        for (label, width, height) in specs {
            let mut buffer = create_argb_buffer(
                shm,
                width,
                height,
                &format!("shadow-{}-{label}", window_id.raw()),
            )
            .ok()?;
            let pixels = shadow_pixels(label, width, height);
            buffer.write_pixels(&pixels).ok()?;
            buffers.push(buffer);
        }
        Some(Self {
            manager,
            shadow: None,
            buffers,
            surface: surface.clone(),
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
            let offset = SHADOW_SIZE as f64;
            shadow.set_left_offset(offset);
            shadow.set_top_offset(offset);
            shadow.set_right_offset(offset);
            shadow.set_bottom_offset(offset);
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

// 为八个方向生成一套连续的 premultiplied 黑色高斯近似阴影。
fn shadow_pixels(label: &str, width: i32, height: i32) -> Vec<u32> {
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let horizontal = match label {
                "left" | "top-left" | "bottom-left" => x as f32 + 0.5,
                "right" | "top-right" | "bottom-right" => width as f32 - x as f32 - 0.5,
                _ => SHADOW_SIZE as f32,
            };
            let vertical = match label {
                "top" | "top-left" | "top-right" => y as f32 + 0.5,
                "bottom" | "bottom-left" | "bottom-right" => height as f32 - y as f32 - 0.5,
                _ => SHADOW_SIZE as f32,
            };
            let distance = if width > 1 && height > 1 {
                horizontal
                    .min(SHADOW_SIZE as f32)
                    .hypot(vertical.min(SHADOW_SIZE as f32))
                    / 2.0_f32.sqrt()
            } else {
                horizontal.min(vertical)
            };
            let normalized = (distance / SHADOW_SIZE as f32).clamp(0.0, 1.0);
            let alpha = ((normalized * normalized * SHADOW_MAX_ALPHA) * 255.0).round() as u32;
            pixels.push(alpha << 24);
        }
    }
    pixels
}
