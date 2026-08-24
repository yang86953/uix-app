//! RichText 公开构建器、reconcile 与测试观测方法。

// 子模块实现需要访问 RichText 根模块的私有状态与公开类型。
use super::*;

// 默认实例沿用组件宏生成的 new 构造器。
impl Default for RichText {
    // 返回默认富文本组件。
    fn default() -> Self {
        // 复用唯一初始化入口。
        Self::new()
    }
}

// 实现 RichText 的公开配置与内部同步方法。
impl RichText {
    /// 获取解析后的默认字体大小（像素）。
    /// 如果设置了物理单位，通过 DPI 转换。
    pub fn resolved_font_size_px(&self, dpi: f32) -> f32 {
        self.default_font_size_unit
            .map(|unit| unit.to_dip(dpi))
            .unwrap_or(self.default_font_size)
    }

    /// 设置富文本内容。
    pub fn content(mut self, segments: Vec<RichTextSegment>) -> Self {
        self.segments = segments;
        self.layout_dirty.set(true);
        self
    }

    /// 设置默认字体大小。
    pub fn font_size(mut self, size: f32) -> Self {
        let authored = size.is_finite() && size > 0.0;
        self.default_font_size = if authored {
            size
        } else {
            self.visual.defaults.font_size
        };
        self.font_size_authored = authored;
        self.default_font_size_unit = None;
        self.layout_dirty.set(true);
        self
    }

    /// 设置物理单位默认字体大小（优先级高于 `font_size()`）。
    pub fn font_size_unit(mut self, unit: PhysicalUnit) -> Self {
        self.default_font_size_unit = Some(unit);
        self.font_size_authored = true;
        self.layout_dirty.set(true);
        self
    }

    /// 设置默认文字颜色。
    pub fn color(mut self, color: Color) -> Self {
        self.default_color = color;
        self.use_theme_color = false;
        self.layout_dirty.set(true);
        self
    }

    // 从声明式下一实例同步公开配置并保留运行时状态。
    pub(crate) fn sync_from(&mut self, next: Self) {
        // 记录协调前结构策略与公开构建器组合后的选择能力。
        let selection_was_enabled = self.selection_enabled();
        // 记录公开段是否变化。
        let segments_changed = self.segments != next.segments;
        // 记录 UIX 静态视觉是否变化。
        let visual_changed = self.visual != next.visual;
        // 预先计算下一份内容可能需要的最大 run 字节数，供陈旧缓冲回收。
        let next_max_run_bytes = next
            .segments
            .iter()
            .map(|segment| match segment {
                RichTextSegment::Text { content, .. }
                | RichTextSegment::Code { content }
                | RichTextSegment::Link { content, .. } => content.len(),
                #[cfg(feature = "image-codecs")]
                RichTextSegment::Image { alt, .. } => alt.len(),
                RichTextSegment::ThematicBreak | RichTextSegment::NewLine => 0,
            })
            .max()
            .unwrap_or(0);
        // 汇总全部影响布局缓存的配置变化。
        let layout_config_changed = segments_changed
            || self.default_font_size != next.default_font_size
            || self.default_font_size_unit != next.default_font_size_unit
            || self.default_color != next.default_color
            || self.use_theme_color != next.use_theme_color
            || visual_changed;

        // 同步公开段列表。
        self.segments = next.segments;
        // 同步默认字号。
        self.default_font_size = next.default_font_size;
        // 同步物理字号覆盖。
        self.default_font_size_unit = next.default_font_size_unit;
        // 同步显式颜色。
        self.default_color = next.default_color;
        // 同步主题颜色策略。
        self.use_theme_color = next.use_theme_color;
        // 同步由 UIX 构建根注入的静态视觉表。
        self.visual = next.visual;
        // 同步字号是否由调用方显式声明。
        self.font_size_authored = next.font_size_authored;
        // 同步公开的选择配置。
        self.selectable = next.selectable;
        // 只在最终组合能力由开变关时清理选择生命周期。
        let selection_disabled = selection_was_enabled && !self.selection_enabled();
        // 只用新实例中明确提供的回调替换旧回调。
        if next.on_link.is_some() {
            // 保存新链接回调。
            self.on_link = next.on_link;
        }

        // 能力关闭边界必须终止已有选区。
        if selection_disabled {
            // 清除旧选区。
            self.selection.set(None);
            // 把锚点恢复到初始位置。
            self.sel_anchor.set(0);
            // 结束拖拽状态。
            self.sel_dragging.set(false);
        }

        // 任何布局配置变化都丢弃派生几何缓存。
        if layout_config_changed {
            // 清除视觉行。
            self.layout_lines.borrow_mut().clear();
            // 清除总高度。
            self.layout_height.set(0.0);
            // 清除内容宽度。
            self.content_width.set(0.0);
            // 清除上次约束。
            self.last_layout_width.set(0.0);
            // 清除主题颜色键。
            self.last_layout_palette.set(None);
            // 清除代码复制区域。
            self.code_regions.borrow_mut().clear();
            // 清除代码悬停状态。
            self.hovered_code.set(None);
            // 标记布局待重建。
            self.layout_dirty.set(true);
        }
        // 公开段变化还必须终止所有按段索引保存的交互状态。
        if segments_changed {
            // 清空复用的逐 run UTF-8 文本，但保留常用容量。
            let scratch = self.run_text_scratch.get_mut();
            scratch.clear();
            // 内容大幅缩小时回收历史超大 run 占用，避免长期滞留峰值内存。
            let retained_limit = next_max_run_bytes.saturating_mul(4).max(1024);
            if scratch.capacity() > retained_limit {
                scratch.shrink_to(next_max_run_bytes);
            }
            // 旧图片索引和资源身份全部失效。
            self.image_states.borrow_mut().clear();
            // 清除选择。
            self.selection.set(None);
            // 重置选择锚点。
            self.sel_anchor.set(0);
            // 结束拖拽。
            self.sel_dragging.set(false);
            // 清除链接悬停。
            self.hovered_link.set(None);
            // 清除按下动作。
            self.pressed_action = None;
            // 清除待提交链接。
            self.pending_submit.borrow_mut().take();
            // 清除待复制代码。
            if let Ok(mut pending_copy) = self.pending_copy.lock() {
                // 丢弃旧段产生的内容。
                pending_copy.take();
            }
            // 把链接焦点钳制到新链接集合。
            self.focused_link = if self.link_count() == 0 {
                // 没有链接时回到零。
                0
            } else {
                // 保留仍然合法的旧序号。
                self.focused_link.min(self.link_count() - 1)
            };
        }
    }

    // 生成组件结构化快照字段。
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        // 保存公开配置与焦点事实。
        SnapshotFields::RichText {
            // 克隆公开段列表。
            segments: self.segments.clone(),
            // 保存默认字号。
            default_font_size: self.default_font_size,
            // 保存物理字号覆盖。
            default_font_size_unit: self.default_font_size_unit,
            // 保存默认颜色。
            default_color: self.default_color,
            // 只有存在链接时才暴露焦点序号。
            focused_link: (self.link_count() > 0).then_some(self.focused_link),
        }
    }

    /// 返回当前键盘焦点链接的显示文本与 URL。
    pub fn focused_link(&self) -> Option<(&str, &str)> {
        self.link_at_ordinal(self.focused_link)
    }

    /// 注册链接激活回调，与 Submit 语义事件共存。
    pub fn on_link<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + 'static,
    {
        self.on_link = Some(Rc::new(callback));
        self
    }

    /// 获取选中的文本。
    pub fn selected_text(&self) -> Option<String> {
        self.selection
            .get()
            .map(|(start, end)| self.extract_text_range(start, end))
    }

    // 返回是否正在参与跨节点文字拖选。
    pub(crate) fn is_cross_text_dragging(&self) -> bool {
        self.sel_dragging.get()
    }

    // 返回跨节点选择使用的逻辑文本长度。
    pub(crate) fn cross_text_len(&self) -> usize {
        self.segments
            .iter()
            .map(|segment| match segment {
                RichTextSegment::Text { content, .. } => content.chars().count(),
                RichTextSegment::Code { content } => content.chars().count(),
                RichTextSegment::Link { content, .. } => content.chars().count(),
                // 图片以 alt 文本参与跨组件选择长度。
                #[cfg(feature = "image-codecs")]
                RichTextSegment::Image { alt, .. } => alt.chars().count(),
                RichTextSegment::ThematicBreak => 0,
                RichTextSegment::NewLine => 1,
            })
            .sum()
    }

    // 返回跨节点选择锚点。
    pub(crate) fn cross_text_anchor(&self) -> usize {
        self.sel_anchor.get()
    }

    // 更新跨节点选择范围。
    pub(crate) fn set_cross_text_range(&self, range: Option<(usize, usize)>) {
        // 只保留非空范围。
        match range {
            // 跨节点选择仍复用完整富文本字素簇归一规则。
            Some((start, end)) if start != end => self.set_selection_range(start, end),
            // 空范围清除选择。
            _ => self.selection.set(None),
        }
    }

    // 把局部指针位置映射到跨节点逻辑字符位置。
    pub(crate) fn cross_text_char_at(&self, frame_local: Point) -> usize {
        // 借用当前真实布局行。
        let lines = self.layout_lines.borrow();
        // 复用方向和字素簇感知命中。
        self.char_at_pos(frame_local, &lines)
    }

    /// 获取待复制的代码内容（由主循环调用）。
    pub fn take_pending_copy(&self) -> Option<String> {
        self.pending_copy
            .lock()
            .ok()
            .and_then(|mut pending| pending.take())
    }

    // 测试目标保留代码复制区域观测入口。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn code_copy_rect_for_test(&self, index: usize) -> Option<Rect> {
        self.code_regions
            .borrow()
            .get(index)
            .map(|region| region.rect)
    }

    // 测试目标保留布局行文本观测入口。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn layout_line_texts_for_test(&self) -> Vec<String> {
        self.layout_lines
            .borrow()
            .iter()
            .map(|line| line.glyphs.iter().map(|glyph| glyph.ch).collect())
            .collect()
    }

    // 测试目标保留链接命中点观测入口。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn link_point_for_test(&self, ordinal: usize) -> Option<Point> {
        // 解析目标链接的公开段索引。
        let segment_idx = self.link_segment_at_ordinal(ordinal)?;
        // 查找该段第一个真实字形的中心点。
        self.layout_lines.borrow().iter().find_map(|line| {
            line.glyphs
                .iter()
                .find(|glyph| glyph.segment_idx == segment_idx)
                .map(|glyph| Point::new(glyph.x + glyph.width * 0.5, line.y + line.height * 0.5))
        })
    }
}
