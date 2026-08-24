// 引入弹层缓存与表面方法所需的矩形类型。
use crate::core::Rect;

// 引入被扩展的日期范围选择器。
use super::DateRangePicker;
// 引入同一组件私有几何模块的解析与转换函数。
use super::geometry::{
    // 将相对组合面板转换为窗口绝对坐标。
    absolute_date_range_popup_rect,
    // 构造首次正式登记前的有限回退表面。
    date_range_fallback_surface,
    // 将绝对组合面板转换为组件本地坐标。
    local_date_range_popup_rect,
    // 归一化当前逻辑表面与锚点。
    normalize_date_range_rect,
    // 解析受当前表面约束的绝对组合面板。
    resolve_date_range_popup_rect,
};

// 为 DateRangePicker 提供共享组合面板缓存与实际视口方法。
impl DateRangePicker {
    // 解析并缓存当前实际组合面板矩形。
    pub(super) fn remember_popup_rect(
        // 借用组件状态。
        &self,
        // 接收触发器绝对布局矩形。
        frame: Rect,
        // 接收当前逻辑表面。
        surface: Rect,
    ) -> Rect {
        // 归一化并缓存当前逻辑表面。
        let surface = normalize_date_range_rect(surface);
        // 使用共享解析器生成绝对组合面板矩形。
        let absolute =
            resolve_date_range_popup_rect(frame, self.presets.len(), surface, self.visual);
        // 转换为组件事件路径可复用的相对矩形。
        let local = local_date_range_popup_rect(frame, absolute);
        // 缓存最终相对组合面板矩形。
        self.popup_rect.set(local);
        // 记录当前逻辑表面供 dirty、命中和旧入口复用。
        self.surface_rect.set(Some(surface));
        // 记录绝对锚点供事件路径复用。
        self.popup_anchor_frame
            // 保存归一化后的触发器矩形。
            .set(Some(normalize_date_range_rect(frame)));
        // 读取本次呈现周期已累计的绝对组合面板脏区。
        let damage = self.popup_damage_rect.get();
        // 只有可见组合面板才进入历史脏区。
        if absolute.w > 0.0 && absolute.h > 0.0 {
            // 首次保存当前面板，后续合并旧新矩形。
            self.popup_damage_rect.set(
                // 已有有效历史时执行合并。
                if damage.w > 0.0 && damage.h > 0.0 {
                    // 覆盖方向、尺寸、预设数量和锚点变化前后的区域。
                    damage.union(&absolute)
                // 处理首次解析。
                } else {
                    // 首次解析直接保存当前组合面板。
                    absolute
                },
            );
        }
        // 返回所有消费者共享的最终本地矩形。
        local
    }

    // 返回最近记录的表面，首次登记前使用有限回退。
    pub(super) fn surface_or_fallback(&self, frame: Rect) -> Rect {
        // 优先读取正式登记或绘制记录的真实表面。
        self.surface_rect
            // 读取可复制的可选表面。
            .get()
            // 首次使用时按当前预设数构造有限表面。
            .unwrap_or_else(|| date_range_fallback_surface(frame, self.presets.len(), self.visual))
    }

    // 返回事件路径应使用的实际组合面板矩形。
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
        let surface = date_range_fallback_surface(frame, self.presets.len(), self.visual);
        // 解析回退绝对组合面板。
        let absolute =
            resolve_date_range_popup_rect(frame, self.presets.len(), surface, self.visual);
        // 只返回本地几何并等待正式登记记录表面。
        local_date_range_popup_rect(frame, absolute)
    }

    // 返回表面变化期间需要覆盖的绝对组合面板脏区。
    pub(super) fn damage_popup_rect(&self, frame: Rect, surface: Rect) -> Rect {
        // 先解析并缓存当前实际组合面板。
        let current = self.remember_popup_rect(frame, surface);
        // 将当前相对组合面板转换为窗口绝对坐标。
        let current = absolute_date_range_popup_rect(frame, current);
        // 读取已经合并方向、尺寸、预设数和锚点变化的历史区域。
        let damage = self.popup_damage_rect.get();
        // 有效历史区域覆盖当前与此前组合面板。
        if damage.w > 0.0 && damage.h > 0.0 {
            // 返回历史合并结果。
            damage
        // 处理空历史。
        } else {
            // 空历史回退到当前组合面板。
            current
        }
    }

    // 开启新的呈现周期时重置组合面板几何缓存。
    pub(super) fn reset_popup_presentation(&self) {
        // 清除此前呈现周期的本地组合面板矩形。
        self.popup_rect.set(Rect::zero());
        // 清除此前呈现周期的逻辑表面。
        self.surface_rect.set(None);
        // 清除此前呈现周期的绝对锚点。
        self.popup_anchor_frame.set(None);
        // 清除此前呈现周期的历史脏区。
        self.popup_damage_rect.set(Rect::zero());
    }
}
