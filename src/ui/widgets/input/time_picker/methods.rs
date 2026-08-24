// 引入弹层缓存与表面方法所需的矩形类型。
use crate::core::Rect;

// 引入被扩展的时间选择器与列标识。
use super::{TimeColumn, TimePicker};
// 引入同一组件私有几何模块的解析与转换函数。
use super::geometry::{
    // 将相对时间面板转换为窗口绝对坐标。
    absolute_time_popup_rect,
    // 将绝对时间面板转换为组件本地坐标。
    local_time_popup_rect,
    // 归一化当前逻辑表面与锚点。
    normalize_time_rect,
    // 解析受当前表面约束的绝对时间面板。
    resolve_time_popup_rect,
    // 构造首次正式登记前的有限回退表面。
    time_fallback_surface,
};

// 为 TimePicker 提供共享时间面板缓存与实际视口方法。
impl TimePicker {
    // 解析并缓存当前实际时间面板矩形。
    pub(super) fn remember_popup_rect(&self, frame: Rect, surface: Rect) -> Rect {
        // 归一化并缓存当前逻辑表面。
        let surface = normalize_time_rect(surface);
        // 使用共享解析器生成绝对时间面板矩形。
        let absolute = resolve_time_popup_rect(frame, surface, self.visual);
        // 转换为组件事件路径可复用的相对矩形。
        let local = local_time_popup_rect(frame, absolute);
        // 读取此前缓存的最终时间面板。
        let previous = self.popup_rect.get();
        // 判断方向或实际视口尺寸是否发生变化。
        let geometry_changed = previous != local;
        // 缓存最终相对时间面板矩形。
        self.popup_rect.set(local);
        // 记录当前逻辑表面供 dirty、命中和旧入口复用。
        self.surface_rect.set(Some(surface));
        // 记录绝对锚点供事件路径复用。
        self.popup_anchor_frame
            // 保存归一化后的触发器矩形。
            .set(Some(normalize_time_rect(frame)));
        // 视口变化时保证两列当前高亮仍与实际视口相交。
        if geometry_changed && local.h > 0.0 {
            // 调整小时列滚动以显露当前高亮。
            self.ensure_highlight_visible(TimeColumn::Hour, local.h);
            // 调整分钟列滚动以显露当前高亮。
            self.ensure_highlight_visible(TimeColumn::Minute, local.h);
        }
        // 读取本次呈现周期已累计的绝对时间面板脏区。
        let damage = self.popup_damage_rect.get();
        // 只有可见时间面板才进入历史脏区。
        if absolute.w > 0.0 && absolute.h > 0.0 {
            // 首次保存当前面板，后续合并旧新矩形。
            self.popup_damage_rect.set(
                // 已有有效历史时执行合并。
                if damage.w > 0.0 && damage.h > 0.0 {
                    // 覆盖方向、尺寸、表面和锚点变化前后的区域。
                    damage.union(&absolute)
                // 处理首次解析。
                } else {
                    // 首次解析直接保存当前时间面板。
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
            // 首次使用时构造有限表面。
            .unwrap_or_else(|| time_fallback_surface(frame, self.visual))
    }

    // 返回事件路径应使用的实际时间面板矩形。
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
        let surface = time_fallback_surface(frame, self.visual);
        // 解析回退绝对时间面板。
        let absolute = resolve_time_popup_rect(frame, surface, self.visual);
        // 只返回本地几何并等待正式登记记录表面。
        local_time_popup_rect(frame, absolute)
    }

    // 返回表面变化期间需要覆盖的绝对时间面板脏区。
    pub(super) fn damage_popup_rect(&self, frame: Rect, surface: Rect) -> Rect {
        // 先解析并缓存当前实际时间面板。
        let current = self.remember_popup_rect(frame, surface);
        // 将当前相对时间面板转换为窗口绝对坐标。
        let current = absolute_time_popup_rect(frame, current);
        // 读取已经合并方向、尺寸、表面和锚点变化的历史区域。
        let damage = self.popup_damage_rect.get();
        // 有效历史区域覆盖当前与此前时间面板。
        if damage.w > 0.0 && damage.h > 0.0 {
            // 返回历史合并结果。
            damage
        // 处理空历史。
        } else {
            // 空历史回退到当前时间面板。
            current
        }
    }

    // 返回当前事件与键盘路径应使用的实际视口高度。
    pub(super) fn interaction_viewport_height(&self) -> f32 {
        // 尝试从最近本地触发器解析当前时间面板。
        let height = self
            // 读取最近本地 frame。
            .last_frame
            // 复制可选矩形。
            .get()
            // 使用共享事件几何解析实际面板高度。
            .map(|frame| self.interaction_popup_rect(frame).h)
            // 丢弃空视口。
            .filter(|height| *height > 0.0);
        // 首次登记前使用自然视口高度。
        height.unwrap_or(self.visual.layout.popup_height)
    }

    // 开启新的呈现周期时重置时间面板几何缓存。
    pub(super) fn reset_popup_presentation(&self) {
        // 清除此前呈现周期的本地时间面板矩形。
        self.popup_rect.set(Rect::zero());
        // 清除此前呈现周期的逻辑表面。
        self.surface_rect.set(None);
        // 清除此前呈现周期的绝对锚点。
        self.popup_anchor_frame.set(None);
        // 清除此前呈现周期的历史脏区。
        self.popup_damage_rect.set(Rect::zero());
    }
}
