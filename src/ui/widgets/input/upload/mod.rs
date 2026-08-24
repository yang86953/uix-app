use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 引入调用方拥有的上传队列状态句柄。
use crate::ui::reactive::state::State;
use crate::ui::{EventResult, MouseButton, SnapshotFields, SystemEvent, WidgetTree};
// 声明同目录 UIX 视觉与公开数据契约。
mod presentation;
mod types;

// 保持 Upload 数据契约随组件公开导出。
pub use types::*;

// 引入同一组件域拥有的 UIX 视觉与格式化入口。
use presentation::*;
use std::cell::Cell;
// 引入类型化变化观察器的共享所有权句柄。
use std::rc::Rc;

// ════════════════════════════════════════════════════════════════════════════
// Upload
// ════════════════════════════════════════════════════════════════════════════

widget! {
    /// 拥有文件队列、筛选、拖放和应用侧上传状态写回的组件。
    pub struct Upload {
        accept: String,
        multiple: bool,
        file_list: Vec<UploadFile>,
        files_binding: Option<State<Vec<UploadFile>>>,
        drag: bool,
        drag_hover: bool,
        max_count: usize,
        max_size: Option<u64>,
        show_upload_list: bool,
        preview_image: bool,
        manual: bool,
        last_width: Cell<f32>,
        layout_requested: Cell<bool>,
        change_callback: Option<Rc<dyn Fn(&UploadChange)>>,
        focused: bool,
        // 记录键盘激活手势的武装键（参照 Button 激活模式）。
        activation_key: Cell<Option<crate::ui::KeyCode>>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static UploadVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        // 读取受控队列以登记声明视图的响应式依赖。
        self.capture_bound_files_dependency();
        let list_h = if self.show_upload_list {
            self.file_list.len() as f32 * self.visual.layout.file_row_height
        } else {
            0.0
        };
        constraints.clamp(Size::new(
            self.visual.layout.intrinsic_width,
            self.visual.layout.dropzone_height + list_h,
        ))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => self.remove_file_at(*pos),
            SystemEvent::FileDrop { files, .. } if self.drag => self.queue_dropped_files(files),
            // 武装激活键（参照 Button 激活模式）：Enter/Space 按下只记录手势，
            // 由 KeyUp 完成同一键盘激活，避免按住时重复触发。
            SystemEvent::KeyDown {
                key: key @ (crate::ui::KeyCode::Enter | crate::ui::KeyCode::Space),
                ..
            } => {
                self.activation_key.set(Some(*key));
                EventResult::Handled
            }
            // 完成键盘激活。设计意图：Upload 的激活语义是「请求浏览文件」；
            // 当前 UI 层没有文件对话框能力（IFileDialog 属 native 私有边界，
            // platform 门面未公开），文件来源依赖系统 FileDrop 事件，指针路径
            // 也只有列表行删除而无浏览入口，因此本分支保持与指针一致的
            // 「激活入口存在但文件选择待平台能力接入」约定，仅消费手势。
            SystemEvent::KeyUp { key, .. }
                if self.activation_key.replace(None) == Some(*key) =>
            {
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.last_width.set(frame.w.max(0.0));
        // 上传区与全部文件行在同一帧只解析一次 UIX 主题角色。
        let visual = self.visual.resolve(ctx.tokens());
        let layout = self.visual.layout;
        let root_radius = Some(Radius::uniform(visual.root_radius));
        let upload_rect = Rect::new(frame.x, frame.y, frame.w, layout.dropzone_height);
        ctx.fill_rect(upload_rect, visual.background, root_radius);

        let drag_border = if self.drag_hover {
            visual.primary
        } else {
            visual.border
        };
        ctx.stroke_rect(
            upload_rect,
            drag_border,
            if self.drag && self.drag_hover {
                self.visual.chrome.drag_border_width
            } else {
                self.visual.chrome.border_width
            },
            root_radius,
        );
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                upload_rect,
                visual.primary,
                self.visual.chrome.focus_border_width,
                root_radius,
            );
        }
        if self.drag && self.drag_hover {
            ctx.stroke_rect(
                Rect::new(
                    frame.x + layout.hover_inset,
                    frame.y + layout.hover_inset,
                    frame.w - layout.hover_width_reduction,
                    layout.hover_height,
                ),
                visual.primary,
                self.visual.chrome.hover_inner_border_width,
                Some(Radius::uniform(visual.hover_radius)),
            );
        }
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            self.visual.icons.upload,
            Rect::new(
                frame.x + frame.w * 0.5 - layout.upload_icon_half_offset,
                frame.y + layout.upload_icon_y,
                layout.upload_icon_width,
                layout.upload_icon_height,
            ),
            visual.text_quaternary,
            visual.upload_icon_size,
        );
        let loc = crate::ui::widget_runtime::locale::use_locale();
        ctx.draw_text(
            loc.upload_drag,
            Point::new(
                frame.x + frame.w * 0.5 - layout.prompt_half_offset,
                frame.y + layout.prompt_y,
            ),
            visual.text_quaternary,
            visual.typography.prompt,
        );
        if !self.accept.is_empty() && self.accept != "*" {
            let suffix = format!("{}: {}", loc.filter_title, self.accept);
            ctx.draw_text(
                &suffix,
                Point::new(
                    frame.x + frame.w * 0.5 - layout.supporting_half_offset,
                    frame.y + layout.supporting_y,
                ),
                visual.text_quaternary,
                visual.typography.supporting,
            );
        }

        if !self.show_upload_list {
            return;
        }

        for (i, f) in self.file_list.iter().enumerate() {
            let y = frame.y + layout.list_top + i as f32 * layout.file_row_height;
            let status_color = match f.status {
                UploadStatus::Error => visual.error,
                UploadStatus::Done => visual.success,
                UploadStatus::Uploading => visual.primary,
                UploadStatus::Pending => visual.text_quaternary,
            };
            let thumbnail = Rect::new(
                frame.x + layout.thumbnail_inset,
                y + layout.thumbnail_inset,
                layout.thumbnail_size,
                layout.thumbnail_size,
            );
            // 图片编解码 capability 启用时才尝试本地文件缩略图。
            #[cfg(feature = "image-codecs")]
            // 能力开启时复用原有预览加载与裁剪逻辑。
            let drew_preview = self.preview_image
                && f.source_path.as_deref().is_some_and(|path| {
                    let Some(handle) = ctx.image_service().ensure_loaded(path) else {
                        return false;
                    };
                    let device_scale = ctx.device_pixel_ratio().max(f32::EPSILON);
                    let target_side = (layout.thumbnail_size * device_scale)
                        .ceil()
                        .clamp(
                            layout.preview_min_device_side,
                            layout.preview_max_device_side,
                        ) as u32;
                    let drawable = ctx
                        .image_service()
                        .rounded_rect_sized(
                            handle,
                            target_side,
                            target_side,
                            layout.preview_radius * device_scale,
                            true,
                        )
                        .unwrap_or(handle);
                    ctx.draw_image_fill(drawable, thumbnail);
                    true
                });
            // 图片编解码 capability 关闭时上传列表只显示文件图标。
            #[cfg(not(feature = "image-codecs"))]
            // 显式消费缩略图区域以保持无默认构建零新增警告。
            let _ = thumbnail;
            // 图片编解码 capability 关闭时上传列表只显示文件图标。
            #[cfg(not(feature = "image-codecs"))]
            // 固定为未绘制预览以复用原有回退分支。
            let drew_preview = false;
            if !drew_preview {
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    self.visual.icons.file,
                    Rect::new(
                        frame.x + layout.file_icon_x,
                        y,
                        layout.file_icon_width,
                        layout.file_icon_height,
                    ),
                    visual.text_quaternary,
                    visual.file_icon_size,
                );
            }
            let text_x = frame.x
                + if drew_preview {
                    layout.preview_text_inset
                } else {
                    layout.fallback_text_inset
                };
            let file_text_clip = Rect::new(
                text_x,
                y,
                (frame.x + frame.w - layout.list_right_padding - text_x).max(0.0),
                layout.file_row_height,
            );
            ctx.push_clip(file_text_clip);
            ctx.draw_text(
                &f.name,
                Point::new(text_x, y + layout.file_name_y),
                visual.text,
                visual.typography.file_name,
            );
            ctx.draw_text(
                // 使用 presentation 边界拥有的文件大小格式化入口。
                &format_file_size(f.size),
                Point::new(text_x, y + layout.file_size_y),
                visual.text_quaternary,
                visual.typography.supporting,
            );
            if f.status == UploadStatus::Uploading {
                let bar_w =
                    (frame.x + frame.w - layout.list_right_padding - text_x).max(0.0);
                let bar_rect = Rect::new(
                    text_x,
                    y + layout.progress_y,
                    bar_w * f.progress,
                    layout.progress_height,
                );
                ctx.fill_rect(bar_rect, visual.primary, None);
            }
            ctx.pop_clip();
            let status_icon = match f.status {
                UploadStatus::Done => self.visual.icons.done,
                UploadStatus::Error => self.visual.icons.error,
                UploadStatus::Pending => self.visual.icons.pending,
                UploadStatus::Uploading => self.visual.icons.uploading,
            };
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                status_icon,
                Rect::new(
                    frame.x + frame.w - layout.status_icon_right,
                    y + layout.row_icon_y,
                    layout.row_icon_width,
                    layout.row_icon_height,
                ),
                status_color,
                visual.row_icon_size,
            );
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                self.visual.icons.remove,
                Rect::new(
                    frame.x + frame.w - layout.remove_icon_right,
                    y + layout.row_icon_y,
                    layout.row_icon_width,
                    layout.row_icon_height,
                ),
                visual.text_quaternary,
                visual.row_icon_size,
            );
        }
    }
}
impl Upload {
    /// 创建接受任意类型、单选、启用拖放且显示文件列表的上传组件。
    pub fn new() -> Self {
        Self {
            accept: "*".into(),
            multiple: false,
            file_list: Vec::new(),
            files_binding: None,
            drag: true,
            drag_hover: false,
            max_count: 10,
            max_size: None,
            show_upload_list: true,
            preview_image: false,
            manual: false,
            last_width: Cell::new(0.0),
            layout_requested: Cell::new(false),
            change_callback: None,
            focused: false,
            // 键盘激活手势尚未武装。
            activation_key: Cell::new(None),
            visual: UPLOAD_VISUAL_REF,
        }
    }
    /// 创建启用拖放区域的上传组件。
    pub fn dragger() -> Self {
        Self::new().drag(true)
    }
    /// 设置首版确定性扩展名过滤，并对动态错误返回类型化结果。
    pub fn accept(mut self, pattern: &str) -> Result<Self, UploadAcceptError> {
        // 在写入组件配置前完整验证模式。
        Self::validate_accept(pattern)?;
        // 保存已验证的原始模式供绘制与匹配使用。
        self.accept = pattern.to_string();
        // 返回成功配置的组件。
        Ok(self)
    }
    /// 验证首版 accept 扩展名语法。
    pub fn validate_accept(pattern: &str) -> Result<(), UploadAcceptError> {
        // 空模式与两种全量通配符均是合法值。
        if pattern.trim().is_empty() || matches!(pattern.trim(), "*" | "*/*") {
            // 通配配置无需继续拆分。
            return Ok(());
        }
        // 所有列表项都必须是 ASCII 扩展名模式。
        let valid = pattern.split(',').map(str::trim).all(|item| {
            // 空列表项、非 ASCII 与 MIME 分隔符均非法。
            if item.is_empty() || !item.is_ascii() || item.contains('/') {
                // 当前列表项不符合首版语法。
                return false;
            }
            // 只接受点扩展名或星号点扩展名。
            let extension = item
                // 优先移除带星号的规范前缀。
                .strip_prefix("*.")
                // 再接受不带星号的点前缀。
                .or_else(|| item.strip_prefix('.'));
            // 扩展名必须非空且不能再包含通配、空白或点。
            extension.is_some_and(|value| {
                // 首版扩展名保持单段 ASCII 文本。
                !value.is_empty()
                    // 内部星号会制造未定义模式语义。
                    && !value.contains('*')
                    // 内部点不属于单扩展名模式。
                    && !value.contains('.')
                    // 内部空白不应被静默保留。
                    && !value.chars().any(char::is_whitespace)
            })
        });
        // 合法列表直接完成验证。
        if valid {
            // 不产生额外规范化以保留调用方展示文本。
            return Ok(());
        }
        // 返回携带原始模式的类型化格式错误。
        Err(UploadAcceptError {
            // 保留完整输入供诊断。
            pattern: pattern.to_string(),
        })
    }
    /// 设置单次文件选择或拖放是否允许接收多个候选。
    pub fn multiple(mut self, v: bool) -> Self {
        self.multiple = v;
        self
    }
    /// 设置是否启用文件拖放交互。
    pub fn drag(mut self, v: bool) -> Self {
        self.drag = v;
        self
    }
    /// 绑定调用方拥有的唯一上传队列状态。
    pub fn files(mut self, state: &State<Vec<UploadFile>>) -> Self {
        // 克隆轻量状态句柄供用户交互原子写回。
        self.files_binding = Some(state.clone());
        // 构造时完整采用外部顺序、进度与结果，不按 max_count 截断。
        self.file_list = state.get();
        // 返回完成受控绑定的组件。
        self
    }
    /// 注册只读取不可变类型化事实的队列变化观察器。
    pub fn on_change<F>(mut self, callback: F) -> Self
    where
        // 处理器与组件拥有相同的静态生命周期。
        F: Fn(&UploadChange) + 'static,
    {
        // 使用共享所有权保存可跨 reconcile 复用的处理器。
        self.change_callback = Some(Rc::new(callback));
        // 返回完成观察器配置的组件。
        self
    }
    /// 设置用户新入队文件的数量上限，不截断已有或受控队列。
    pub fn max_count(mut self, n: usize) -> Self {
        self.max_count = n;
        self
    }
    /// Limit newly queued real files to at most `bytes` bytes.
    pub fn max_size(mut self, bytes: u64) -> Self {
        self.max_size = Some(bytes);
        self
    }
    /// 设置是否绘制上传文件列表，不改变队列内容。
    pub fn show_upload_list(mut self, show: bool) -> Self {
        self.show_upload_list = show;
        self
    }
    /// Render decodable real local image files as list thumbnails.
    // 图片编解码 capability 启用时才公开上传缩略图入口。
    #[cfg(feature = "image-codecs")]
    // 关闭 capability 后使用方无法请求不可用的解码行为。
    pub fn preview_image(mut self, preview: bool) -> Self {
        self.preview_image = preview;
        self
    }
    /// Require an explicit [`Upload::upload`] call to produce an application-side upload batch.
    pub fn manual(mut self, manual: bool) -> Self {
        self.manual = manual;
        self
    }
    /// 加入一个没有本地路径的兼容合成队列项。
    pub fn add_file(&mut self, name: &str) {
        // 从受控状态或非受控内部队列取得最新真值。
        let mut files = self.current_files();
        // 用户新增上限只阻止本次新项，不修改现有外部状态。
        if files.len() >= self.max_count {
            // 达到上限时保持队列与事件流不变。
            return;
        }
        // 生成与当前队列不重复的稳定身份。
        let id = Self::next_unique_id(&files);
        // 构造等待应用服务处理的合成文件。
        files.push(UploadFile::with_id(id.clone(), name, 0));
        // 原子提交一次新增事实。
        self.commit_change(
            // 传入更新后的完整队列。
            files,
            // 变化种类携带本次新增身份。
            move |files| UploadChange::Added {
                // 单项新增仍使用批量身份集合。
                ids: vec![id],
                // 保存更新后的队列快照。
                files,
            },
        );
    }
    /// 尝试加入一个真实本地文件，并返回稳定身份或类型化拒绝。
    pub fn try_add_file(&mut self, path: &str) -> Result<UploadFileId, UploadRejection> {
        // 复用批量原子入队路径保持一致语义。
        let result = self.queue_files(&[path.to_string()]);
        // 单候选成功时必有且只有一个新增身份。
        if let Some(id) = result.added_ids.into_iter().next() {
            // 返回调用方可用于进度更新的稳定身份。
            return Ok(id);
        }
        // 单候选失败时返回其确定拒绝原因。
        Err(result
            .rejected
            .into_iter()
            .next()
            .unwrap_or(UploadRejection {
                // 保留原始路径。
                path: path.to_string(),
                // 防御性回退为队列上限，正常路径不会触发。
                reason: UploadRejectReason::MaxCount,
            }))
    }
    /// 按当前 multiple 配置原子处理一批真实本地文件候选。
    pub fn queue_files(&mut self, paths: &[String]) -> UploadQueueResult {
        // 从唯一真值取得本次事务基线。
        let mut files = self.current_files();
        // 单批上限不影响未来独立操作。
        let batch_limit = if self.multiple { usize::MAX } else { 1 };
        // 收集成功项的稳定身份。
        let mut added_ids = Vec::new();
        // 收集未修改队列的拒绝结果。
        let mut rejected = Vec::new();
        // 只处理当前批次允许的候选数量。
        for path in paths.iter().take(batch_limit) {
            // 队列上限仅在用户新增时检查。
            if files.len() >= self.max_count {
                // 记录确定的上限拒绝。
                rejected.push(UploadRejection {
                    // 保存原始候选路径。
                    path: path.clone(),
                    // 标记队列已满。
                    reason: UploadRejectReason::MaxCount,
                });
                // 继续给同批剩余候选生成各自拒绝结果。
                continue;
            }
            // 扩展名不匹配不得修改暂存队列。
            if !self.accepts_file(path) {
                // 记录确定的扩展名拒绝。
                rejected.push(UploadRejection {
                    // 保存原始候选路径。
                    path: path.clone(),
                    // 标记扩展名不匹配。
                    reason: UploadRejectReason::ExtensionMismatch,
                });
                // 继续检查下一候选。
                continue;
            }
            // 文件拖放只接受可读取的普通文件。
            let Ok(metadata) = std::fs::metadata(path) else {
                // 记录不可读取拒绝。
                rejected.push(UploadRejection {
                    // 保存原始候选路径。
                    path: path.clone(),
                    // 标记元数据不可读取。
                    reason: UploadRejectReason::UnreadableFile,
                });
                // 继续检查下一候选。
                continue;
            };
            // 目录和其他特殊节点不属于上传文件。
            if !metadata.is_file() {
                // 记录非普通文件拒绝。
                rejected.push(UploadRejection {
                    // 保存原始候选路径。
                    path: path.clone(),
                    // 复用不可读取文件语义。
                    reason: UploadRejectReason::UnreadableFile,
                });
                // 继续检查下一候选。
                continue;
            }
            // 单文件大小上限在暂存前执行。
            if self
                // 读取可选上限。
                .max_size
                // 仅在配置存在且文件超限时拒绝。
                .is_some_and(|max_size| metadata.len() > max_size)
            {
                // 记录确定的大小拒绝。
                rejected.push(UploadRejection {
                    // 保存原始候选路径。
                    path: path.clone(),
                    // 标记大小超限。
                    reason: UploadRejectReason::TooLarge,
                });
                // 继续检查下一候选。
                continue;
            }
            // 为本候选生成队列内唯一身份。
            let id = Self::next_unique_id(&files);
            // 构造包含真实路径的待处理文件项。
            let file = UploadFile::with_id(
                // 保存稳定身份。
                id.clone(),
                // 文件名只用于展示。
                Self::display_name(path),
                // 保存真实字节大小。
                metadata.len(),
            )
            // 附加应用服务可读取的原始路径。
            .source_path(path.clone());
            // 暂存文件项但尚未写回 State。
            files.push(file);
            // 保存本批成功身份。
            added_ids.push(id);
        }
        // 只有至少一个成功项时才原子写回并发布事实。
        if !added_ids.is_empty() {
            // 为变化事实克隆本批身份集合。
            let change_ids = added_ids.clone();
            // 一次提交完整批次，避免观察者看到中间队列。
            self.commit_change(files, move |files| UploadChange::Added {
                // 保存本批所有稳定身份。
                ids: change_ids,
                // 保存更新后的完整快照。
                files,
            });
        }
        // 返回成功身份与逐候选拒绝原因。
        UploadQueueResult {
            // 交还本批成功身份。
            added_ids,
            // 交还不产生事件的拒绝记录。
            rejected,
        }
    }
    /// 按当前队列索引移除文件并发布稳定身份事实。
    pub fn remove_file(&mut self, index: usize) -> Option<UploadFile> {
        // 从唯一真值读取最新队列。
        let mut files = self.current_files();
        // 无效索引不得修改状态或发布事件。
        if index >= files.len() {
            // 返回没有移除项。
            return None;
        }
        // 从暂存队列移除目标项。
        let removed = files.remove(index);
        // 保存移除项的稳定身份用于事实发布。
        let id = removed.id.clone();
        // 原子提交移除后的完整队列。
        self.commit_change(files, move |files| UploadChange::Removed {
            // 单项移除仍使用批量身份集合。
            ids: vec![id],
            // 保存更新后的完整快照。
            files,
        });
        // 返回完整移除项供命令调用方使用。
        Some(removed)
    }
    /// 按稳定身份移除文件。
    pub fn remove_file_by_id(&mut self, id: &UploadFileId) -> Option<UploadFile> {
        // 在最新真值中定位身份，避免索引漂移。
        let index = self
            // 取得最新队列快照。
            .current_files()
            // 遍历队列项。
            .iter()
            // 找到首个完全相同的稳定身份。
            .position(|file| &file.id == id)?;
        // 复用单次原子移除入口。
        self.remove_file(index)
    }
    /// 原子清空队列并发布一次清空事实。
    pub fn clear_files(&mut self) {
        // 空队列不产生重复事实。
        if self.current_files().is_empty() {
            // 保持状态和观察器不变。
            return;
        }
        // 原子提交空队列。
        self.commit_change(Vec::new(), |files| UploadChange::Cleared {
            // 清空事实仍携带更新后快照。
            files,
        });
    }
    /// 应用服务按稳定身份写回有限上传进度。
    pub fn update_progress(
        // 借用可变组件以同步非受控快照。
        &mut self,
        // 使用稳定身份而非易漂移索引。
        id: &UploadFileId,
        // 接收应用服务报告的归一化进度。
        progress: f32,
    ) -> Result<(), UploadUpdateError> {
        // 非有限进度必须显式失败，不能静默归零。
        if !progress.is_finite() {
            // 返回类型化数值错误。
            return Err(UploadUpdateError::NonFiniteProgress);
        }
        // 取得最新队列快照。
        let mut files = self.current_files();
        // 按稳定身份定位目标项。
        let Some(file) = files.iter_mut().find(|file| &file.id == id) else {
            // 返回携带缺失身份的类型化错误。
            return Err(UploadUpdateError::MissingFile(id.clone()));
        };
        // 有限进度收敛到公开零到一范围。
        file.progress = progress.clamp(0.0, 1.0);
        // 进度写回表示应用已经开始传输。
        file.status = UploadStatus::Uploading;
        // 同步完整状态但不伪造队列结构变化事实。
        self.commit_status(files);
        // 报告更新成功。
        Ok(())
    }
    /// 应用服务按稳定身份写回最终结果。
    pub fn complete_file(
        // 借用可变组件以同步非受控快照。
        &mut self,
        // 使用稳定身份定位目标项。
        id: &UploadFileId,
        // true 表示完成，false 表示失败。
        success: bool,
    ) -> Result<(), UploadUpdateError> {
        // 取得最新队列快照。
        let mut files = self.current_files();
        // 按稳定身份定位目标项。
        let Some(file) = files.iter_mut().find(|file| &file.id == id) else {
            // 返回携带缺失身份的类型化错误。
            return Err(UploadUpdateError::MissingFile(id.clone()));
        };
        // 成功结果把进度固定为完整值。
        if success {
            // 完成状态必须显示完整进度。
            file.progress = 1.0;
        }
        // 根据应用结果写回最终状态。
        file.status = if success {
            // 成功进入完成状态。
            UploadStatus::Done
        } else {
            // 失败进入错误状态，重试由应用显式改回 Pending。
            UploadStatus::Error
        };
        // 同步完整状态但不伪造结构变化事实。
        self.commit_status(files);
        // 报告更新成功。
        Ok(())
    }
    /// 返回组件当前队列快照中的文件数量。
    pub fn file_count(&self) -> usize {
        self.file_list.len()
    }
    /// 借用组件当前采用的上传队列快照。
    pub fn file_list(&self) -> &[UploadFile] {
        // 受控外部更新将在 reconcile 时完整覆盖该快照。
        &self.file_list
    }

    /// Mark pending manual entries as uploading and return this call's application-side batch.
    pub fn upload(&mut self) -> Vec<UploadFile> {
        // 非手动模式不替应用启动传输。
        if !self.manual {
            // 返回空批次。
            return Vec::new();
        }
        // 从唯一真值取得最新队列。
        let mut files = self.current_files();
        // 收集本次交给应用服务的批次。
        let mut batch = Vec::new();
        // 逐项查找等待启动的文件。
        for file in &mut files {
            // 已处理项不重复启动。
            if file.status != UploadStatus::Pending {
                // 跳过非 Pending 文件。
                continue;
            }
            // 标记应用批次已进入上传中。
            file.status = UploadStatus::Uploading;
            // 每次显式启动都从零进度开始。
            file.progress = 0.0;
            // 复制不可变应用批次项。
            batch.push(file.clone());
        }
        // 只有实际批次才触发状态写回。
        if !batch.is_empty() {
            // 同步状态但不发布队列结构变化事实。
            self.commit_status(files);
        }
        // 返回本次应用侧批次。
        batch
    }

    fn remove_file_at(&mut self, pos: Point) -> EventResult {
        if !self.show_upload_list || self.file_list.is_empty() {
            return EventResult::NotHandled;
        }
        let width = self.last_width.get();
        let layout = self.visual.layout;
        if width <= 0.0
            || pos.x < (width - layout.fallback_text_inset).max(0.0)
            || pos.x > width
            || pos.y < layout.list_top
        {
            return EventResult::NotHandled;
        }
        // 命中行索引与绘制阶段的行号计算保持一致。
        let index = ((pos.y - layout.list_top) / layout.file_row_height).floor() as usize;
        let Some(removed) = self.remove_file(index) else {
            return EventResult::NotHandled;
        };
        // remove_file 已先写回唯一状态并同步发布类型化事实。
        let _ = removed;
        EventResult::Handled
    }

    fn queue_dropped_files(&mut self, files: &[String]) -> EventResult {
        // 批量入口保证至多一次 State 写回与一次类型化事件。
        let result = self.queue_files(files);
        // 全部拒绝时不发布 Change。
        if result.added_ids.is_empty() {
            // 让上层继续处理没有产生队列变化的拖放。
            return EventResult::NotHandled;
        }
        // 成功批次已经完成状态与事实提交。
        EventResult::Handled
    }

    fn accepts_file(&self, path: &str) -> bool {
        let accept = self.accept.trim();
        if accept.is_empty() || matches!(accept, "*" | "*/*") {
            return true;
        }
        let name = Self::display_name(path);
        let extension = name.rsplit_once('.').map(|(_, extension)| extension);
        accept.split(',').map(str::trim).any(|pattern| {
            // 任一通配项都接受当前文件。
            if matches!(pattern, "*" | "*/*") {
                // 立即报告匹配。
                return true;
            }
            // validate_accept 已保证这里只存在规范扩展名模式。
            let expected = pattern
                // 优先移除星号点前缀。
                .strip_prefix("*.")
                // 再移除单点前缀。
                .or_else(|| pattern.strip_prefix('.'))
                // 已验证配置必然命中一种前缀。
                .unwrap_or_default();
            // 扩展名比较忽略 ASCII 大小写。
            extension.is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
        })
    }

    // 从受控状态或非受控内部队列取得最新真值快照。
    fn current_files(&self) -> Vec<UploadFile> {
        // 受控绑定存在时必须优先读取调用方状态。
        self.files_binding
            // 借用可选状态句柄。
            .as_ref()
            // 复制当前原子快照。
            .map(State::get)
            // 非受控模式保留内部队列兼容行为。
            .unwrap_or_else(|| self.file_list.clone())
    }

    // 生成当前队列内不会重复的稳定身份。
    fn next_unique_id(files: &[UploadFile]) -> UploadFileId {
        // 重复生成直到避开调用方可能提供的同文本身份。
        loop {
            // 取得新的进程内单调身份。
            let id = UploadFileId::generated();
            // 当前队列不存在该身份时即可使用。
            if files.iter().all(|file| file.id != id) {
                // 返回队列内唯一身份。
                return id;
            }
        }
    }

    // 原子提交结构变化并在写回后同步发布不可变事实。
    fn commit_change(
        // 借用组件以更新内部快照与绑定状态。
        &mut self,
        // 接收更新后的完整队列。
        files: Vec<UploadFile>,
        // 延迟构造事实以复用最终快照。
        change: impl FnOnce(Vec<UploadFile>) -> UploadChange,
    ) {
        // 保存旧长度用于布局失效判断。
        let old_len = self.file_list.len();
        // 受控模式先执行唯一一次原子状态写回。
        if let Some(state) = self.files_binding.as_ref() {
            // 为状态事务复制最终队列。
            let committed = files.clone();
            // 使用一次 update 暴露单一完整快照。
            state.update(move |current| {
                // 原子替换调用方队列真值。
                *current = committed;
            });
        }
        // 运行时快照紧跟已提交的唯一真值。
        self.file_list = files.clone();
        // 列表长度改变时请求重新布局。
        if self.show_upload_list && old_len != self.file_list.len() {
            // 标记一次布局失效。
            self.layout_requested.set(true);
        }
        // 在状态写回完成后构造不可变变化事实。
        let change = change(files);
        // 只有登记观察器时才同步发布事实。
        if let Some(callback) = self.change_callback.as_ref() {
            // 处理器只借用本次不可变事实。
            callback(&change);
        }
    }

    // 同步进度与结果状态，但不伪造队列结构变化事实。
    fn commit_status(&mut self, files: Vec<UploadFile>) {
        // 受控模式原子覆盖同一调用方状态。
        if let Some(state) = self.files_binding.as_ref() {
            // 为状态事务复制最终队列。
            let committed = files.clone();
            // 使用一次 update 保持观察者快照原子。
            state.update(move |current| {
                // 替换完整队列状态。
                *current = committed;
            });
        }
        // 更新运行时绘制快照。
        self.file_list = files;
    }

    // 在声明视图构建期间登记受控队列依赖。
    fn capture_bound_files_dependency(&self) {
        // 只有受控组件需要捕获外部状态。
        if let Some(state) = self.files_binding.as_ref() {
            // 读取当前快照即可登记依赖。
            let _ = state.get();
        }
    }

    fn display_name(path: &str) -> &str {
        path.rsplit(['/', '\\'])
            .find(|segment| !segment.is_empty())
            .unwrap_or(path)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // 保存视觉表变化，避免声明更新后沿用旧测量。
        let visual_changed = !std::ptr::eq(self.visual, next.visual);
        // UIX 视觉表变化时采用下一声明值。
        self.visual = next.visual;
        // 保存列表可见性变化。
        let list_visibility_changed = self.show_upload_list != next.show_upload_list;
        // 保存旧队列长度。
        let old_len = self.file_list.len();
        // 声明配置始终采用下一视图值。
        self.accept = next.accept;
        // 同步单批多选配置。
        self.multiple = next.multiple;
        // 同步拖放入口配置。
        self.drag = next.drag;
        // max_count 只限制后续用户新增，不截断队列。
        self.max_count = next.max_count;
        // 同步单文件大小上限。
        self.max_size = next.max_size;
        // 同步列表可见性。
        self.show_upload_list = next.show_upload_list;
        // 同步可选预览配置。
        self.preview_image = next.preview_image;
        // 同步兼容手动批次配置。
        self.manual = next.manual;
        // 同步类型化观察器生命周期。
        self.change_callback = next.change_callback;
        // 下一视图携带绑定时完整采用外部队列快照。
        if next.files_binding.is_some() {
            // 外部顺序、进度与结果均精确覆盖运行时快照。
            self.file_list = next.file_list;
        }
        // 保存下一视图的状态句柄；解绑时保留当前队列作为非受控值。
        self.files_binding = next.files_binding;
        // 可见性或长度变化需要重新布局。
        if visual_changed || list_visibility_changed || old_len != self.file_list.len() {
            // 请求一次布局失效。
            self.layout_requested.set(true);
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Upload {
            accept: self.accept.clone(),
            multiple: self.multiple,
            drag: self.drag,
            max_count: self.max_count,
            max_size: self.max_size,
            show_upload_list: self.show_upload_list,
            preview_image: self.preview_image,
            manual: self.manual,
            files: self.file_list.clone(),
        }
    }
}
impl Default for Upload {
    fn default() -> Self {
        Self::new()
    }
}

// UIX 根把声明视觉注入 Rust 交互内核。
fn build_upload_view(mut kernel: Upload, visual: &'static UploadVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Upload {
    fn build(self) -> ViewNode {
        build_upload_view(self, UPLOAD_VISUAL_REF)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/other/misc/upload_tests.rs"]
mod tests;
