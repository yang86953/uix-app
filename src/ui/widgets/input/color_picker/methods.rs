// 引入弹层缓存与表面方法所需的矩形类型。
use crate::core::Rect;

// 引入被扩展的颜色选择器。
use super::ColorPicker;
// 引入同一组件私有几何模块的解析与转换函数。
use super::geometry::{
    // 将相对颜色面板转换为窗口绝对坐标。
    absolute_color_popup_rect,
    // 构造首次正式登记前的有限回退表面。
    color_fallback_surface,
    // 计算触发器、面板与当前表面的可见并集。
    color_surface_rect,
    // 将绝对颜色面板转换为组件本地坐标。
    local_color_popup_rect,
    // 归一化当前逻辑表面与锚点。
    normalize_color_rect,
    // 解析受当前表面约束的绝对颜色面板。
    resolve_color_popup_rect,
};

// 为颜色选择器提供共享面板缓存与表面方法。
impl ColorPicker {
    // 解析并缓存当前实际颜色面板矩形。
    pub(super) fn remember_popup_rect(&self, frame: Rect, surface: Rect) -> Rect {
        // 归一化并缓存当前逻辑表面。
        let surface = normalize_color_rect(surface);
        // 使用共享解析器生成绝对颜色面板矩形。
        let absolute = resolve_color_popup_rect(frame, surface, self.preset_colors.len());
        // 转换为组件事件路径可复用的相对矩形。
        let local = local_color_popup_rect(frame, absolute);
        // 缓存最终相对颜色面板矩形。
        self.popup_rect.set(local);
        // 记录当前逻辑表面供脏区、命中和旧入口复用。
        self.surface_rect.set(Some(surface));
        // 记录归一化后的绝对触发器锚点。
        self.popup_anchor_frame
            .set(Some(normalize_color_rect(frame)));
        // 读取本次呈现周期已经累计的绝对颜色面板脏区。
        let damage = self.popup_damage_rect.get();
        // 只有可见颜色面板才进入历史脏区。
        if absolute.w > 0.0 && absolute.h > 0.0 {
            // 首次保存当前面板，后续合并新旧矩形。
            self.popup_damage_rect
                .set(if damage.w > 0.0 && damage.h > 0.0 {
                    // 覆盖方向、尺寸、表面和锚点变化前后的区域。
                    damage.union(&absolute)
                // 处理首次解析。
                } else {
                    // 首次解析直接保存当前颜色面板。
                    absolute
                });
        }
        // 返回所有消费者共享的最终本地矩形。
        local
    }

    // 返回最近记录的表面，首次登记前使用有限回退。
    pub(super) fn surface_or_fallback(&self, frame: Rect) -> Rect {
        // 优先读取正式登记或绘制记录的真实表面。
        self.surface_rect
            // 复制可选表面。
            .get()
            // 首次使用时构造有限表面。
            .unwrap_or_else(|| color_fallback_surface(frame, self.preset_colors.len()))
    }

    // 返回事件路径应使用的实际颜色面板矩形。
    pub(super) fn interaction_popup_rect(&self, frame: Rect) -> Rect {
        // 真实表面与绝对锚点齐备时按同帧信息解析。
        if let (Some(surface), Some(anchor)) =
            // 同时读取最近表面和绝对锚点。
            (self.surface_rect.get(), self.popup_anchor_frame.get())
        {
            // 保持窗口登记与本地事件坐标转换一致。
            return self.remember_popup_rect(anchor, surface);
        }
        // 首次正式登记前构造有限回退表面。
        let surface = color_fallback_surface(frame, self.preset_colors.len());
        // 解析回退绝对颜色面板。
        let absolute = resolve_color_popup_rect(frame, surface, self.preset_colors.len());
        // 只返回本地几何并等待正式登记记录表面。
        local_color_popup_rect(frame, absolute)
    }

    // 返回表面变化期间需要覆盖的绝对颜色面板脏区。
    pub(super) fn damage_popup_rect(&self, frame: Rect, surface: Rect) -> Rect {
        // 先解析并缓存当前实际颜色面板。
        let current = self.remember_popup_rect(frame, surface);
        // 将当前相对颜色面板转换为窗口绝对坐标。
        let current = absolute_color_popup_rect(frame, current);
        // 读取已经合并方向、尺寸、表面和锚点变化的历史区域。
        let damage = self.popup_damage_rect.get();
        // 有效历史区域覆盖当前与此前颜色面板。
        if damage.w > 0.0 && damage.h > 0.0 {
            // 返回历史合并结果。
            damage
        // 处理空历史。
        } else {
            // 空历史回退到当前颜色面板。
            current
        }
    }

    // 返回当前呈现周期在逻辑表面内的完整脏区。
    pub(super) fn presentation_dirty_rect(&self, frame: Rect) -> Rect {
        // 读取最近登记或首帧有限回退表面。
        let surface = self.surface_or_fallback(frame);
        // 合并当前与本呈现周期历史颜色面板。
        let popup = self.damage_popup_rect(frame, surface);
        // 将触发器与颜色面板历史裁剪到当前表面内。
        color_surface_rect(frame, popup, surface)
    }

    // 开启新的呈现周期时重置颜色面板几何缓存。
    pub(super) fn reset_popup_presentation(&self) {
        // 清除此前呈现周期的本地颜色面板矩形。
        self.popup_rect.set(Rect::zero());
        // 清除此前呈现周期的逻辑表面。
        self.surface_rect.set(None);
        // 清除此前呈现周期的绝对锚点。
        self.popup_anchor_frame.set(None);
        // 清除此前呈现周期的历史脏区。
        self.popup_damage_rect.set(Rect::zero());
    }
}
