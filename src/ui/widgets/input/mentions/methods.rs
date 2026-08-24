// 引入弹层缓存与视口方法所需的矩形类型。
use crate::core::Rect;

// 引入被扩展的提及组件。
use super::Mentions;
// 引入同一组件私有几何模块的解析与转换函数。
use super::geometry::{
    // 将相对弹层转换为窗口绝对坐标。
    absolute_mentions_popup_rect,
    // 将绝对弹层转换为组件本地坐标。
    local_mentions_popup_rect,
    // 构造首次登记前的有限回退表面。
    mentions_fallback_surface,
    // 归一化当前逻辑表面与锚点。
    normalize_mentions_rect,
    // 解析受当前表面约束的绝对弹层。
    resolve_mentions_popup_rect,
};

// 为 Mentions 提供共享弹层缓存与实际视口方法。
impl Mentions {
    // 解析并缓存当前实际提及弹层矩形。
    pub(super) fn remember_popup_rect(
        // 借用组件状态。
        &self,
        // 接收触发器绝对布局矩形。
        frame: Rect,
        // 接收当前逻辑表面。
        surface: Rect,
        // 接收当前候选行数。
        item_count: usize,
        // 返回相对触发器原点的最终弹层矩形。
    ) -> Rect {
        // 归一化并缓存当前逻辑表面。
        let surface = normalize_mentions_rect(surface);
        // 使用共享解析器生成绝对弹层矩形。
        let absolute = resolve_mentions_popup_rect(frame, item_count, surface, self.visual);
        // 转换为组件事件路径可复用的相对矩形。
        let local = local_mentions_popup_rect(frame, absolute);
        // 缓存最终相对弹层矩形。
        self.popup_rect.set(local);
        // 记录缓存对应的至少一行显示账本。
        self.popup_row_count.set(item_count.max(1));
        // 记录当前逻辑表面供 dirty、命中和旧入口复用。
        self.surface_rect.set(Some(surface));
        // 记录绝对锚点供候选数变化后的事件重算复用。
        self.popup_anchor_frame
            // 保存归一化后的触发器矩形。
            .set(Some(normalize_mentions_rect(frame)));
        // 读取当前呈现周期已累计的绝对弹层脏区。
        let damage = self.popup_damage_rect.get();
        // 只有可见弹层才进入历史脏区。
        if absolute.w > 0.0 && absolute.h > 0.0 {
            // 首次解析直接保存，后续变化合并旧新矩形。
            self.popup_damage_rect.set(
                // 已有有效脏区时合并。
                if damage.w > 0.0 && damage.h > 0.0 {
                    // 覆盖方向、尺寸、过滤或锚点变化前后的区域。
                    damage.union(&absolute)
                } else {
                    // 首次解析使用当前弹层。
                    absolute
                },
            );
        }
        // 返回同一最终本地矩形。
        local
    }

    // 返回最近记录的表面，首次登记前使用有限回退。
    pub(super) fn surface_or_fallback(&self, frame: Rect, item_count: usize) -> Rect {
        // 优先读取显式登记或绘制记录的真实表面。
        self.surface_rect
            // 读取可复制的可选表面。
            .get()
            // 首次使用时按当前行数构造有限表面。
            .unwrap_or_else(|| mentions_fallback_surface(frame, item_count, self.visual))
    }

    // 返回事件路径应使用的实际弹层矩形。
    pub(super) fn interaction_popup_rect(
        // 借用组件状态。
        &self,
        // 接收组件本地触发器 frame。
        frame: Rect,
        // 接收事件所需的当前候选行数。
        item_count: usize,
        // 返回相对组件原点的最终弹层矩形。
    ) -> Rect {
        // 统一空列表与非空列表的至少一行显示账本。
        let display_row_count = item_count.max(1);
        // 同行数缓存可直接保证事件与登记、绘制一致。
        if self.popup_row_count.get() == display_row_count {
            // 返回最近解析的最终本地矩形。
            return self.popup_rect.get();
        }
        // 真实表面与绝对锚点齐备时按当前候选数重新解析。
        if let (Some(surface), Some(anchor)) =
            (self.surface_rect.get(), self.popup_anchor_frame.get())
        {
            // 使用绝对锚点保持窗口与事件坐标转换一致。
            return self.remember_popup_rect(anchor, surface, item_count);
        }
        // 首次登记前构造有限回退表面。
        let surface = mentions_fallback_surface(frame, item_count, self.visual);
        // 解析回退绝对弹层。
        let absolute = resolve_mentions_popup_rect(frame, item_count, surface, self.visual);
        // 只返回本地几何，等待正式登记记录绝对锚点与脏区。
        local_mentions_popup_rect(frame, absolute)
    }

    // 返回状态或表面变化期间需要覆盖的绝对弹层脏区。
    pub(super) fn damage_popup_rect(
        // 借用组件状态。
        &self,
        // 接收触发器绝对布局矩形。
        frame: Rect,
        // 接收当前逻辑表面。
        surface: Rect,
        // 接收当前候选行数。
        item_count: usize,
        // 返回当前呈现周期累计的绝对脏区。
    ) -> Rect {
        // 先解析并缓存当前实际弹层。
        let current = self.remember_popup_rect(frame, surface, item_count);
        // 将当前相对矩形转换到窗口绝对坐标。
        let current = absolute_mentions_popup_rect(frame, current);
        // 读取已经合并方向、尺寸、过滤与锚点变化的历史区域。
        let damage = self.popup_damage_rect.get();
        // 有效历史区域覆盖当前与此前弹层。
        if damage.w > 0.0 && damage.h > 0.0 {
            // 返回历史合并结果。
            damage
        } else {
            // 空历史回退到当前弹层。
            current
        }
    }

    // 返回滚动与键盘显露应使用的实际弹层高度。
    pub(super) fn effective_popup_viewport_height(&self, row_count: usize) -> f32 {
        // 事件路径与绘制共用最终弹层高度。
        self.interaction_popup_rect(self.interaction_frame(), row_count)
            // 读取实际视口高度并防止超过自然规格。
            .h
            // 夹取到既有最大视口高度。
            .clamp(0.0, self.visual.layout.max_popup_height)
    }
}
