// 第一段保存基础控件、展示与反馈组件的快照变体。
macro_rules! snapshot_fields_part_one {
    // 把本段变体交给第二段继续累积。
    ($finish:ident) => {
        // 使用同一 token 流保持枚举变体顺序与公开形状。
        snapshot_fields_part_two! {
            // 透传最终类型生成宏。
            $finish;
            /// 组件没有提供可识别的快照字段。
            Unknown,
            /// 由公开组件宏捕获的自定义组件字段。
            Custom {
                /// 自定义组件声明的静态类型名称。
                widget: &'static str,
                /// 按组件声明顺序捕获的字段集合。
                fields: Vec<SnapshotField>,
            },
            /// 按钮的内容、状态与样式快照。
            Button {
                /// 按钮显示的文字。
                text: String,
                /// 按钮是否禁止交互。
                disabled: bool,
                /// 按钮是否填满可用横向空间。
                block: bool,
                /// 按钮是否处于加载状态。
                loading: bool,
                /// 按钮声明的图标名称。
                icon: String,
                /// 按钮在组合按钮中的边缘位置。
                group_position: Option<ButtonGroupPosition>,
                /// 按钮使用的交互状态样式集合。
                style_set: Arc<StyleSet>,
                /// 按钮当前的基础样式。
                style: Arc<Style>,
            },
            /// 原生窗口控制按钮的动作与无障碍名称快照。
            WindowControl {
                /// 窗口控制按钮触发的原生窗口动作。
                control: WindowControl,
                /// 窗口控制按钮向无障碍消费者公开的名称。
                accessible_name: String,
            },
            /// 标签的文字、字体、颜色与固定尺寸快照。
            Label {
                /// 标签显示的文字。
                text: String,
                /// 标签声明的基础字号。
                font_size: f32,
                /// 字号使用的可选物理单位。
                font_size_unit: Option<PhysicalUnit>,
                /// 标签显式声明的文字颜色。
                color: Option<Color>,
                /// 标签显式声明的固定宽度。
                fixed_width: Option<f32>,
                /// 标签显式声明的固定高度。
                fixed_height: Option<f32>,
                /// 标签可选的完整样式覆写。
                style: Option<Style>,
            },
            /// 文本输入组件的装饰、能力与校验状态快照。
            Input {
                /// 输入为空时显示的占位文字。
                placeholder: String,
                /// 输入控件采用的标准尺寸档位。
                input_size: ControlSize,
                /// 输入控件是否禁止交互。
                disabled: bool,
                /// 输入框前缀装饰文字。
                prefix: String,
                /// 输入框后缀装饰文字。
                suffix: String,
                /// 输入控件前置附加内容。
                addon_before: String,
                /// 输入控件后置附加内容。
                addon_after: String,
                /// 输入是否按密码模式隐藏内容。
                password: bool,
                /// 密码模式下内容当前是否可见。
                password_visible: bool,
                /// 输入是否提供一键清空动作。
                clearable: bool,
                /// 输入是否提供搜索提交动作。
                search: bool,
                /// 输入当前的可选校验状态。
                status: Option<InputStatus>,
                /// 与校验状态关联的提示文字。
                status_message: String,
                /// 输入是否采用多行文本区域模式。
                textarea: bool,
                /// 多行模式声明的可见行数。
                textarea_rows: usize,
                /// 输入允许的可选最大字符数。
                max_length: Option<usize>,
            },
            /// 间距容器的排列方向、间隔与尺寸策略快照。
            Space {
                /// 子项排列采用的主轴方向。
                direction: FlexDirection,
                /// 子项之间采用的标准间距档位。
                space_size: SpaceSize,
                /// 空间不足时子项是否允许换行。
                wrap: bool,
                /// 子项沿主轴的分布方式。
                justify: JustifyContent,
                /// 子项沿交叉轴的对齐方式。
                align: AlignItems,
                /// 容器显式声明的固定宽度。
                fixed_width: Option<f32>,
                /// 容器显式声明的固定高度。
                fixed_height: Option<f32>,
                /// 容器参与父级弹性分配的增长权重。
                flex_grow: f32,
            },
            /// 分隔线的文字、方向与视觉配置快照。
            Divider {
                /// 分隔线上可选的说明文字。
                text: Option<String>,
                /// 说明文字在分隔线上的位置。
                orientation: DividerOrientation,
                /// 分隔线采用水平或垂直方向。
                direction: DividerDirection,
                /// 分隔线显式声明的颜色。
                color: Option<Color>,
                /// 分隔线说明文字的字号。
                text_size: f32,
                /// 分隔线是否采用虚线样式。
                dashed: bool,
            },
            /// 图标的资源名称、显示尺寸与声明色快照。
            Icon {
                /// 图标资源的稳定名称。
                name: String,
                /// 图标声明的方形边长。
                size: f32,
                /// 图标声明的主题感知颜色身份；None 表示使用静态默认色。
                color: Option<ColorValue>,
            },
            /// 富文本排版组件的内容、修饰与颜色快照。
            Typography {
                /// 组件显示的文字内容。
                content: String,
                /// 文字采用的语义排版类型。
                type_: TypographyType,
                /// 文字交互是否禁用。
                disabled: bool,
                /// 文字是否使用标记高亮样式。
                mark: bool,
                /// 文字是否使用行内代码样式。
                code: bool,
                /// 文字是否显示下划线。
                underline: bool,
                /// 文字是否显示删除线。
                delete: bool,
                /// 文字是否使用加粗字重。
                strong: bool,
                /// 文字是否使用斜体样式。
                italic: bool,
                /// 文字是否提供复制动作。
                copyable: bool,
                /// 保存主题感知的语义文字颜色身份。
                semantic_color: Option<ColorValue>,
                /// 显式指定且优先于语义颜色的固定颜色。
                color_override: Option<Color>,
                /// 可选的有序字体族列表，保留未声明身份。
                font_family: Option<crate::ui::theme::style::FontFamily>,
                /// 可选的精确字体粗细，保留显式 normal。
                font_weight: Option<crate::ui::theme::style::FontWeight>,
                /// 可选的倍率或像素行高。
                line_height: Option<crate::ui::theme::style::LineHeight>,
                /// 可选的统一样式文本水平对齐，保留显式 left。
                text_align: Option<crate::ui::theme::style::TextAlign>,
                /// 可选的统一样式文本装饰，保留显式 none。
                text_decoration: Option<crate::ui::theme::style::TextDecoration>,
            },
            /// 复选框的选中、禁用与标签快照。
            Checkbox {
                /// 复选框当前是否选中。
                checked: bool,
                /// 复选框是否禁止交互。
                disabled: bool,
                /// 复选框关联的显示标签。
                label: String,
            },
            /// 单选组的候选项、选择状态与布局快照。
            Radio {
                /// 互斥单选组的稳定名称。
                group_name: String,
                /// 按声明顺序保存的候选项标签。
                options: Vec<String>,
                /// 当前选中候选项的索引。
                selected: usize,
                /// 整个单选组是否禁止交互。
                disabled: bool,
                /// 候选项采用的排列方向。
                direction: RadioDirection,
                /// 每个候选项声明的行高。
                item_h: f32,
            },
            /// 开关的名称、状态与尺寸快照。
            Switch {
                /// 展示名称供无障碍 name 播报；空值表示无名称开关。
                label: String,
                /// 开关当前是否开启。
                checked: bool,
                /// 开关是否禁止交互。
                disabled: bool,
                /// 开关声明的视觉尺寸。
                size: f32,
            },
            /// 单值滑块的范围、步长、标记与提示配置快照。
            Slider {
                /// 滑块允许的最小值。
                min: f64,
                /// 滑块允许的最大值。
                max: f64,
                /// 键盘或步进操作采用的增量。
                step: f64,
                /// 滑块当前值。
                value: f64,
                /// 按数值定位的标记及其显示文本。
                marks: Vec<(f64, String)>,
                /// 当前值提示气泡的可选方位。
                tooltip: Option<crate::ui::widgets::TooltipPlacement>,
            },
            /// 双端范围滑块的边界、值与活动端点快照。
            RangeSlider {
                /// 范围滑块允许的最小值。
                min: f64,
                /// 范围滑块允许的最大值。
                max: f64,
                /// 键盘或步进操作采用的增量。
                step: f64,
                /// 当前选中范围的起始值。
                start: f64,
                /// 当前选中范围的结束值。
                end: f64,
                /// 当前接收键盘操作的滑块端点。
                active_thumb: RangeSliderThumb,
                /// 范围滑块采用的标准尺寸档位。
                size: ControlSize,
            },
            /// 评分组件的数量、取值与交互配置快照。
            Rate {
                /// 可供选择的评分符号总数。
                count: usize,
                /// 当前选中的完整评分数量。
                value: usize,
                /// 是否允许选择半个评分单位。
                half: bool,
                /// 评分组件是否禁止交互。
                disabled: bool,
                /// 再次选择当前值时是否允许清空评分。
                clearable: bool,
                /// 每个评分单位显示的字符或图标文本。
                character: String,
            },
            /// 数值输入组件的范围、步长、格式化与显示状态快照。
            InputNumber {
                /// 当前数值。
                value: f64,
                /// 允许输入的最小值。
                min: f64,
                /// 允许输入的最大值。
                max: f64,
                /// 步进操作使用的增量。
                step: f64,
                /// 保存声明式小数位精度。
                precision: Option<u8>,
                /// 空值时显示的占位文字。
                placeholder: String,
                /// 数值输入是否禁止交互。
                disabled: bool,
                /// 是否允许键盘增减数值。
                keyboard: bool,
                /// 是否启用格式化显示。
                formatted: bool,
                /// 格式化后的可选显示文字。
                display_value: Option<String>,
            },
            /// 头像的文字、图片、颜色、尺寸与形状快照。
            Avatar {
                /// 图片缺失时显示的替代文字。
                text: String,
                /// 头像声明的方形边长。
                size: f32,
                /// 头像显式声明的背景颜色。
                bg_color: Option<Color>,
                /// 头像显式声明的文字颜色。
                text_color: Option<Color>,
                /// 头像是否使用方形轮廓。
                square: bool,
                /// 头像图片资源路径。
                src: String,
            },
            /// 徽标的计数、状态、位置与组合装饰快照。
            Badge {
                /// 徽标当前显示的计数值。
                count: i32,
                /// 数字徽标直接显示的最大值。
                max: i32,
                /// 徽标是否采用圆点模式。
                dot: bool,
                /// 徽标显式声明的背景颜色。
                color: Option<Color>,
                /// 前景色是否根据背景自动选择对比色。
                adaptive_foreground: bool,
                /// 徽标是否采用缎带样式。
                ribbon: bool,
                /// 徽标声明的视觉尺寸。
                size: f32,
                /// 徽标可选的语义状态样式。
                status: Option<BadgeStatus>,
                /// 计数为零时是否仍显示徽标。
                show_zero: bool,
                /// 徽标附带的说明文字。
                text: String,
                /// 徽标相对锚点的横向偏移。
                offset_x: f32,
                /// 徽标相对锚点的纵向偏移。
                offset_y: f32,
                /// 横纵偏移分别采用的可选物理单位。
                offset_unit: Option<(PhysicalUnit, PhysicalUnit)>,
                /// 标记 Badge 是否作为透明组合装饰器。
                composite: bool,
                /// 标记运行时是否已经登记唯一真实子节点。
                child_present: bool,
                /// 保存最终装饰绘制边界，不复制真实子节点快照。
                decoration_bounds: crate::core::Rect,
            },
            /// 卡片的标题、盒模型、操作区与焦点状态快照。
            Card {
                /// 卡片可选的标题文字。
                title: Option<String>,
                /// 卡片是否绘制边框。
                bordered: bool,
                /// 卡片是否启用悬停反馈。
                hoverable: bool,
                /// 卡片显式声明的固定宽度。
                fixed_width: Option<f32>,
                /// 卡片显式声明的固定高度。
                fixed_height: Option<f32>,
                /// 卡片内容区使用的内边距。
                padding: f32,
                /// 卡片阴影采用的层级。
                elevation: u8,
                /// 卡片参与父级弹性分配的增长权重。
                flex_grow: f32,
                /// 按声明顺序保存的操作文字。
                actions: Vec<String>,
                /// 当前获得键盘焦点的操作索引。
                focused_action: Option<usize>,
            },
            /// 空状态组件的说明、图标与图片资源快照。
            Empty {
                /// 空状态显示的说明文字。
                description: String,
                /// 空状态使用的图标名称。
                icon_name: String,
                /// 空状态使用的图片资源路径。
                image: String,
            },
            /// 单张图片的资源、替代内容、尺寸与预览状态快照。
            Image {
                /// 主图片资源路径。
                src: String,
                /// 图片无法感知时使用的替代文字。
                alt: String,
                /// 主图片加载失败时使用的回退资源或文字。
                fallback: String,
                /// 图片声明的宽度。
                width: f32,
                /// 图片声明的高度。
                height: f32,
                /// 图片裁剪轮廓使用的圆角半径。
                radius: f32,
                /// 图片是否允许打开预览。
                preview: bool,
                /// 图片预览当前是否打开。
                preview_open: bool,
                /// 图片是否保持宽高比适配目标矩形。
                fit: bool,
            },
            /// 图片组的资源序列、起始项与预览运行状态快照。
            ImageGroup {
                /// 按声明顺序保存的图片资源路径。
                images: Vec<String>,
                /// 图片组首次展示的图片索引。
                start_index: usize,
                /// 当前展示或预览的图片索引。
                current: usize,
                /// 图片组预览当前是否打开。
                preview_open: bool,
            },
            /// 标签的文字、颜色、能力与可见状态快照。
            Tag {
                /// 标签显示的文字。
                text: String,
                /// 标签采用的预设颜色类型。
                color: TagColor,
                /// 标签是否提供关闭动作。
                closable: bool,
                /// 标签文字使用的字号。
                font_size: f32,
                /// 标签显式声明的自定义颜色。
                custom_color: Option<Color>,
                /// 标签是否允许切换选中状态。
                checkable: bool,
                /// 可选标签当前是否选中。
                checked: bool,
                /// 标签当前是否可见。
                visible: bool,
                /// 标签声明的图标名称。
                icon: String,
            },
            /// 时间轴的条目、待完成提示与排列方向快照。
            Timeline {
                /// 按声明顺序保存的时间轴条目。
                items: Vec<TimelineItem>,
                /// 时间轴是否显示待完成节点。
                pending: bool,
                /// 时间轴是否按反向顺序呈现。
                reverse: bool,
            },
            /// 日历的尺寸、导航模式、展示月份与选择状态快照。
            Calendar {
                /// 日期格的首选方形边长。
                cell_size: f32,
                /// 标题导航是否按整年跳转。
                year_jump: bool,
                /// 当前展示的年份。
                year: i32,
                /// 当前展示的一至十二月月份编号。
                month: usize,
                /// 当前选中的完整日期。
                selected: Option<Date>,
                /// 当前接收键盘操作的日号。
                focused_day: usize,
            },
            /// 骨架屏的形状与占位尺寸快照。
            Skeleton {
                /// 骨架占位采用的形状。
                shape: SkeletonShape,
                /// 骨架占位的宽度。
                width: f32,
                /// 骨架占位的高度。
                height: f32,
            },
            /// 浮动按钮的内容、徽标、尺寸与放置策略快照。
            FloatButton {
                /// 浮动按钮使用的图标名称。
                icon: String,
                /// 保存展开说明的声明配置。
                description: String,
                /// 悬停或聚焦时显示的提示文字。
                tooltip: String,
                /// 按钮徽标当前显示的计数。
                badge_count: i32,
                /// 保存圆点徽标的声明配置。
                badge_dot: bool,
                /// 浮动按钮声明的方形边长。
                size: f32,
                /// 兼容 frame 相对放置时的横向坐标。
                x: f32,
                /// 兼容 frame 相对放置时的纵向坐标。
                y: f32,
                /// 可选窗口放置方向；`None` 保留 frame 相对兼容语义。
                placement: Option<Placement>,
                /// 浮动按钮是否在父布局中保留占位空间。
                reserve_layout_space: bool,
            },
            /// 浮动按钮组的成员数量、触发方式与展开状态快照。
            FloatButtonGroup {
                /// 按钮组登记的浮动按钮数量。
                button_count: usize,
                /// 按钮组展开使用的触发方式。
                trigger: TriggerMode,
                /// 按钮组当前是否展开。
                expanded: bool,
            },
            // 反馈 capability 关闭时同步收缩警告提示快照变体。
            #[cfg(feature = "feedback")]
            /// 警告提示的内容、级别、图标与关闭能力快照。
            Alert {
                /// 警告提示的主要消息。
                message: String,
                /// 警告提示的补充说明。
                description: String,
                /// 警告提示采用的语义状态级别。
                type_: StatusLevel,
                /// 警告提示是否允许关闭。
                closable: bool,
                /// 警告提示是否显示状态图标。
                show_icon: bool,
            },
            // 反馈 capability 关闭时同步收缩全局消息快照变体。
            #[cfg(feature = "feedback")]
            /// 全局消息队列的位置与当前内容快照。
            Message {
                /// 全局消息队列使用的浮层方位。
                placement: Placement,
                /// 按队列顺序保存的消息内容。
                contents: Vec<String>,
            },
            // 反馈 capability 关闭时同步收缩通知快照变体。
            #[cfg(feature = "feedback")]
            /// 通知队列的位置、标题与说明快照。
            Notification {
                /// 通知队列使用的浮层方位。
                placement: Placement,
                /// 按通知顺序保存的标题。
                titles: Vec<String>,
                /// 与标题顺序对应的通知说明。
                descriptions: Vec<String>,
            },
            // 反馈 capability 关闭时同步收缩进度条快照变体。
            #[cfg(feature = "feedback")]
            /// 进度条的取值、归一化证据、颜色与几何快照。
            ProgressBar {
                /// 归一化到零至一的当前进度。
                progress: f32,
                /// 进度值采用的声明输入模式。
                mode: ProgressMode,
                /// 动态输入是否经过安全归一化。
                input_normalized: bool,
                /// 动态输入发生归一化时的稳定原因。
                normalization_reason: Option<ProgressNormalizationReason>,
                /// 进度前景显式声明的颜色。
                stroke_color: Option<Color>,
                /// 进度轨道显式声明的颜色。
                track_color: Option<Color>,
                /// 线形进度条声明的高度。
                height: f32,
                /// 进度条声明的宽度。
                width: f32,
                /// 进度条端点是否使用圆角。
                round: bool,
                /// 进度条采用的线形或环形类型。
                progress_type: ProgressType,
            },
            // 反馈 capability 关闭时同步收缩加载指示器快照变体。
            #[cfg(feature = "feedback")]
            /// 加载指示器的尺寸、颜色、状态与包装模式快照。
            Spin {
                /// 加载指示器采用的标准尺寸档位。
                size: SpinSize,
                /// 加载指示器显式声明的颜色。
                color: Option<Color>,
                /// 加载指示器当前是否旋转。
                spinning: bool,
                /// 加载状态旁显示的提示文字。
                tip: String,
                /// 指示器是否作为内容包装器工作。
                wrapper_mode: bool,
            },
            // 反馈 capability 关闭时同步收缩文字提示快照变体。
            #[cfg(feature = "feedback")]
            /// 文字提示的内容、方位、触发、样式与延迟状态快照。
            Tooltip {
                /// 提示浮层显示的文字。
                text: String,
                /// 提示浮层相对锚点的方位。
                placement: TooltipPlacement,
                /// 提示浮层使用的触发方式。
                trigger: TriggerMode,
                /// 提示浮层显式声明的背景颜色。
                bg_color: Option<Color>,
                /// 提示浮层显式声明的文字颜色。
                text_color: Option<Color>,
                /// 提示浮层打开前的延迟毫秒数。
                delay_ms: u32,
                /// 当前延迟任务的运行时标识。
                timer_id: u32,
                /// 提示浮层是否显示指向箭头。
                arrow: bool,
            },
            // 反馈 capability 关闭时同步收缩气泡卡片快照变体。
            #[cfg(feature = "feedback")]
            /// 气泡卡片的标题、内容、方位、触发与可见状态快照。
            Popover {
                /// 气泡卡片显示的标题。
                title: String,
                /// 气泡卡片显示的正文内容。
                content: String,
                /// 气泡卡片相对锚点的方位。
                placement: PopoverPlacement,
                /// 气泡卡片使用的触发方式。
                trigger: PopoverTrigger,
                /// 气泡卡片是否显示指向箭头。
                arrow: bool,
                /// 气泡卡片当前是否可见。
                visible: bool,
            },
            // 反馈 capability 关闭时同步收缩气泡确认框快照变体。
            #[cfg(feature = "feedback")]
            /// 气泡确认框的完整专属配置与运行状态快照。
            Popconfirm(SnapshotPopconfirm),
            // 反馈 capability 关闭时同步收缩对话框快照变体。
            #[cfg(feature = "feedback")]
            /// 对话框的标题、尺寸、关闭策略与遮罩效果快照。
            Modal {
                /// 对话框标题。
                title: String,
                /// 对话框当前是否打开。
                open: bool,
                /// 对话框声明的宽度。
                width: f32,
                /// 对话框声明的高度。
                height: f32,
                /// 对话框采用的标准尺寸档位。
                modal_size: ControlSize,
                /// 对话框是否显示关闭动作。
                closable: bool,
                /// 点击遮罩是否允许关闭对话框。
                mask_closable: bool,
                /// 对话框是否显示底部操作区。
                footer_visible: bool,
                /// 对话框是否在可用区域居中。
                centered: bool,
                /// 对话框是否作为受控浮层呈现。
                overlay: bool,
                /// 声明式对话框请求的可选背景模糊效果。
                backdrop_blur: Option<OverlayBackdropBlur>,
            },
            // 反馈 capability 关闭时同步收缩抽屉快照变体。
            #[cfg(feature = "feedback")]
            /// 抽屉的标题、尺寸、方位、关闭策略与遮罩效果快照。
            Drawer {
                /// 抽屉标题。
                title: String,
                /// 抽屉当前是否打开。
                open: bool,
                /// 抽屉声明的宽度。
                width: f32,
                /// 抽屉声明的高度。
                height: f32,
                /// 抽屉采用的标准尺寸档位。
                drawer_size: ControlSize,
                /// 抽屉从窗口哪一侧展开。
                placement: DrawerPlacement,
                /// 抽屉是否显示关闭动作。
                closable: bool,
                /// 点击遮罩是否允许关闭抽屉。
                mask_closable: bool,
                /// 抽屉是否显示遮罩。
                mask: bool,
                /// 声明式抽屉请求的可选背景模糊效果。
                backdrop_blur: Option<OverlayBackdropBlur>,
                /// 抽屉是否显示底部操作区。
                footer_visible: bool,
                /// 抽屉标题区显示的额外文字。
                extra: String,
            },
        }
    };
}
