use super::*;

impl WidgetTree {
    /// 2D 命中测试：根据屏幕坐标找到最深的 widget。
    pub fn hit_test(&self, pos: Point) -> Option<WidgetId> {
        // 已停止的树不得用半提交的节点关系计算命中目标。
        if !self.accepts_external_work() {
            // 对事件入口报告没有可交互目标。
            return None;
        }
        self.root_id
            .and_then(|root| self.hit_test_internal(root, pos))
    }

    /// 3D 命中测试：根据 3D 射线找到最深的 widget。
    ///
    /// `spatial` 为当前空间上下文（用于逆变换）。
    /// 使用 Widget::hit_test_3d 方法，支持 3D 变换后的 widget。
    pub fn hit_test_3d(
        &self,
        ray: &crate::draw::geometry::spatial::Ray3D,
        spatial: &crate::draw::geometry::spatial::SpatialContext,
    ) -> Option<WidgetId> {
        // 已停止的树不得用半提交的节点关系计算三维命中目标。
        if !self.accepts_external_work() {
            // 对事件入口报告没有可交互目标。
            return None;
        }
        let root = self.root_id?;
        // 复用与二维命中相同的树级排序工作区；重入时安全回退到局部容器。
        if let Ok(mut order_scratch) = self.hit_test_order_scratch.try_borrow_mut() {
            order_scratch.clear();
            return self.hit_test_3d_internal(root, ray, spatial, &mut order_scratch);
        }
        self.hit_test_3d_internal(root, ray, spatial, &mut Vec::new())
    }

    /// 3D 命中测试内部递归。
    fn hit_test_3d_internal(
        &self,
        id: WidgetId,
        ray: &crate::draw::geometry::spatial::Ray3D,
        spatial: &crate::draw::geometry::spatial::SpatialContext,
        order_scratch: &mut Vec<(WidgetId, usize)>,
    ) -> Option<WidgetId> {
        let node = self.get(id)?;
        if !node.visible() || self.is_pending_removal_subtree(id) {
            return None;
        }
        // 把当前层追加到树级工作区；递归子层只使用尾部并在返回前截断。
        let children_start = order_scratch.len();
        order_scratch.extend(
            node.children()
                .iter()
                .copied()
                .enumerate()
                .map(|(original_order, child_id)| (child_id, original_order)),
        );
        let children_end = order_scratch.len();
        order_scratch[children_start..children_end].sort_unstable_by(
            |&(a, original_a), &(b, original_b)| {
                let za = self.get(a).map_or(0, |c| c.z_index());
                let zb = self.get(b).map_or(0, |c| c.z_index());
                // 保持旧稳定排序：同 z-index 的 3D 子节点仍按声明顺序检查。
                zb.cmp(&za).then_with(|| original_a.cmp(&original_b))
            },
        );
        let mut child_hit = None;
        for child_index in children_start..children_end {
            let child_id = order_scratch[child_index].0;
            if let Some(hit) = self.hit_test_3d_internal(child_id, ray, spatial, order_scratch) {
                child_hit = Some(hit);
                break;
            }
        }
        // 当前层完成后释放逻辑长度，保留容量供下一次命中测试复用。
        order_scratch.truncate(children_start);
        if child_hit.is_some() {
            return child_hit;
        }
        let frame = node.frame();
        if node.hit_test_3d(ray, spatial, frame) {
            Some(id)
        } else {
            None
        }
    }

    pub(super) fn hit_test_internal(&self, id: WidgetId, pos: Point) -> Option<WidgetId> {
        // 子树入口先验证完整祖先链；后续递归只需检查当前节点的待移除门控。
        if self.is_pending_removal_subtree(id) {
            return None;
        }
        // 任意子树入口先完整解析一次视觉祖先链，递归后代改用增量坐标。
        let layout_pos = self.point_to_node_layout(id, pos)?;
        // 正常事件循环复用树级工作区；极少数重入调用回退到局部容器避免 RefCell panic。
        if let Ok(mut order_scratch) = self.hit_test_order_scratch.try_borrow_mut() {
            order_scratch.clear();
            return self.hit_test_internal_with_scratch(id, pos, layout_pos, &mut order_scratch);
        }
        self.hit_test_internal_with_scratch(id, pos, layout_pos, &mut Vec::new())
    }

    fn hit_test_internal_with_scratch(
        &self,
        id: WidgetId,
        pos: Point,
        layout_pos: Point,
        order_scratch: &mut Vec<(WidgetId, usize)>,
    ) -> Option<WidgetId> {
        let node = self.get(id)?;
        // 祖先在入口或上一层递归中已经确认，避免每个后代重复回溯父链。
        if !node.visible() || node.pending_removal() {
            return None;
        }

        // 当前递归帧内组件只读不变，复用一次事件能力查询。
        let hit_test_children = node.hit_test_children();
        let can_hit_regular_children = hit_test_children
            && node
                .children_clip(node.frame())
                .is_none_or(|clip| clip.contains(layout_pos));
        // fixed 直接子树即使位于父裁剪外也必须进入命中遍历。
        if hit_test_children {
            // 把父节点视口坐标转换为子内容坐标，供片段裁剪命中复用。
            let child_clip_pos = node
                // 读取父节点施加在全部子项上的滚动偏移。
                .viewport_scroll_offset()
                // 滚动内容坐标等于视口坐标加当前偏移。
                .map(|(scroll_x, scroll_y)| {
                    // 构造与子节点布局 frame 相同坐标系中的命中点。
                    Point::new(layout_pos.x + scroll_x, layout_pos.y + scroll_y)
                })
                // 非滚动父节点直接沿用当前布局坐标。
                .unwrap_or(layout_pos);
            // 把本层子节点与原始顺序追加到唯一工作区，避免每个递归节点创建 Vec。
            let children_start = order_scratch.len();
            let children = node.children();
            let uniform_z = children.len() < 2 || {
                let first_z = self.get(children[0]).map_or(0, |child| child.z_index());
                children[1..].iter().all(|child_id| {
                    self.get(*child_id).map_or(0, |child| child.z_index()) == first_z
                })
            };
            if uniform_z {
                // 同层级无需比较排序，直接按“后声明优先”写入最终命中顺序。
                order_scratch.extend(
                    children
                        .iter()
                        .copied()
                        .enumerate()
                        .rev()
                        .map(|(original_order, child_id)| (child_id, original_order)),
                );
            } else {
                order_scratch.extend(
                    children
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(original_order, child_id)| (child_id, original_order)),
                );
            }
            let children_end = order_scratch.len();
            if !uniform_z {
                order_scratch[children_start..children_end].sort_unstable_by(
                    |&(a, original_a), &(b, original_b)| {
                        let za = self.get(a).map_or(0, |c| c.z_index());
                        let zb = self.get(b).map_or(0, |c| c.z_index());
                        // 同 z-index 时，后声明的兄弟绘制在上层，必须优先命中。
                        zb.cmp(&za).then_with(|| original_b.cmp(&original_a))
                    },
                );
            }
            let mut child_hit = None;
            for child_index in children_start..children_end {
                let child_id = order_scratch[child_index].0;
                // 单次借用复用 fixed、片段、浮层能力与局部视觉变换元数据。
                let Some(child) = self.get(child_id) else {
                    continue;
                };
                // fixed 子树脱离当前父级的滚动与裁剪命中门禁。
                let fixed = child.position().mode == crate::ui::PositionMode::Fixed;
                // 普通子树仍要求指针位于父级可命中裁剪内。
                if !fixed && !can_hit_regular_children {
                    // 继续检查可能提升为 fixed 的其他兄弟。
                    continue;
                }
                // 先按父布局为该子树声明的片段集合过滤命中。
                let inside_parent_regions = fixed || {
                    // 缺少片段表示普通未裁剪子树；Some(empty) 自然拒绝全部命中。
                    let mut inside = true;
                    child.visit_parent_clip_regions(&mut |regions| {
                        // 不连续片段使用集合命中，不能退化为联合包围盒。
                        inside = regions.iter().any(|region| region.contains(child_clip_pos));
                    });
                    inside
                };
                // 指针落在片段间隙时跳过整个子树。
                if !inside_parent_regions {
                    // 继续检查下一层视觉兄弟节点。
                    continue;
                }
                // 复用父节点已解析的内容坐标，只追加直接子节点的视觉变换。
                let Some(child_layout_pos) =
                    self.point_to_child_layout(child_id, child, child_clip_pos, pos)
                else {
                    // 不可逆变换与完整视觉路径语义一致，不产生命中。
                    continue;
                };
                if let Some(hit) = self.hit_test_internal_with_scratch(
                    child_id,
                    pos,
                    child_layout_pos,
                    order_scratch,
                ) {
                    child_hit = Some(hit);
                    break;
                }
            }
            // 当前层无论命中与否都恢复逻辑长度，容量留给后续高频事件。
            order_scratch.truncate(children_start);
            if child_hit.is_some() {
                return child_hit;
            }
        }
        // 使用 widget 的 hit_test_frame 代替原始 frame，支持 overlay 模式
        let actual_frame = node.frame();
        let hit_frame = node.hit_test_frame(actual_frame);
        if hit_frame.contains(layout_pos) {
            Some(id)
        } else {
            None
        }
    }

    // 验证屏幕点是否位于目标节点及其祖先链的父级片段裁剪中。
    pub(super) fn point_inside_parent_clip_regions(
        // 接收需要验证的目标节点。
        &self,
        // 接收目标节点标识。
        target: WidgetId,
        // 接收屏幕坐标中的指针位置。
        pos: Point,
    ) -> bool {
        // 从目标开始逐级检查每条父子边上的可选片段集合。
        let mut current = Some(target);
        // 祖先链有限，直到根节点结束。
        while let Some(id) = current {
            // 节点已移除时不能继续视为命中。
            let Some(node) = self.get(id) else {
                // 返回失败避免事件落到失效节点。
                return false;
            };
            // fixed 根及其内部边界已经检查完毕，不再继承外层父级片段。
            if self.node_is_fixed(id) {
                // 结束祖先裁剪遍历。
                break;
            }
            // 保存父节点标识供本轮片段换算与下一轮遍历。
            let parent = node.parent();
            // 只在父布局声明片段时执行集合命中，借用不逃逸到组件树外。
            let mut regions_accept = true;
            let has_regions = node.visit_parent_clip_regions(&mut |regions| {
                // 根节点没有父坐标系，不能合法携带父级片段。
                let Some(parent_id) = parent else {
                    regions_accept = false;
                    return;
                };
                // 读取父节点以换算其滚动内容坐标。
                let Some(parent_node) = self.get(parent_id) else {
                    regions_accept = false;
                    return;
                };
                // 将屏幕点转换到父节点布局坐标。
                let Some(mut parent_pos) = self.point_to_node_layout(parent_id, pos) else {
                    regions_accept = false;
                    return;
                };
                // 父节点滚动时，片段与子 frame 位于内容坐标。
                if let Some((scroll_x, scroll_y)) = parent_node.viewport_scroll_offset() {
                    parent_pos.x += scroll_x;
                    parent_pos.y += scroll_y;
                }
                // 不允许完整 frame 包围盒绕过不连续片段。
                regions_accept = regions.iter().any(|region| region.contains(parent_pos));
            });
            if has_regions && !regions_accept {
                return false;
            }
            // 上移到父节点继续检查更外层片段。
            current = parent;
        }
        // 全部父级片段约束都通过后接受命中。
        true
    }

    /// 在失效批次边界内向组件树分发系统事件。
    pub fn dispatch_event(&mut self, event: &SystemEvent) -> EventResult {
        self.dispatch_event_with_focus_policy(event, false)
    }

    /// 分发已通过 Agent 授权且显式绑定窗口的事件，不借用操作系统前台焦点。
    pub(crate) fn dispatch_agent_event(&mut self, event: &SystemEvent) -> EventResult {
        let result = self.dispatch_event_with_focus_policy(event, true);
        // 合成指针按下被拒（无人消费）时不保留按压手势：agent 桥不会为
        // 失败的按下补发成对抬起，残留按压会把后续合成移动锁死在按压
        // 捕获上，悬停全部失效；真实指针由系统保证按下/抬起成对，不经过
        // 此入口，行为不变。不补发抬起：按压目标与命中一致时抬起会合成
        // 语义 Click，让「报告失败」的动作实际生效。
        if result == EventResult::NotHandled
            && matches!(event, SystemEvent::PointerDown { .. })
        {
            self.cancel_active_pointer_gesture();
        }
        result
    }

    /// 已授权的完整点击序列：Down 未消费仍保留手势到同次 Up。
    pub(crate) fn dispatch_agent_click(&mut self, position: Point) -> EventResult {
        let down = self.dispatch_event_with_focus_policy(
            &SystemEvent::PointerDown {
                pos: position,
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
            true,
        );
        let up = self.dispatch_event_with_focus_policy(
            &SystemEvent::PointerUp {
                pos: position,
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
            true,
        );
        if up == EventResult::NotHandled {
            down
        } else {
            up
        }
    }

    fn dispatch_event_with_focus_policy(
        &mut self,
        event: &SystemEvent,
        allow_unfocused_input: bool,
    ) -> EventResult {
        // 已停止的树不得继续分发可能触发组件回调的系统事件。
        if !self.accepts_external_work() {
            // 明确拒绝事件，避免外层循环把半树视为可交互。
            return EventResult::NotHandled;
        }
        self.begin_invalidation_batch();
        let result = self.dispatch_event_inner(event, allow_unfocused_input);
        self.cancel_hidden_interaction();
        self.finish_invalidation_batch();
        result
    }

    pub(super) fn dispatch_event_inner(
        &mut self,
        event: &SystemEvent,
        allow_unfocused_input: bool,
    ) -> EventResult {
        if self
            .managers()
            .focus
            .focused_widget()
            .is_some_and(|focused| !self.focus_target_available(focused))
        {
            self.set_focus(None);
        }
        if !allow_unfocused_input && self.ignores_input_while_window_unfocused(event) {
            return EventResult::NotHandled;
        }

        match event {
            SystemEvent::PointerDown { pos, button, mods }
            | SystemEvent::PointerDoubleClick { pos, button, mods } => {
                self.dispatch_pointer_press(event, *pos, *button, *mods)
            }
            SystemEvent::PointerUp { pos, button, mods } => {
                self.dispatch_pointer_release(event, *pos, *button, *mods)
            }
            SystemEvent::PointerMove { pos, mods } => {
                // 拖拽手势检测：potential → active 转换
                if self.managers().drag.is_potential() && !self.managers().drag.is_dragging() {
                    let start_pos = self.managers().drag.start_pos();
                    let dx = pos.x - start_pos.x;
                    let dy = pos.y - start_pos.y;
                    // 5px 阈值：超出才视为拖拽开始
                    if dx.abs() > 5.0 || dy.abs() > 5.0 {
                        self.managers_mut().drag.activate_gesture();
                        // 发射 DragStart 到拖拽目标
                        if let Some(target) = self.managers().drag.target() {
                            let drag_start = SystemEvent::DragStart {
                                pos: *pos,
                                button: self.managers().drag.button(),
                                mods: *mods,
                            };
                            // DragStart 未被拖拽目标消费：记录日志定位路由失败，行为不变。
                            if self.dispatch_to(target, &drag_start) == EventResult::NotHandled {
                                tracing::warn!(
                                    event = "DragStart",
                                    target = ?target,
                                    "drag start was not handled"
                                );
                            }
                        }
                    }
                }
                // 拖拽进行中：发射 DragMove
                if self.managers().drag.is_dragging() {
                    if let Some(target) = self.managers().drag.target() {
                        let last_pos = self.managers().drag.last_pos();
                        let delta = Point::new(pos.x - last_pos.x, pos.y - last_pos.y);
                        let drag_move = SystemEvent::DragMove {
                            pos: *pos,
                            delta,
                            mods: *mods,
                        };
                        // DragMove 未被拖拽目标消费：记录日志，行为不变。
                        if self.dispatch_to(target, &drag_move) == EventResult::NotHandled {
                            tracing::warn!(event = "DragMove", target = ?target, "drag move was not handled");
                        }
                    }
                }
                if self.managers().drag.is_dragging() || self.managers().drag.is_potential() {
                    self.managers_mut().drag.update_drag(*pos);
                }

                if let Some(drag_target) = self.managers().interaction.pressed_widget() {
                    // 文字拖选：pressed 捕获会把 Move 锁在起点节点，须在树层
                    // 协调同父级兄弟行，才能向上/向下扩展选区。
                    if self
                        .get(drag_target)
                        .is_some_and(crate::ui::text_selection::is_dragging)
                        && self.apply_cross_text_selection_drag(drag_target, *pos)
                    {
                        self.rebuild_widget_overlays();
                        return EventResult::Handled;
                    }
                    let result = self.dispatch_to(drag_target, event);
                    self.rebuild_widget_overlays();
                    return result;
                }

                let current_hover = self.managers().interaction.hovered_widget();

                if let Some(hovered) = current_hover.filter(|hovered| {
                    self.get(*hovered)
                        .is_some_and(|node| node.is_interaction_enabled())
                }) {
                    // 快路径只对「指针位置不存在更深命中」的目标有效：叶目标
                    // （不再命中子节点或没有子节点）才允许跳过重命中。容器型
                    // 目标的 frame 覆盖整片区域，重建时序可能让当初的命中停在
                    // 祖先容器；此后指针在 frame 内的每次移动都必须重跑完整
                    // 命中，否则更深的可交互后代永远收不到 PointerEnter。
                    let hover_blocks_deeper_hits = self.get(hovered).is_some_and(|node| {
                        !node.hit_test_children() || node.children().is_empty()
                    });
                    if hover_blocks_deeper_hits
                        && self.pointer_inside_widget_hit_frame(hovered, *pos)
                    {
                        let result = if self
                            .get(hovered)
                            .is_some_and(|node| node.wants_continuous_pointer_move())
                        {
                            self.dispatch_to(hovered, event)
                        } else {
                            EventResult::NotHandled
                        };
                        self.rebuild_widget_overlays();
                        return result;
                    }
                }

                let old_hover = current_hover;
                let new_hover = self
                    .overlay_target_at(*pos)
                    .or_else(|| self.hit_test(*pos))
                    .filter(|target| {
                        self.get(*target)
                            .is_some_and(|node| node.is_interaction_enabled())
                    });
                if new_hover != current_hover {
                    // 仅当组件实际处理了 enter/leave（有 hover 视觉态）才窄标脏。
                    // Label/Icon 等叶子 NotHandled 时若仍 invalidate，局部清屏会挖掉父背景。
                    if let Some(old) = current_hover {
                        if self.dispatch_to(old, &SystemEvent::PointerLeave) == EventResult::Handled
                        {
                            self.invalidate_paint(old);
                        }
                    }
                    if let Some(new) = new_hover {
                        if self.dispatch_to(new, &SystemEvent::PointerEnter) == EventResult::Handled
                        {
                            self.invalidate_paint(new);
                        }
                    }
                    self.managers_mut()
                        .interaction
                        .set_hovered_widget(new_hover);
                }
                let result = if let Some(t) = new_hover {
                    if new_hover != old_hover
                        || self
                            .get(t)
                            .is_some_and(|node| node.wants_continuous_pointer_move())
                    {
                        self.dispatch_to(t, event)
                    } else {
                        EventResult::NotHandled
                    }
                } else {
                    EventResult::NotHandled
                };
                self.rebuild_widget_overlays();
                result
            }
            SystemEvent::Wheel { pos, .. } => {
                let target = self
                    .overlay_target_at(*pos)
                    .or_else(|| self.hit_test(*pos))
                    .or(self.managers().interaction.hovered_widget())
                    .or(self.root_id);
                if let Some(t) = target {
                    // 捕获阶段：ScrollView 等祖先先处理；Handled 时由 capture 侧登记动画与视口重绘，
                    // 避免仅 mark_dirty 子节点导致 strip 局部清除后内容消失。
                    if self.capture_wheel_to(t, event) == EventResult::Handled {
                        return EventResult::Handled;
                    }
                    self.dispatch_wheel_to(t, event)
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyDown { key, mods } => self.dispatch_key_down(event, *key, *mods),
            SystemEvent::KeyUp { key, .. } => self.dispatch_key_up(event, *key),
            SystemEvent::TextInput { text } => {
                if let Some(t) = self.managers().focus.focused_widget() {
                    self.invalidate_paint(t);
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled {
                        // TextInput 语义未被消费：记录日志，保持返回值语义不变。
                        if self.dispatch_semantic(SemanticEvent::text_input(t, text.clone()))
                            == EventResult::NotHandled
                        {
                            tracing::warn!(
                                event = "TextInput",
                                target = ?t,
                                "text input semantic was not handled"
                            );
                        }
                    }
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::ImeCompositionStart
            | SystemEvent::ImeCompositionUpdate { .. }
            | SystemEvent::ImeCompositionEnd { .. } => {
                if let Some(t) = self.managers().focus.focused_widget() {
                    self.invalidate_paint(t);
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled {
                        let semantic = match event {
                            SystemEvent::ImeCompositionStart => {
                                SemanticEvent::ime_composition_start(t)
                            }
                            SystemEvent::ImeCompositionUpdate { text } => {
                                SemanticEvent::ime_composition_update(t, text.clone())
                            }
                            SystemEvent::ImeCompositionEnd { text } => {
                                SemanticEvent::ime_composition_end(t, text.clone())
                            }
                            // 外层按 Ime 组事件分派，其余变体不可能进入本臂。
                            _ => unreachable!("ime arm only receives composition events"),
                        };
                        // IME 组合语义未被消费：记录日志，行为不变。
                        if self.dispatch_semantic(semantic) == EventResult::NotHandled {
                            tracing::warn!(
                                event = "ImeComposition",
                                target = ?t,
                                "ime composition semantic was not handled"
                            );
                        }
                    }
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::Copy | SystemEvent::Cut | SystemEvent::Paste { .. } => {
                if let Some(t) = self.managers().focus.focused_widget() {
                    self.invalidate_paint(t);
                    if matches!(event, SystemEvent::Copy) && self.try_copy_cross_text_selection(t) {
                        // 跨文本选区复制语义未被消费：记录日志，行为不变。
                        if self.dispatch_semantic(SemanticEvent::copy(t)) == EventResult::NotHandled
                        {
                            tracing::warn!(event = "Copy", target = ?t, "copy semantic was not handled");
                        }
                        return EventResult::Handled;
                    }
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled {
                        let semantic = match event {
                            SystemEvent::Copy => SemanticEvent::copy(t),
                            SystemEvent::Cut => SemanticEvent::cut(t),
                            SystemEvent::Paste { text } => SemanticEvent::paste(t, text.clone()),
                            // 外层按剪贴板组事件分派，其余变体不可能进入本臂。
                            _ => unreachable!("clipboard arm only receives copy/cut/paste"),
                        };
                        // 剪贴板语义未被消费：记录日志，行为不变。
                        if self.dispatch_semantic(semantic) == EventResult::NotHandled {
                            tracing::warn!(event = "Clipboard", target = ?t, "clipboard semantic was not handled");
                        }
                    }
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn | SystemEvent::FocusOut => {
                if let Some(t) = self.managers().focus.focused_widget() {
                    self.dispatch_to(t, event)
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerEnter | SystemEvent::PointerLeave => EventResult::NotHandled,
            SystemEvent::ThemeChanged { .. } => {
                self.notify_theme_changed();
                if let Some(root) = self.root_id {
                    self.dispatch_to(root, event)
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::LocaleChanged { .. } => {
                if let Some(root) = self.root_id {
                    self.push_layout_invalidation(root);
                    self.invalidate_paint(root);
                    self.dispatch_to(root, event)
                } else {
                    EventResult::NotHandled
                }
            }
            // 窗口状态变化事件 → 统一分发给 root，让应用层处理
            SystemEvent::WindowMaximize
            | SystemEvent::WindowMinimize
            | SystemEvent::WindowRestore
            | SystemEvent::WindowFocus
            | SystemEvent::WindowBlur => self.dispatch_window_lifecycle(event),
            SystemEvent::Timer { .. } => {
                let target = self
                    .managers()
                    .interaction
                    .hovered_widget()
                    .or(self.managers().focus.focused_widget())
                    .or(self.root_id);
                if let Some(target) = target {
                    let result = self.dispatch_to(target, event);
                    self.rebuild_widget_overlays();
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FileDrop { files, position } => {
                // 文件拖放优先分发给 overlay 命中节点，否则交给主树/root。
                if let Some(target) = self
                    .overlay_target_at(*position)
                    .or_else(|| self.hit_test(*position))
                    .or(self.root_id)
                {
                    let result = self.dispatch_to(target, event);
                    // FileDrop 语义未被消费：记录日志，行为不变。
                    if self.dispatch_semantic(SemanticEvent::file_drop(
                        target,
                        files.clone(),
                        *position,
                    )) == EventResult::NotHandled
                    {
                        tracing::warn!(event = "FileDrop", target = ?target, "file drop semantic was not handled");
                    }
                    result
                } else {
                    EventResult::NotHandled
                }
            }
            // 拖拽组合事件：由 dispatch_event 内部合成并直接 dispatch_to，
            // 不会从外部传入 dispatch_event。
            SystemEvent::DragStart { .. }
            | SystemEvent::DragMove { .. }
            | SystemEvent::DragEnd { .. } => EventResult::NotHandled,
            SystemEvent::Resize { width, height } => {
                if let Some(root) = self.root_id {
                    if *width > 0.0 && *height > 0.0 {
                        // 根宽度跨越 @media 阈值时由下一轮根构建重新评估条件层。
                        self.request_reconcile_if_media_crosses(*width);
                        let resized_root = if let Some(root_mut) = self.get_mut(root) {
                            root_mut.set_frame(Rect::new(0.0, 0.0, *width, *height));
                            true
                        } else {
                            false
                        };
                        if resized_root {
                            self.push_layout_invalidation(root);
                            self.invalidate_paint(root);
                            // 递增 tree_version 使 LayerTree 重建
                            // LayerTree 缓存了 ClipRect（如 ScrollView 的裁剪矩形），
                            // 不重建则子节点位置更新了但裁剪区还是旧尺寸 → 内容被裁剪。
                            self.tree_version += 1;
                        }
                    }
                    self.dispatch_to(root, event)
                } else {
                    EventResult::NotHandled
                }
            }
        }
    }
}
// Agent 合成指针路径专项测试（树级同步驱动，不经过平台输入）。
#[cfg(test)]
#[path = "../../../../../tests-src/ui/widget_runtime/widget/tree_events/events_tests.rs"]
mod agent_pointer_tests;
