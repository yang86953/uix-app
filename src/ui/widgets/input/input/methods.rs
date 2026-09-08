use super::*;
// 光标字符索引到字节偏移的换算复用 input 层共享辅助。
use crate::ui::widgets::input::byte_index_for_char;
// 受控文本绑定的依赖捕获与「相等则不写」写回复用组件层共享原语。
use crate::ui::widgets::binding::{capture_dependency, write_if_changed};

impl Input {
    pub(super) fn intrinsic_size(&self) -> Size {
        let typography =
            crate::ui::widget_runtime::measurement::with_measurement_tokens::<Self, _>(|tokens| {
                self.resolved_typography(tokens)
            });
        let padding = self.content_padding();
        if self.textarea {
            let line_count = logical_line_count(&self.value).max(self.textarea_rows);
            let h = (line_count as f32 * typography.line_height + padding.vertical())
                .max(self.visual.layout.textarea_min_height)
                + self.status_message_height();
            Size::new(self.visual.layout.natural_width, h)
        } else {
            let layout = self.visual.layout;
            let prefix_w = if self.prefix.is_empty() {
                0.0
            } else {
                layout.prefix_width
            };
            let suffix_w = if self.suffix.is_empty() {
                0.0
            } else {
                layout.suffix_width
            };
            let clear_w = if self.clearable {
                layout.clear_width
            } else {
                0.0
            };
            let password_w = if self.password {
                layout.password_width
            } else {
                0.0
            };
            let search_w = if self.search {
                layout.search_width
            } else {
                0.0
            };
            Size::new(
                layout.natural_width
                    + addon_width(&self.addon_before, self.visual, typography.addon_font_size)
                    + addon_width(&self.addon_after, self.visual, typography.addon_font_size)
                    + prefix_w
                    + suffix_w
                    + clear_w
                    + password_w
                    + search_w,
                input_height(self.input_size).max(typography.line_height + padding.vertical())
                    + self.status_message_height(),
            )
        }
    }

    /// 创建空的单行输入框，并从当前组件配置读取尺寸、禁用和装饰默认值。
    pub fn new(placeholder: impl Into<String>) -> Self {
        let config = crate::ui::widget_runtime::config::use_config();
        let input_overrides = config.overrides.input;
        Self {
            value: String::new(),
            value_binding: None,
            placeholder: placeholder.into(),
            input_size: config.size,
            disabled: config.disabled,
            focused: false,
            hovered: false,
            composition: String::new(),
            caret_rect: Cell::new(Rect::zero()),
            cursor_char: 0,
            scroll_offset_x: Cell::new(0.0),
            scroll_line: Cell::new(0),
            // 初始化单行 shaping 字形缓存。
            glyphs: RefCell::new(Vec::new()),
            // 初始化多行 shaping 字形缓存。
            line_glyphs: RefCell::new(Vec::new()),
            selection: Cell::new(None),
            sel_anchor: Cell::new(0),
            sel_dragging: Cell::new(false),
            prefix: input_overrides.prefix.unwrap_or_default(),
            suffix: input_overrides.suffix.unwrap_or_default(),
            addon_before: String::new(),
            addon_after: String::new(),
            password: false,
            password_visible: false,
            clearable: false,
            search: false,
            status: None,
            status_message: String::new(),
            textarea: false,
            textarea_rows: 3,
            max_length: None,
            view_width: None,
            view_height: None,
            view_flex_grow: 0.0,
            view_flex_shrink: 1.0,
            view_style: None,
            text_origin: Cell::new(Point::new(12.0, 6.0)),
            painted_line_height: Cell::new(21.0),
            pending_change: RefCell::new(None),
            pending_submit: RefCell::new(None),
            clear_icon_rect: Cell::new(Rect::zero()),
            pwd_icon_rect: Cell::new(Rect::zero()),
            visual: INPUT_VISUAL_REF,
        }
    }
    /// 创建多行文本输入框。
    pub fn textarea() -> Self {
        let mut input = Self::new("");
        input.textarea = true;
        input
    }
    /// 创建密码输入框。
    pub fn password() -> Self {
        let mut input = Self::new("");
        input.password = true;
        input
    }
    /// 创建搜索输入框（带搜索图标，Enter 触发搜索）。
    pub fn search() -> Self {
        let mut input = Self::new("");
        input.search = true;
        input
    }
    /// 兼容早期预览名称；新代码使用 [`Input::search`].
    #[deprecated(note = "use Input::search()")]
    pub fn search_input() -> Self {
        Self::search()
    }
    /// 设置非受控初始值，并移除已有外部状态绑定。
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value_binding = None;
        self.replace_value(value.into());
        self
    }
    /// 将输入框绑定到外部 `State<String>`；输入与外部更新保持双向同步。
    pub fn value(mut self, state: &State<String>) -> Self {
        self.value_binding = Some(state.clone());
        self.replace_value(state.get());
        self
    }
    /// 设置输入框使用的控件尺寸档位。
    pub fn size(mut self, s: ControlSize) -> Self {
        self.input_size = s;
        self
    }
    /// 设置输入框是否拒绝编辑交互。
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    /// 接收 View DSL 的显式尺寸与 Flex 覆盖，保持 Input 的编辑状态归组件私有所有。
    pub(crate) fn apply_view_layout_style(
        &mut self,
        style: &crate::ui::theme::style::Style,
        flex_grow: Option<f32>,
        flex_shrink: Option<f32>,
    ) {
        self.view_style = Some(style.clone());
        if let Some(width) = style.width {
            self.view_width = width.is_finite().then_some(width.max(0.0));
        }
        if let Some(height) = style.height {
            self.view_height = height.is_finite().then_some(height.max(0.0));
        }
        if let Some(grow) = flex_grow {
            self.view_flex_grow = if grow.is_finite() { grow.max(0.0) } else { 0.0 };
        }
        if let Some(shrink) = flex_shrink {
            self.view_flex_shrink = if shrink.is_finite() {
                shrink.max(0.0)
            } else {
                1.0
            };
        }
    }
    /// 设置值为空时显示的占位文本。
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }
    /// 返回组件当前缓存值；controlled 用法应以绑定的 `State` 为真值来源。
    pub fn current_value(&self) -> &str {
        &self.value
    }
    /// 替换当前值、重置编辑位置，并写回已绑定的外部状态。
    pub fn set_value(&mut self, v: impl Into<String>) {
        self.replace_value(v.into());
        self.write_bound_value();
    }
    pub(super) fn replace_value(&mut self, value: String) {
        self.value = if self.textarea {
            normalize_newlines(&value).into_owned()
        } else {
            value
        };
        self.composition.clear();
        self.cursor_char = self.value.chars().count();
        self.scroll_offset_x.set(0.0);
        self.scroll_line.set(0);
        self.selection.set(None);
    }
    pub(super) fn sync_bound_value(&mut self) {
        let Some(state) = self.value_binding.as_ref() else {
            return;
        };
        let value = state.get();
        if value != self.value {
            self.replace_value(value);
        }
    }
    pub(super) fn capture_bound_value_dependency(&self) {
        capture_dependency(self.value_binding.as_ref());
    }
    pub(super) fn write_bound_value(&self) {
        write_if_changed(self.value_binding.as_ref(), self.value.clone());
    }
    pub(super) fn publish_change(&self) {
        self.write_bound_value();
        self.pending_change.replace(Some(self.value.clone()));
    }
    /// 设置输入框的聚焦状态（供 tree.build 后恢复焦点用）
    pub fn set_focused(&mut self, v: bool) {
        self.focused = v;
    }
    pub(crate) fn sync_from(&mut self, next: Self) {
        // 未绑定时保留 reconcile 前的文本、光标与焦点；controlled 模式仅在外部值
        // 真正变化时替换文本，避免无关重建打断编辑位置。
        let controlled_value = next.value_binding.as_ref().map(|_| next.value.clone());
        let textarea_changed = self.textarea != next.textarea;
        self.textarea = next.textarea;
        self.value_binding = next.value_binding;
        if let Some(value) = controlled_value {
            if value != self.value {
                self.replace_value(value);
            }
        } else if textarea_changed && self.textarea && self.value.contains('\r') {
            let normalized = normalize_newlines(&self.value).into_owned();
            self.replace_value(normalized);
        }
        self.placeholder = next.placeholder;
        self.input_size = next.input_size;
        self.disabled = next.disabled;
        self.prefix = next.prefix;
        self.suffix = next.suffix;
        self.addon_before = next.addon_before;
        self.addon_after = next.addon_after;
        let remained_password = self.password && next.password;
        self.password = next.password;
        if !remained_password {
            self.password_visible = false;
        }
        self.clearable = next.clearable;
        self.search = next.search;
        self.status = next.status;
        self.status_message = next.status_message;
        self.textarea_rows = next.textarea_rows;
        self.max_length = next.max_length;
        // 声明树重建时同步最新叶控件布局覆盖，避免旧 hug 宽度残留。
        self.view_style = next.view_style;
        self.view_width = next.view_width;
        self.view_height = next.view_height;
        self.view_flex_grow = next.view_flex_grow;
        self.view_flex_shrink = next.view_flex_shrink;
        self.visual = next.visual;
    }
    pub(crate) fn controlled_value_changed(&self, next: &Self) -> bool {
        next.value_binding.is_some() && self.value != next.value
    }
    /// 返回快照跳过的 View 布局字段是否发生变化。
    pub(crate) fn view_layout_changed(&self, next: &Self) -> bool {
        self.view_style != next.view_style
            || self.view_width != next.view_width
            || self.view_height != next.view_height
            || self.view_flex_grow != next.view_flex_grow
            || self.view_flex_shrink != next.view_flex_shrink
    }
    /// 设置输入区域内部、文本之前显示的前缀。
    pub fn prefix(mut self, s: &str) -> Self {
        self.prefix = s.to_string();
        self
    }
    /// 设置输入区域内部、文本之后显示的后缀。
    pub fn suffix(mut self, s: &str) -> Self {
        self.suffix = s.to_string();
        self
    }
    /// 设置输入框边框之前连接显示的附加文本。
    pub fn addon_before(mut self, s: &str) -> Self {
        self.addon_before = s.to_string();
        self
    }
    /// 设置输入框边框之后连接显示的附加文本。
    pub fn addon_after(mut self, s: &str) -> Self {
        self.addon_after = s.to_string();
        self
    }
    /// 设置非空输入是否显示清除按钮。
    pub fn clearable(mut self, v: bool) -> Self {
        self.clearable = v;
        self
    }
    /// 设置是否显示搜索动作并让 Enter 触发提交。
    pub fn search_enabled(mut self, v: bool) -> Self {
        self.search = v;
        self
    }
    /// 设置输入框的校验状态样式。
    pub fn status(mut self, status: InputStatus) -> Self {
        self.status = Some(status);
        self
    }
    /// 设置输入框下方显示的状态说明文本。
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.status_message = message.into();
        self
    }
    /// 清除校验状态及其说明文本。
    pub fn clear_status(mut self) -> Self {
        self.status = None;
        self.status_message.clear();
        self
    }
    /// 启用多行模式并设置至少为一的最小可见行数。
    pub fn rows(mut self, rows: usize) -> Self {
        self.textarea = true;
        self.textarea_rows = rows.max(1);
        if self.value.contains('\r') {
            let normalized = normalize_newlines(&self.value).into_owned();
            self.replace_value(normalized);
        }
        self
    }
    /// 限制后续用户输入允许包含的最大字符数。
    pub fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    fn value_with_composition(&self) -> Cow<'_, str> {
        if self.composition.is_empty() {
            return Cow::Borrowed(&self.value);
        }
        let byte_pos = byte_index_for_char(&self.value, self.cursor_char);
        let mut value = String::with_capacity(self.value.len() + self.composition.len());
        value.push_str(&self.value[..byte_pos]);
        value.push_str(&self.composition);
        value.push_str(&self.value[byte_pos..]);
        Cow::Owned(value)
    }

    fn status_message_height(&self) -> f32 {
        if self.status_message.is_empty() {
            0.0
        } else {
            self.visual.chrome.status_message_height
        }
    }

    pub(super) fn display_value_with_composition(&self) -> Cow<'_, str> {
        if !self.password || self.password_visible {
            return self.value_with_composition();
        }

        let byte_pos = byte_index_for_char(&self.value, self.cursor_char);
        let masked = |text: &str| {
            text.chars()
                .map(|ch| if ch == '\n' { '\n' } else { '\u{2022}' })
                .collect::<String>()
        };
        let mut value = String::with_capacity(self.value.len() + self.composition.len());
        value.push_str(&masked(&self.value[..byte_pos]));
        value.push_str(&self.composition);
        value.push_str(&masked(&self.value[byte_pos..]));
        Cow::Owned(value)
    }

    pub(super) fn visual_text_before_cursor(&self) -> Cow<'_, str> {
        let byte_pos = byte_index_for_char(&self.value, self.cursor_char);
        let before = &self.value[..byte_pos];
        if !self.password || self.password_visible {
            Cow::Borrowed(before)
        } else {
            Cow::Owned(
                before
                    .chars()
                    .map(|ch| if ch == '\n' { '\n' } else { '\u{2022}' })
                    .collect(),
            )
        }
    }
    // ── 内部：光标移动 ──

    pub(super) fn move_cursor_left(&mut self, ctrl: bool, extend: bool) {
        // 普通左移优先把现有选择折叠到逻辑起点。
        if !extend && self.selection.get().is_some() {
            // 读取已归一选择起点。
            let start = self.selection.get().map(|(start, _)| start).unwrap_or(0);
            // 把光标折叠到选择起点。
            self.cursor_char = start;
            // 清除折叠后的选择。
            self.selection.set(None);
            // 更新后续选择锚点。
            self.sel_anchor.set(start);
            // 折叠已经完成本次移动。
            return;
        }
        if ctrl {
            // 单次正向扫描记录光标前最后一个空格分词起点。
            let mut pos = 0usize;
            let mut in_word = false;
            for (index, ch) in self.value.chars().enumerate() {
                // 光标及其后的字符不参与向左按词定位。
                if index >= self.cursor_char {
                    break;
                }
                if ch == ' ' {
                    // 空格结束当前词，下一个非空格字符会建立新起点。
                    in_word = false;
                } else if !in_word {
                    // 保存最近一个词的逻辑字符起点。
                    pos = index;
                    in_word = true;
                }
            }
            // 按词结果向后收敛，禁止停在字素簇内部。
            self.cursor_char = TextIndexCursor::new(&self.value)
                // 归一原始字符位置。
                .normalize_char(CharIndex(pos), BoundaryBias::Backward)
                // 保存兼容字符下标。
                .0;
        } else {
            // 普通左移一次越过整个扩展字素簇。
            self.cursor_char = TextIndexCursor::new(&self.value)
                // 从当前字符位置查找前一边界。
                .previous_grapheme_boundary(CharIndex(self.cursor_char))
                // 保存兼容字符下标。
                .0;
        }
        // Shift 移动保留锚点并扩展选择。
        if extend {
            // 更新为完整字素簇选择范围。
            self.set_selection_range(self.sel_anchor.get(), self.cursor_char);
        } else {
            // 普通移动清除旧选择。
            self.selection.set(None);
            // 普通移动重置选择锚点。
            self.sel_anchor.set(self.cursor_char);
        }
    }

    pub(super) fn move_cursor_right(&mut self, ctrl: bool, extend: bool) {
        // 普通右移优先把现有选择折叠到逻辑终点。
        if !extend && self.selection.get().is_some() {
            // 读取已归一选择终点。
            let end = self.selection.get().map(|(_, end)| end).unwrap_or(0);
            // 把光标折叠到选择终点。
            self.cursor_char = end;
            // 清除折叠后的选择。
            self.selection.set(None);
            // 更新后续选择锚点。
            self.sel_anchor.set(end);
            // 折叠已经完成本次移动。
            return;
        }
        if ctrl {
            // 单次扫描跳过光标前缀，再越过空格和紧随其后的完整单词。
            let mut pos = 0usize;
            let mut in_word = false;
            for ch in self.value.chars() {
                if pos < self.cursor_char {
                    // 先把逻辑位置限制到现有文本末尾。
                    pos += 1;
                    continue;
                }
                if ch == ' ' {
                    if in_word {
                        // 已越过一个词，停在它的排他终点。
                        break;
                    }
                    // 前导空格属于本次按词移动范围。
                    pos += 1;
                } else {
                    // 越过当前词的每个非空格字符。
                    in_word = true;
                    pos += 1;
                }
            }
            // 按词结果向前收敛，禁止停在字素簇内部。
            self.cursor_char = TextIndexCursor::new(&self.value)
                // 归一原始字符位置。
                .normalize_char(CharIndex(pos), BoundaryBias::Forward)
                // 保存兼容字符下标。
                .0;
        } else {
            // 普通右移一次越过整个扩展字素簇。
            self.cursor_char = TextIndexCursor::new(&self.value)
                // 从当前字符位置查找后一边界。
                .next_grapheme_boundary(CharIndex(self.cursor_char))
                // 保存兼容字符下标。
                .0;
        }
        // Shift 移动保留锚点并扩展选择。
        if extend {
            // 更新为完整字素簇选择范围。
            self.set_selection_range(self.sel_anchor.get(), self.cursor_char);
        } else {
            // 普通移动清除旧选择。
            self.selection.set(None);
            // 普通移动重置选择锚点。
            self.sel_anchor.set(self.cursor_char);
        }
    }

    pub(super) fn move_cursor_up(&mut self) {
        let (line, col) = self.cursor_line_col();
        if line == 0 {
            return;
        }
        let mut lines = LogicalLineCursor::new(&self.value);
        lines.skip_lines(line - 1);
        let Some((_previous, start, end)) = lines.next_existing_line() else {
            return;
        };
        // 目标列不能越过上一逻辑行末尾。
        let target = start + col.min(end - start);
        // 垂直移动也必须收敛到最近扩展字素簇边界。
        self.cursor_char = TextIndexCursor::new(&self.value)
            // 归一目标字符位置。
            .normalize_char(CharIndex(target), BoundaryBias::Nearest)
            // 保存兼容字符下标。
            .0;
        self.sel_anchor.set(self.cursor_char);
        self.selection.set(None);
    }

    pub(super) fn move_cursor_down(&mut self) {
        let (line, col) = self.cursor_line_col();
        let mut lines = LogicalLineCursor::new(&self.value);
        lines.skip_lines(line + 1);
        let Some((_next, start, end)) = lines.next_existing_line() else {
            return;
        };
        // 目标列不能越过下一逻辑行末尾。
        let target = start + col.min(end - start);
        // 垂直移动也必须收敛到最近扩展字素簇边界。
        self.cursor_char = TextIndexCursor::new(&self.value)
            // 归一目标字符位置。
            .normalize_char(CharIndex(target), BoundaryBias::Nearest)
            // 保存兼容字符下标。
            .0;
        self.sel_anchor.set(self.cursor_char);
        self.selection.set(None);
    }

    /// 返回 (行号, 列号) 对应 cursor_char 的位置
    pub(super) fn cursor_line_col(&self) -> (usize, usize) {
        // 从首行开始流式定位，避免光标闪烁重绘时收集临时行数组。
        let mut remaining = self.cursor_char;
        // 保存越界光标最终收敛所需的末行位置。
        let mut last = (0usize, 0usize);
        // split 对空文本也产生一个空行，保持旧 logical_lines 语义。
        for (i, line) in self.value.split('\n').enumerate() {
            let line_len = line.chars().count();
            // 持续更新末行兜底位置。
            last = (i, line_len);
            // 每个换行符消耗 1 个字符位置（'\n'）
            if remaining <= line_len {
                return (i, remaining);
            }
            remaining -= line_len + 1; // +1 for the newline
        }
        // 旧状态中的越界位置收敛到最后一个逻辑行末尾。
        last
    }

    pub(super) fn insert_text_at_cursor(&mut self, text: &str) -> bool {
        // 多行输入先统一平台换行；没有回车时继续借用事件载荷。
        let normalized = if self.textarea {
            normalize_newlines(text)
        } else {
            Cow::Borrowed(text)
        };
        // 统一声明当前输入模式允许进入值状态的字符。
        let accepts = |ch: char| {
            if self.textarea {
                ch >= ' ' || ch == '\n' || ch == '\r'
            } else {
                !ch.is_control()
            }
        };
        // 常见的完整合法事件保持借用；仅在确实需要过滤时构造紧凑副本。
        let (insertion, insertion_chars) = match normalized {
            Cow::Borrowed(value) => {
                // 校验时同时计数字符，合法常见路径无需稍后再次扫描事件载荷。
                let mut char_count = 0usize;
                let valid = value.chars().all(|ch| {
                    let accepted = accepts(ch);
                    char_count += usize::from(accepted);
                    accepted
                });
                if valid {
                    (Cow::Borrowed(value), char_count)
                } else {
                    // 过滤后 UTF-8 字节数不会增长，按源长度一次预留即可避免渐进扩容。
                    let mut filtered = String::with_capacity(value.len());
                    char_count = 0;
                    for ch in value.chars().filter(|ch| accepts(*ch)) {
                        filtered.push(ch);
                        char_count += 1;
                    }
                    (Cow::Owned(filtered), char_count)
                }
            }
            Cow::Owned(mut value) => {
                // 换行规范化已经取得所有权时直接原地过滤，避免第二份字符串。
                value.retain(accepts);
                let char_count = value.chars().count();
                (Cow::Owned(value), char_count)
            }
        };
        if insertion.is_empty() {
            return false;
        }
        let replaced_selection = self.selection.get().is_some();
        if replaced_selection {
            self.delete_selection();
        }
        // 借用最终待插入片段，并记录它消费的逻辑字符数量。
        let insertion = insertion.as_ref();
        let (insertion, inserted_chars) = if let Some(max_length) = self.max_length {
            // 计算还能接收的 Unicode 标量数量。
            let available = max_length.saturating_sub(self.value.chars().count());
            // 最大长度截断只能发生在不晚于预算的完整字素簇边界。
            let safe_len = TextIndexCursor::new(insertion)
                // 从字符预算向后收敛。
                .normalize_char(CharIndex(available), BoundaryBias::Backward)
                // 提取可安全保留的字符数量。
                .0;
            // 一次字符到字节转换得到可直接交给 String 的完整前缀。
            let safe_byte_len = TextIndexCursor::new(insertion)
                .char_to_byte(CharIndex(safe_len))
                .0;
            (&insertion[..safe_byte_len], safe_len)
        } else {
            // 无长度限制直接复用校验或过滤阶段得到的标量数量。
            (insertion, insertion_chars)
        };
        if insertion.is_empty() {
            if replaced_selection {
                self.sel_anchor.set(self.cursor_char);
                self.publish_change();
                return true;
            }
            return false;
        }
        // 只定位一次插入点，随后让 String 一次搬移尾部并完成容量增长。
        let byte_pos = TextIndexCursor::new(&self.value)
            .char_to_byte(CharIndex(self.cursor_char))
            .0;
        self.value.insert_str(byte_pos, insertion);
        // 光标按逻辑字符而不是 UTF-8 字节推进。
        self.cursor_char += inserted_chars;
        self.sel_anchor.set(self.cursor_char);
        self.publish_change();
        true
    }

    pub(super) fn char_at_x(&self, text_x: f32) -> usize {
        // 借用真实视觉 shaping 字形簇。
        let glyphs = self.glyphs.borrow();
        // 使用共享方向感知命中逻辑取得原始字符边界。
        let raw_index = crate::draw::resources::font::text_backend::glyph_hit_test_index(
            // 传入视觉顺序字形。
            &glyphs, // 传入行内水平坐标。
            text_x,
        )
        // 空布局回退到文本起点。
        .unwrap_or(0);
        // 命中结果最终收敛到最近扩展字素簇边界。
        TextIndexCursor::new(&self.value)
            // 归一后端字符边界。
            .normalize_char(CharIndex(raw_index), BoundaryBias::Nearest)
            // 返回兼容字符下标。
            .0
    }

    /// 多行模式下根据 (x, y) 找字符索引
    pub(super) fn char_at_xy(&self, x: f32, y: f32) -> usize {
        // 顶部 padding 区映射到首行；其他位置按可见行偏移并收敛到末行。
        let requested_line = (y.max(0.0) / self.painted_line_height.get().max(1.0)) as usize;
        let requested_line = requested_line.saturating_add(self.scroll_line.get());
        let mut lines = LogicalLineCursor::new(&self.value);
        let (line_idx, _line, line_start, _end) = lines.line_at_or_last(requested_line);
        // 取得现有逐行几何给出的原始字符位置。
        let raw_index = self.x_to_char_on_line(line_idx, line_start, x);
        // 多行命中同样必须收敛到最近扩展字素簇边界。
        TextIndexCursor::new(&self.value)
            // 归一原始字符位置。
            .normalize_char(CharIndex(raw_index), BoundaryBias::Nearest)
            // 返回兼容字符下标。
            .0
    }

    /// 根据 x 坐标在该行内找字符索引
    fn x_to_char_on_line(&self, line_idx: usize, line_offset: usize, x: f32) -> usize {
        // 借用逐行真实 shaping 字形簇。
        let line_glyphs = self.line_glyphs.borrow();
        if let Some(glyphs) = line_glyphs.get(line_idx) {
            // 空视觉行命中其逻辑行起点。
            if glyphs.is_empty() {
                return line_offset;
            }
            // 使用共享方向感知 shaping cluster 命中逻辑。
            let local_index = crate::draw::resources::font::text_backend::glyph_hit_test_index(
                // 传入当前视觉行字形。
                glyphs, // 传入行内水平坐标。
                x,
            )
            // 空字形兜底到行首。
            .unwrap_or(0);
            // 把行内逻辑字符位置平移到全文字符下标。
            line_offset + local_index
        } else {
            // fallback: 无 glyph 数据时放到行首
            line_offset
        }
    }

    /// 编辑命令提交正文后恢复逻辑光标与选区；后续声明协调保持这些位置。
    pub(crate) fn restore_edit_selection(&mut self, start: usize, end: usize) {
        let index = TextIndexCursor::new(&self.value);
        let start = index
            .normalize_char(CharIndex(start), BoundaryBias::Backward)
            .0;
        let end = index
            .normalize_char(CharIndex(end), BoundaryBias::Forward)
            .0;
        self.sel_anchor.set(start);
        self.cursor_char = end;
        self.set_selection_range(start, end);
    }

    pub(crate) fn set_selection_range(&self, a: usize, b: usize) {
        // 同一逻辑位置始终表示空选择，不因旧位置非法而扩展文本。
        if a == b {
            // 清除空选择。
            self.selection.set(None);
            // 无需构造范围。
            return;
        }
        // 把无方向选择向外扩展到完整字素簇边界。
        let (start, end) = TextIndexCursor::new(&self.value)
            // 归一显式字符索引范围。
            .normalize_selection(CharIndex(a), CharIndex(b));
        // 空范围不保留选择状态。
        if start == end {
            self.selection.set(None);
        } else {
            // 保存合法字符边界组成的选择范围。
            self.selection.set(Some((start.0, end.0)));
        }
    }
    pub(super) fn slice_range(&self, start_char: usize, end_char: usize) -> String {
        // 借用文本并流式归一选择与字节端点。
        let index_cursor = TextIndexCursor::new(&self.value);
        // 防御性地把调用方范围扩展到完整字素簇边界。
        let (start, end) = index_cursor.normalize_selection(
            // 包装字符起点。
            CharIndex(start_char),
            // 包装字符终点。
            CharIndex(end_char),
        );
        // 把合法字符起点转换为字节偏移。
        let (byte_start, byte_end) = index_cursor.char_range_to_bytes(start, end);
        // 返回完整 UTF-8 字素簇片段。
        self.value[byte_start.0..byte_end.0].to_owned()
    }
    pub(super) fn delete_selection(&mut self) {
        if let Some((s, e)) = self.selection.get() {
            // 借用删除前文本并流式归一选择与字节端点。
            let index_cursor = TextIndexCursor::new(&self.value);
            // 防御性地把选择扩展到完整字素簇边界。
            let (start, end) = index_cursor.normalize_selection(CharIndex(s), CharIndex(e));
            // 单次字符遍历转换两个合法字节端点。
            let (byte_start, byte_end) = index_cursor.char_range_to_bytes(start, end);
            // 删除完整字素簇范围。
            self.value.replace_range(byte_start.0..byte_end.0, "");
            // 光标停在删除范围原起点。
            self.cursor_char = start.0;
            self.selection.set(None);
        }
    }

    // 删除光标前一个完整扩展字素簇。
    pub(super) fn delete_previous_grapheme(&mut self) -> bool {
        // 借用删除前文本，避免建立临时字素簇与字节向量。
        let index_cursor = TextIndexCursor::new(&self.value);
        // 把光标收敛到真实可停靠位置。
        let cursor = index_cursor.normalize_char(
            // 包装当前字符下标。
            CharIndex(self.cursor_char),
            // 旧状态使用最近边界修复。
            BoundaryBias::Nearest,
        );
        // 查找完整前一字素簇起点。
        let previous = index_cursor.previous_grapheme_boundary(cursor);
        // 文本起点没有可删除的前一字素簇。
        if previous == cursor {
            // 报告未发生删除。
            return false;
        }
        // 起点转换为 UTF-8 字节偏移。
        let (byte_start, byte_end) = index_cursor.char_range_to_bytes(previous, cursor);
        // 删除完整前一字素簇。
        self.value.replace_range(byte_start.0..byte_end.0, "");
        // 光标退到删除范围起点。
        self.cursor_char = previous.0;
        // 后续 Shift 选择从新光标开始。
        self.sel_anchor.set(previous.0);
        // 报告删除成功。
        true
    }

    // 删除光标后一个完整扩展字素簇。
    pub(super) fn delete_next_grapheme(&mut self) -> bool {
        // 借用删除前文本，避免建立临时字素簇与字节向量。
        let index_cursor = TextIndexCursor::new(&self.value);
        // 把光标收敛到真实可停靠位置。
        let cursor = index_cursor.normalize_char(
            // 包装当前字符下标。
            CharIndex(self.cursor_char),
            // 旧状态使用最近边界修复。
            BoundaryBias::Nearest,
        );
        // 查找完整后一字素簇终点。
        let next = index_cursor.next_grapheme_boundary(cursor);
        // 文本末尾没有可删除的后一字素簇。
        if next == cursor {
            // 报告未发生删除。
            return false;
        }
        // 起点转换为 UTF-8 字节偏移。
        let (byte_start, byte_end) = index_cursor.char_range_to_bytes(cursor, next);
        // 删除完整后一字素簇。
        self.value.replace_range(byte_start.0..byte_end.0, "");
        // 光标保持在删除范围起点。
        self.cursor_char = cursor.0;
        // 后续 Shift 选择从当前光标开始。
        self.sel_anchor.set(cursor.0);
        // 报告删除成功。
        true
    }
}
