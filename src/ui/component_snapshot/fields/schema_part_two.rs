// 第二段保存布局、导航、数据与可选能力的快照变体。
macro_rules! snapshot_fields_part_two {
    // 接收第一段变体并追加本段内容。
    ($finish:ident; $($variants:tt)*) => {
        // 由根模块的唯一生成宏定义公开类型。
        $finish! {
            // 先保留第一段的原始变体顺序。
            $($variants)*
            Layout {
                bg_color: Option<Color>,
                // 保存会改变直接子区域排列的主轴方向。
                direction: FlexDirection,
            },
            Header {
                height: f32,
                bg_color: Option<Color>,
            },
            Sider {
                width: f32,
                bg_color: Option<Color>,
                collapsible: bool,
                collapsed: bool,
                collapsed_width: f32,
            },
            Content {
                bg_color: Option<Color>,
            },
            Footer {
                height: f32,
                bg_color: Option<Color>,
            },
            Splitter {
                vertical: bool,
                panel_count: usize,
                min_sizes: Vec<f32>,
                handle_size: f32,
                ratios: Vec<f32>,
                active_handle: usize,
            },
            Affix {
                offset_top: f32,
                scroll_y: f32,
                affixed: bool,
            },
            BackTop {
                visibility_height: f32,
                visible: bool,
            },
            // 导航 capability 关闭时同步收缩面包屑快照变体。
            #[cfg(feature = "navigation")]
            Breadcrumb {
                items: Vec<BreadcrumbItem>,
                separator: String,
            },
            // 导航 capability 关闭时同步收缩分页快照变体。
            #[cfg(feature = "navigation")]
            Pagination {
                total: usize,
                page_size: usize,
                current: usize,
                show_size_changer: bool,
                show_total: bool,
                size: f32,
                page_size_options: Vec<usize>,
            },
            // 导航 capability 关闭时同步收缩锚点快照变体。
            #[cfg(feature = "navigation")]
            Anchor {
                items: Vec<AnchorItem>,
                active_index: usize,
                offset_top: f32,
                bg_color: Option<Color>,
            },
            // 导航 capability 关闭时同步收缩菜单快照变体。
            #[cfg(feature = "navigation")]
            Menu {
                items: Vec<MenuItem>,
                active_key: String,
                mode: MenuMode,
                item_h: f32,
            },
            // 导航 capability 关闭时同步收缩下拉菜单快照变体。
            #[cfg(feature = "navigation")]
            Dropdown {
                label: String,
                // 保留完整 keyed 选项以观察稳定身份与展示文字。
                items: Vec<DropdownItem>,
                open: bool,
                selected_index: Option<usize>,
                highlighted_index: Option<usize>,
            },
            // 导航 capability 关闭时同步收缩标签页快照变体。
            #[cfg(feature = "navigation")]
            Tabs {
                tabs: Vec<Tab>,
                active_index: usize,
                active_key: Option<String>,
                position: TabPosition,
                tab_height: f32,
                fixed_width: Option<f32>,
                fixed_height: Option<f32>,
            },
            // 导航 capability 关闭时同步收缩步骤条快照变体。
            #[cfg(feature = "navigation")]
            Steps {
                steps: Vec<Step>,
                current: usize,
                direction: StepsDirection,
            },
            // 导航 capability 关闭时同步收缩导航项快照变体。
            #[cfg(feature = "navigation")]
            NavItem {
                label: String,
                key: String,
                icon: String,
                fixed_width: f32,
                fixed_height: f32,
                index: usize,
                compact: bool,
                active: bool,
            },
            // Navigation 外壳只快照调用方元数据与整栏折叠事实。
            #[cfg(feature = "navigation")]
            Navigation {
                title: String,
                version: Option<String>,
                collapsed: bool,
            },
            // 树组件 capability 关闭时不保留展示树快照变体。
            #[cfg(feature = "tree-widgets")]
            Tree {
                nodes: Vec<SnapshotTreeNode>,
                selected_key: String,
                selected_keys: Vec<String>,
                expanded_keys: Vec<String>,
                multiple: bool,
            },
            List {
                header: String,
                footer: String,
                bordered: bool,
                list_size: ControlSize,
                items: Vec<String>,
                load_more_text: String,
            },
            Collapse {
                panels: Vec<SnapshotCollapsePanel>,
                accordion: bool,
                focused_header: usize,
            },
            Carousel {
                show_dots: bool,
                show_arrows: bool,
                fixed_width: Option<f32>,
                fixed_height: Option<f32>,
                current: usize,
                slide_count: usize,
            },
            Select {
                options: Vec<String>,
                optgroups: Vec<OptGroup>,
                selected: usize,
                selected_multi: Vec<usize>,
                open: bool,
                disabled: bool,
                loading: bool,
                placeholder: String,
                multiple: bool,
                search: bool,
                search_query: String,
                custom_options: bool,
            },
            AutoComplete {
                placeholder: String,
                options: Vec<String>,
                value: String,
                open: bool,
            },
            // 树组件 capability 关闭时不保留树选择器快照变体。
            #[cfg(feature = "tree-widgets")]
            TreeSelect {
                placeholder: String,
                nodes: Vec<SnapshotTreeNode>,
                value: String,
                value_key: String,
                open: bool,
            },
            Cascader {
                options: Vec<CascaderOption>,
                placeholder: String,
                selected_labels: Vec<String>,
                selected_values: Vec<String>,
                open: bool,
                loading_children: Vec<String>,
                searchable: bool,
                search_query: String,
                search_results: Vec<CascaderValue>,
            },
            ColorPicker {
                value: Color,
                preset_colors: Vec<Color>,
                open: bool,
            },
            DatePicker {
                placeholder: String,
                value: Option<String>,
                open: bool,
            },
            DateRangePicker {
                placeholder: String,
                start: Option<String>,
                end: Option<String>,
                open: bool,
            },
            TimePicker {
                placeholder: String,
                value: Option<String>,
                open: bool,
            },
            Mentions {
                placeholder: String,
                options: Vec<String>,
                value: String,
                suggesting: bool,
            },
            Segmented {
                options: Vec<String>,
                selected: usize,
                disabled: bool,
                disabled_options: Vec<bool>,
            },
            FormItem {
                label: String,
                name: String,
                required: bool,
                help: String,
                label_width: f32,
                layout: FormLayout,
            },
            Form {
                label_width: f32,
                gap: f32,
                layout: FormLayout,
            },
            Descriptions {
                title: String,
                items: Vec<DescriptionsItem>,
                bordered: bool,
                column: usize,
                label_width: f32,
                descriptions_size: ControlSize,
            },
            Result {
                result_type: ResultType,
                title: String,
                subtitle: String,
                extra_text: String,
            },
            // 表格 capability 关闭时同步收缩公开快照枚举。
            #[cfg(feature = "table")]
            // 启用后记录表格列、数据、布局与交互状态。
            Table {
                columns: Vec<SnapshotTableColumn>,
                column_groups: Vec<SnapshotTableColumnGroup>,
                rows: Vec<TableRow>,
                row_keys: Vec<String>,
                view_columns: Vec<usize>,
                row_h: f32,
                header_h: f32,
                expandable: bool,
                expand_height: f32,
                sortable: bool,
                selection: bool,
                bordered: bool,
                loading: bool,
                selected_row: Option<usize>,
                checked_rows: Vec<usize>,
                empty_text: String,
                current_page: Option<usize>,
                total: Option<usize>,
                page_size: usize,
                virtual_scroll: bool,
            },
            SelectableList {
                items: Vec<SelectableItem>,
                active_index: usize,
                header_button_text: String,
                footer_text: String,
                item_height: f32,
            },
            ScrollView {
                direction: ScrollDirection,
                fixed_width: Option<f32>,
                fixed_height: Option<f32>,
                flex_grow: f32,
                flex_shrink: f32,
                show_scrollbar: bool,
                scroll_x: f32,
                scroll_y: f32,
            },
            // 图表 capability 启用时才保留柱状图快照变体。
            #[cfg(feature = "charts")]
            // 启用后记录柱状图数据与布局配置。
            BarChart {
                data: Vec<BarData>,
                fixed_width: f32,
                fixed_height: f32,
                max_value: f32,
                show_value: bool,
                bar_radius: f32,
            },
            // 图表 capability 启用时才保留折线图快照变体。
            #[cfg(feature = "charts")]
            // 启用后记录折线图数据、布局与样式配置。
            LineChart {
                data: Vec<LineData>,
                fixed_width: f32,
                fixed_height: f32,
                line_color: Option<Color>,
                max_value: f32,
                auto_min: bool,
                show_grid: bool,
                show_dots: bool,
                line_width: f32,
                dot_radius: f32,
            },
            // 图表 capability 启用时才保留饼图快照变体。
            #[cfg(feature = "charts")]
            // 启用后记录饼图数据与环形布局配置。
            PieChart {
                data: Vec<PieData>,
                fixed_size: f32,
                hole_radius: f32,
            },
            // 图表 capability 启用时才保留高级图表快照变体。
            #[cfg(feature = "charts")]
            // 启用后记录高级图表的标题与类型摘要。
            ChartPlaceholder {
                title: String,
                subtitle: String,
                kind_name: &'static str,
            },
            // 关闭二维码 capability 时同步收缩公开快照枚举。
            #[cfg(feature = "qrcode")]
            // 启用后保留二维码编码与矩阵状态字段。
            QRCode {
                value: String,
                size: f32,
                error_level: u8,
                module_count: usize,
                encoding_error: Option<String>,
            },
            // 关闭富文本 capability 时同步收缩公开快照枚举。
            #[cfg(feature = "rich-text")]
            // 启用后保留富文本内容、样式与焦点状态字段。
            RichText {
                segments: Vec<RichTextSegment>,
                default_font_size: f32,
                default_font_size_unit: Option<PhysicalUnit>,
                default_color: Color,
                focused_link: Option<usize>,
            },
            ThemeToggle {
                dark: bool,
            },
            Transfer {
                source: Vec<SnapshotTransferItem>,
                target: Vec<SnapshotTransferItem>,
            },
            Upload {
                accept: String,
                multiple: bool,
                drag: bool,
                max_count: usize,
                max_size: Option<u64>,
                show_upload_list: bool,
                preview_image: bool,
                manual: bool,
                files: Vec<UploadFile>,
            },
            Watermark {
                text: String,
                color: Color,
                font_size: f32,
                opacity: f32,
                rotate: f32,
                gap_x: f32,
                gap_y: f32,
                x_offset: f32,
                y_offset: f32,
            },
            Container {
                style: Style,
            },
            Grid {
                style: Style,
                breakpoints: Option<Breakpoints>,
                cols: Vec<Col>,
            },
        }
    };
}
