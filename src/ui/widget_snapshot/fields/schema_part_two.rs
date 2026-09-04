// 第二段保存布局、导航、数据与可选能力的快照变体。
macro_rules! snapshot_fields_part_two {
    // 接收第一段变体并追加本段内容。
    ($finish:ident; $($variants:tt)*) => {
        // 由根模块的唯一生成宏定义公开类型。
        $finish! {
            // 先保留第一段的原始变体顺序。
            $($variants)*
            /// 页面布局外壳的背景与直接子区域排列方向快照。
            Layout {
                /// 布局外壳显式声明的背景颜色。
                bg_color: Option<Color>,
                /// 会改变直接子区域排列的主轴方向。
                direction: FlexDirection,
            },
            /// 页面头部区域的高度与背景快照。
            Header {
                /// 头部区域声明的高度。
                height: f32,
                /// 头部区域显式声明的背景颜色。
                bg_color: Option<Color>,
            },
            /// 页面侧栏的宽度、背景与折叠状态快照。
            Sider {
                /// 侧栏展开时声明的宽度。
                width: f32,
                /// 侧栏显式声明的背景颜色。
                bg_color: Option<Color>,
                /// 侧栏是否允许折叠。
                collapsible: bool,
                /// 侧栏当前是否折叠。
                collapsed: bool,
                /// 侧栏折叠后的宽度。
                collapsed_width: f32,
            },
            /// 页面内容区域的背景快照。
            Content {
                /// 内容区域显式声明的背景颜色。
                bg_color: Option<Color>,
            },
            /// 页面底部区域的高度与背景快照。
            Footer {
                /// 底部区域声明的高度。
                height: f32,
                /// 底部区域显式声明的背景颜色。
                bg_color: Option<Color>,
            },
            /// 分割面板的方向、约束、比例与拖动状态快照。
            Splitter {
                /// 面板是否沿纵向分割。
                vertical: bool,
                /// 当前登记的面板数量。
                panel_count: usize,
                /// 与面板顺序对应的最小尺寸。
                min_sizes: Vec<f32>,
                /// 分割拖动手柄的命中尺寸。
                handle_size: f32,
                /// 与面板顺序对应的当前尺寸比例。
                ratios: Vec<f32>,
                /// 当前活动拖动手柄的索引。
                active_handle: usize,
            },
            /// 固钉组件的顶部偏移、滚动位置与吸附状态快照。
            Affix {
                /// 触发吸附时相对视口顶部的偏移。
                offset_top: f32,
                /// 当前纵向滚动位置。
                scroll_y: f32,
                /// 内容当前是否处于吸附状态。
                affixed: bool,
            },
            /// 返回顶部按钮的显示阈值与可见状态快照。
            BackTop {
                /// 按钮开始显示的纵向滚动阈值。
                visibility_height: f32,
                /// 返回顶部按钮当前是否可见。
                visible: bool,
            },
            // 导航 capability 关闭时同步收缩面包屑快照变体。
            #[cfg(feature = "navigation")]
            /// 面包屑的层级条目与分隔符快照。
            Breadcrumb {
                /// 按导航层级顺序保存的面包屑条目。
                items: Vec<BreadcrumbItem>,
                /// 相邻面包屑条目之间显示的分隔符。
                separator: String,
            },
            // 导航 capability 关闭时同步收缩分页快照变体。
            #[cfg(feature = "navigation")]
            /// 分页器的总量、页码、尺寸与每页条数配置快照。
            Pagination {
                /// 数据集合的总记录数。
                total: usize,
                /// 当前每页显示的记录数。
                page_size: usize,
                /// 当前一基页码。
                current: usize,
                /// 是否显示每页条数切换入口。
                show_size_changer: bool,
                /// 是否显示总记录数与当前范围。
                show_total: bool,
                /// 单个页码控件的方形边长。
                size: f32,
                /// 按循环顺序保存的每页条数候选项。
                page_size_options: Vec<usize>,
            },
            // 导航 capability 关闭时同步收缩锚点快照变体。
            #[cfg(feature = "navigation")]
            /// 锚点导航的条目、活动项、顶部偏移与背景快照。
            Anchor {
                /// 按文档顺序保存的锚点条目。
                items: Vec<AnchorItem>,
                /// 当前活动锚点的索引。
                active_index: usize,
                /// 定位目标时保留的顶部偏移。
                offset_top: f32,
                /// 锚点导航显式声明的背景颜色。
                bg_color: Option<Color>,
            },
            // 导航 capability 关闭时同步收缩菜单快照变体。
            #[cfg(feature = "navigation")]
            /// 菜单的条目、活动键、布局模式与行高快照。
            Menu {
                /// 按声明层级保存的菜单条目。
                items: Vec<MenuItem>,
                /// 当前活动菜单项的稳定键。
                active_key: String,
                /// 菜单采用的水平或垂直布局模式。
                mode: MenuMode,
                /// 菜单项声明的行高。
                item_h: f32,
            },
            // 导航 capability 关闭时同步收缩下拉菜单快照变体。
            #[cfg(feature = "navigation")]
            /// 下拉菜单的标签、稳定选项与打开、高亮、选择状态快照。
            Dropdown {
                /// 下拉菜单触发器显示的标签。
                label: String,
                /// 保留完整带键选项以观察稳定身份与展示文字。
                items: Vec<DropdownItem>,
                /// 下拉菜单当前是否打开。
                open: bool,
                /// 当前选中选项的索引。
                selected_index: Option<usize>,
                /// 当前键盘高亮选项的索引。
                highlighted_index: Option<usize>,
            },
            // 导航 capability 关闭时同步收缩菜单栏快照变体。
            #[cfg(feature = "navigation")]
            /// 菜单栏的顶级菜单、打开索引与高亮条目快照。
            MenuBar {
                /// 按声明顺序保存的顶级菜单。
                menus: Vec<MenuBarMenu>,
                /// 当前展开（含退出动画中）的顶级菜单索引。
                open_index: Option<usize>,
                /// 弹层内键盘高亮条目索引。
                highlighted_index: Option<usize>,
            },
            // 导航 capability 关闭时同步收缩标签页快照变体。
            #[cfg(feature = "navigation")]
            /// 标签页的条目、活动身份、方位与尺寸快照。
            Tabs {
                /// 按声明顺序保存的标签页条目。
                tabs: Vec<Tab>,
                /// 当前活动标签页的索引。
                active_index: usize,
                /// 当前活动标签页的可选稳定键。
                active_key: Option<String>,
                /// 标签栏相对内容区域的方位。
                position: TabPosition,
                /// 标签栏声明的高度。
                tab_height: f32,
                /// 标签页组件显式声明的固定宽度。
                fixed_width: Option<f32>,
                /// 标签页组件显式声明的固定高度。
                fixed_height: Option<f32>,
            },
            // 导航 capability 关闭时同步收缩步骤条快照变体。
            #[cfg(feature = "navigation")]
            /// 步骤条的条目、当前步骤与排列方向快照。
            Steps {
                /// 按流程顺序保存的步骤条目。
                steps: Vec<Step>,
                /// 当前步骤的索引。
                current: usize,
                /// 步骤条采用的排列方向。
                direction: StepsDirection,
            },
            // 导航 capability 关闭时同步收缩导航项快照变体。
            #[cfg(feature = "navigation")]
            /// 单个导航项的内容、身份、尺寸、索引与活动状态快照。
            NavItem {
                /// 导航项显示的文字。
                label: String,
                /// 导航项的稳定业务键。
                key: String,
                /// 导航项声明的图标名称。
                icon: String,
                /// 导航项声明的固定宽度。
                fixed_width: f32,
                /// 导航项声明的固定高度。
                fixed_height: f32,
                /// 导航项在所属导航栏中的索引。
                index: usize,
                /// 导航项是否使用紧凑布局。
                compact: bool,
                /// 导航项当前是否活动。
                active: bool,
            },
            // Navigation 外壳只快照调用方元数据与整栏折叠事实。
            #[cfg(feature = "navigation")]
            /// 导航外壳的标题、版本与整栏折叠状态快照。
            Navigation {
                /// 导航外壳显示的标题。
                title: String,
                /// 导航外壳可选的版本文字。
                version: Option<String>,
                /// 整个导航栏当前是否折叠。
                collapsed: bool,
            },
            // 树组件 capability 关闭时不保留展示树快照变体。
            #[cfg(feature = "tree-widgets")]
            /// 展示树的节点、选择集合、展开集合与多选模式快照。
            Tree {
                /// 按层级保存的树节点快照。
                nodes: Vec<SnapshotTreeNode>,
                /// 单选兼容模式下当前选中节点的稳定键。
                selected_key: String,
                /// 当前所有选中节点的稳定键集合。
                selected_keys: Vec<String>,
                /// 当前所有展开节点的稳定键集合。
                expanded_keys: Vec<String>,
                /// 树是否允许同时选择多个节点。
                multiple: bool,
            },
            // 终端 capability 关闭时不保留终端快照变体。
            #[cfg(feature = "terminal")]
            /// 终端的提示符、输出行、当前输入与历史命令快照。
            Terminal {
                /// 当前显示在输入行前方的提示符文本。
                prompt: String,
                /// 按声明顺序保存的输出行纯文本。
                lines: Vec<String>,
                /// 输入行当前草稿文本。
                input: String,
                /// 按提交顺序保存的历史命令。
                history: Vec<String>,
            },
            /// 列表的首尾内容、边框、尺寸、条目与加载入口快照。
            List {
                /// 列表头部显示的文字。
                header: String,
                /// 列表底部显示的文字。
                footer: String,
                /// 列表是否绘制边框。
                bordered: bool,
                /// 列表采用的标准尺寸档位。
                list_size: ControlSize,
                /// 按声明顺序保存的列表条目文字。
                items: Vec<String>,
                /// 列表加载更多入口显示的文字。
                load_more_text: String,
                /// 页首是否由真实声明子树负责。
                header_view: bool,
                /// 页尾是否由真实声明子树负责。
                footer_view: bool,
                /// 加载入口是否由真实声明子树负责。
                load_more_view: bool,
            },
            /// 折叠面板的条目、手风琴模式与焦点状态快照。
            Collapse {
                /// 按声明顺序保存的折叠面板快照。
                panels: Vec<SnapshotCollapsePanel>,
                /// 是否限制为最多展开一个面板。
                accordion: bool,
                /// 当前获得键盘焦点的面板标题索引。
                focused_header: usize,
            },
            /// 走马灯的控制器、尺寸、当前页与页数快照。
            Carousel {
                /// 是否显示页码指示点。
                show_dots: bool,
                /// 是否显示前后翻页箭头。
                show_arrows: bool,
                /// 走马灯显式声明的固定宽度。
                fixed_width: Option<f32>,
                /// 走马灯显式声明的固定高度。
                fixed_height: Option<f32>,
                /// 当前幻灯片索引。
                current: usize,
                /// 当前登记的幻灯片数量。
                slide_count: usize,
            },
            /// 下拉选择器的选项、选择、打开、能力与搜索状态快照。
            Select {
                /// 按展示顺序保存的扁平选项文字。
                options: Vec<String>,
                /// 按声明顺序保存的选项分组。
                optgroups: Vec<OptGroup>,
                /// 单选模式下当前选中选项的索引。
                selected: usize,
                /// 多选模式下当前选中选项的索引集合。
                selected_multi: Vec<usize>,
                /// 选项弹层当前是否打开。
                open: bool,
                /// 选择器是否禁止交互。
                disabled: bool,
                /// 选择器是否处于异步加载状态。
                loading: bool,
                /// 尚未选择时显示的占位文字。
                placeholder: String,
                /// 选择器是否允许多选。
                multiple: bool,
                /// 选择器是否启用搜索输入。
                search: bool,
                /// 当前搜索查询文字。
                search_query: String,
                /// 选择器是否使用自定义选项视图。
                custom_options: bool,
            },
            /// 自动完成输入的占位、候选项、当前值与展开状态快照。
            AutoComplete {
                /// 输入为空时显示的占位文字。
                placeholder: String,
                /// 当前可用的自动完成候选项。
                options: Vec<String>,
                /// 输入框当前文字值。
                value: String,
                /// 候选项弹层当前是否打开。
                open: bool,
            },
            // 树组件 capability 关闭时不保留树选择器快照变体。
            #[cfg(feature = "tree-widgets")]
            /// 树选择器的占位、节点、显示值、稳定键与展开状态快照。
            TreeSelect {
                /// 尚未选择时显示的占位文字。
                placeholder: String,
                /// 按层级保存的可选树节点快照。
                nodes: Vec<SnapshotTreeNode>,
                /// 当前选中节点向用户显示的文字。
                value: String,
                /// 当前选中节点的稳定键。
                value_key: String,
                /// 树选择弹层当前是否打开。
                open: bool,
            },
            /// 级联选择器的层级选项、选择路径、加载与搜索状态快照。
            Cascader {
                /// 按层级保存的级联选项。
                options: Vec<CascaderOption>,
                /// 尚未选择时显示的占位文字。
                placeholder: String,
                /// 当前选择路径的显示标签。
                selected_labels: Vec<String>,
                /// 当前选择路径的稳定值。
                selected_values: Vec<String>,
                /// 级联选项弹层当前是否打开。
                open: bool,
                /// 正在异步加载子项的选项稳定值。
                loading_children: Vec<String>,
                /// 级联选择器是否允许搜索。
                searchable: bool,
                /// 当前搜索查询文字。
                search_query: String,
                /// 与当前查询匹配的级联路径结果。
                search_results: Vec<CascaderValue>,
            },
            /// 颜色选择器的当前颜色、预设集合与展开状态快照。
            ColorPicker {
                /// 当前选中的颜色。
                value: Color,
                /// 按声明顺序保存的预设颜色。
                preset_colors: Vec<Color>,
                /// 颜色选择弹层当前是否打开。
                open: bool,
            },
            /// 日期选择器的占位、格式化值与展开状态快照。
            DatePicker {
                /// 尚未选择日期时显示的占位文字。
                placeholder: String,
                /// 当前选中日期的可选格式化值。
                value: Option<String>,
                /// 日期面板当前是否打开。
                open: bool,
            },
            /// 日期范围选择器的占位、起止值与展开状态快照。
            DateRangePicker {
                /// 尚未选择范围时显示的占位文字。
                placeholder: String,
                /// 当前范围起始日期的可选格式化值。
                start: Option<String>,
                /// 当前范围结束日期的可选格式化值。
                end: Option<String>,
                /// 日期范围面板当前是否打开。
                open: bool,
            },
            /// 时间选择器的占位、格式化值与展开状态快照。
            TimePicker {
                /// 尚未选择时间时显示的占位文字。
                placeholder: String,
                /// 当前选中时间的可选格式化值。
                value: Option<String>,
                /// 时间选择面板当前是否打开。
                open: bool,
            },
            /// 提及输入的占位、候选项、文字值与建议状态快照。
            Mentions {
                /// 输入为空时显示的占位文字。
                placeholder: String,
                /// 当前可用的提及候选项。
                options: Vec<String>,
                /// 输入框当前完整文字值。
                value: String,
                /// 提及建议列表当前是否显示。
                suggesting: bool,
            },
            /// 分段控件的选项、选择、整体禁用与逐项禁用快照。
            Segmented {
                /// 按声明顺序保存的分段选项文字。
                options: Vec<String>,
                /// 当前选中分段的索引。
                selected: usize,
                /// 整个分段控件是否禁止交互。
                disabled: bool,
                /// 与选项顺序对应的逐项禁用标志。
                disabled_options: Vec<bool>,
            },
            /// 表单项的标签、字段名、校验提示与布局快照。
            FormItem {
                /// 表单项显示的标签文字。
                label: String,
                /// 表单模型中的稳定字段名。
                name: String,
                /// 表单项是否要求提供值。
                required: bool,
                /// 表单项显示的帮助或校验文字。
                help: String,
                /// 表单项标签区域的声明宽度。
                label_width: f32,
                /// 表单项采用的标签与控件布局方式。
                layout: FormLayout,
            },
            /// 表单容器的标签宽度、项间距与布局方式快照。
            Form {
                /// 表单默认标签区域宽度。
                label_width: f32,
                /// 相邻表单项之间的间距。
                gap: f32,
                /// 表单采用的标签与控件布局方式。
                layout: FormLayout,
            },
            /// 描述列表的标题、条目、边框、列数与尺寸快照。
            Descriptions {
                /// 描述列表显示的标题。
                title: String,
                /// 按声明顺序保存的描述条目。
                items: Vec<DescriptionsItem>,
                /// 描述列表是否绘制单元格边框。
                bordered: bool,
                /// 每行期望展示的描述列数。
                column: usize,
                /// 描述标签区域的声明宽度。
                label_width: f32,
                /// 描述列表采用的标准尺寸档位。
                descriptions_size: ControlSize,
            },
            /// 结果页的语义类型、标题、副标题与额外说明快照。
            Result {
                /// 结果页采用的语义结果类型。
                result_type: ResultType,
                /// 结果页主要标题。
                title: String,
                /// 结果页补充说明文字。
                subtitle: String,
                /// 结果页额外区域的文字内容。
                extra_text: String,
            },
            // 表格 capability 关闭时同步收缩公开快照枚举。
            #[cfg(feature = "table")]
            /// 表格的列、数据、布局、分页、选择与虚拟滚动状态快照。
            Table {
                /// 按展示顺序保存的表格列配置快照。
                columns: Vec<SnapshotTableColumn>,
                /// 表头中连续列范围的分组快照。
                column_groups: Vec<SnapshotTableColumnGroup>,
                /// 表格当前持有的行数据。
                rows: Vec<TableRow>,
                /// 与行顺序对应的稳定业务键。
                row_keys: Vec<String>,
                /// 当前视图列到原始列的索引映射。
                view_columns: Vec<usize>,
                /// 数据行声明的高度。
                row_h: f32,
                /// 表头声明的高度。
                header_h: f32,
                /// 表格行是否允许展开详情。
                expandable: bool,
                /// 展开详情区域声明的高度。
                expand_height: f32,
                /// 表格是否允许列排序。
                sortable: bool,
                /// 表格是否启用行选择能力。
                selection: bool,
                /// 表格是否绘制单元格边框。
                bordered: bool,
                /// 表格是否处于加载状态。
                loading: bool,
                /// 当前单选行的索引。
                selected_row: Option<usize>,
                /// 当前所有勾选行的索引。
                checked_rows: Vec<usize>,
                /// 没有数据时显示的文字。
                empty_text: String,
                /// 当前可选的一基分页页码。
                current_page: Option<usize>,
                /// 分页数据的可选总记录数。
                total: Option<usize>,
                /// 分页模式下的每页记录数。
                page_size: usize,
                /// 表格是否启用有界虚拟滚动。
                virtual_scroll: bool,
            },
            /// 可选择列表的条目、活动索引、首尾文字与行高快照。
            SelectableList {
                /// 按声明顺序保存的可选择条目。
                items: Vec<SelectableItem>,
                /// 非受控兼容模式下的当前活动索引。
                active_index: usize,
                /// 列表头部动作按钮显示的文字。
                header_button_text: String,
                /// 列表底部显示的辅助文字。
                footer_text: String,
                /// 每个可选择条目的行高。
                item_height: f32,
            },
            /// 滚动视图的方向、尺寸、弹性、滚动条与偏移快照。
            ScrollView {
                /// 滚动视图允许滚动的轴向。
                direction: ScrollDirection,
                /// 滚动视图显式声明的固定宽度。
                fixed_width: Option<f32>,
                /// 滚动视图显式声明的固定高度。
                fixed_height: Option<f32>,
                /// 滚动视图参与父级弹性增长的权重。
                flex_grow: f32,
                /// 滚动视图参与父级弹性收缩的权重。
                flex_shrink: f32,
                /// 是否显示可见滚动条。
                show_scrollbar: bool,
                /// 当前横向滚动偏移。
                scroll_x: f32,
                /// 当前纵向滚动偏移。
                scroll_y: f32,
            },
            // 图表 capability 启用时才保留柱状图快照变体。
            #[cfg(feature = "charts")]
            /// 柱状图的数据、尺寸、量程、标签与圆角快照。
            BarChart {
                /// 按绘制顺序保存的柱状图数据。
                data: Vec<BarData>,
                /// 柱状图声明的固定宽度。
                fixed_width: f32,
                /// 柱状图声明的固定高度。
                fixed_height: f32,
                /// 柱状图纵轴采用的最大值。
                max_value: f32,
                /// 是否在柱体附近显示数值。
                show_value: bool,
                /// 柱体顶部使用的圆角半径。
                bar_radius: f32,
            },
            // 图表 capability 启用时才保留折线图快照变体。
            #[cfg(feature = "charts")]
            /// 折线图的数据、尺寸、量程、网格与线点样式快照。
            LineChart {
                /// 按绘制顺序保存的折线图数据。
                data: Vec<LineData>,
                /// 折线图声明的固定宽度。
                fixed_width: f32,
                /// 折线图声明的固定高度。
                fixed_height: f32,
                /// 折线显式声明的颜色。
                line_color: Option<Color>,
                /// 折线图纵轴采用的最大值。
                max_value: f32,
                /// 是否根据数据自动计算纵轴最小值。
                auto_min: bool,
                /// 是否绘制背景网格线。
                show_grid: bool,
                /// 是否绘制数据点标记。
                show_dots: bool,
                /// 折线绘制宽度。
                line_width: f32,
                /// 数据点标记半径。
                dot_radius: f32,
            },
            // 图表 capability 启用时才保留饼图快照变体。
            #[cfg(feature = "charts")]
            /// 饼图的数据、固定尺寸与中心孔半径快照。
            PieChart {
                /// 按绘制顺序保存的饼图数据。
                data: Vec<PieData>,
                /// 饼图声明的方形边长。
                fixed_size: f32,
                /// 环形饼图中心孔的半径。
                hole_radius: f32,
            },
            // 图表 capability 启用时才保留高级图表快照变体。
            #[cfg(feature = "charts")]
            /// 尚未专用实现的高级图表标题、说明与类型摘要快照。
            ChartPlaceholder {
                /// 高级图表显示的标题。
                title: String,
                /// 高级图表显示的补充说明。
                subtitle: String,
                /// 高级图表类型的稳定静态名称。
                kind_name: &'static str,
            },
            // 关闭二维码 capability 时同步收缩公开快照枚举。
            #[cfg(feature = "qrcode")]
            /// 二维码的源值、尺寸、纠错级别、矩阵与编码错误快照。
            QRCode {
                /// 二维码编码的原始文字值。
                value: String,
                /// 二维码声明的方形边长。
                size: f32,
                /// 二维码采用的纠错级别编码。
                error_level: u8,
                /// 编码完成后的二维码模块边长。
                module_count: usize,
                /// 无法编码时保留的可选稳定错误说明。
                encoding_error: Option<String>,
            },
            // 关闭富文本 capability 时同步收缩公开快照枚举。
            #[cfg(feature = "rich-text")]
            /// 富文本的分段、默认样式与当前链接焦点快照。
            RichText {
                /// 解析并保留交互信息的富文本分段。
                segments: Vec<RichTextSegment>,
                /// 未显式指定时采用的默认字号。
                default_font_size: f32,
                /// 默认字号使用的可选物理单位。
                default_font_size_unit: Option<PhysicalUnit>,
                /// 未显式指定时采用的默认文字颜色。
                default_color: Color,
                /// 当前获得键盘焦点的链接索引。
                focused_link: Option<usize>,
            },
            /// 主题切换器的当前明暗模式快照。
            ThemeToggle {
                /// 当前是否使用暗色主题。
                dark: bool,
            },
            /// 穿梭框源列表与目标列表的条目快照。
            Transfer {
                /// 当前位于源列表的条目。
                source: Vec<SnapshotTransferItem>,
                /// 当前位于目标列表的条目。
                target: Vec<SnapshotTransferItem>,
            },
            /// 上传组件的文件限制、交互模式、展示能力与文件队列快照。
            Upload {
                /// 文件选择器接受的类型过滤表达式。
                accept: String,
                /// 是否允许一次选择多个文件。
                multiple: bool,
                /// 是否启用拖放文件入口。
                drag: bool,
                /// 文件队列允许保留的最大数量。
                max_count: usize,
                /// 单个文件允许的可选最大字节数。
                max_size: Option<u64>,
                /// 是否显示当前上传文件列表。
                show_upload_list: bool,
                /// 图片文件是否允许打开预览。
                preview_image: bool,
                /// 是否由调用方手动启动实际上传。
                manual: bool,
                /// 按队列顺序保存的上传文件状态。
                files: Vec<UploadFile>,
            },
            /// 水印的文字、颜色、字号、透明度、旋转与平铺几何快照。
            Watermark {
                /// 水印重复绘制的文字。
                text: String,
                /// 水印文字使用的颜色。
                color: Color,
                /// 水印文字使用的字号。
                font_size: f32,
                /// 水印整体使用的透明度。
                opacity: f32,
                /// 水印文字旋转的角度。
                rotate: f32,
                /// 相邻水印单元的横向间距。
                gap_x: f32,
                /// 相邻水印单元的纵向间距。
                gap_y: f32,
                /// 水印平铺起点的横向偏移。
                x_offset: f32,
                /// 水印平铺起点的纵向偏移。
                y_offset: f32,
            },
            /// 通用容器的完整样式快照。
            Container {
                /// 容器当前使用的盒模型与视觉样式。
                style: Style,
            },
            /// 网格容器的样式、响应式断点与列声明快照。
            Grid {
                /// 网格容器当前使用的布局与视觉样式。
                style: Style,
                /// 网格可选的响应式断点配置。
                breakpoints: Option<Breakpoints>,
                /// 按声明顺序保存的网格列配置。
                cols: Vec<Col>,
            },
        }
    };
}
