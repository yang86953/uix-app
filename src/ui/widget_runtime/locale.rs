/// Locale — 组件文案国际化（全覆盖）。
#[derive(Clone, PartialEq, Eq)]
pub struct Locale {
    // ── General ──
    /// 空数据状态的主文案。
    pub empty_data: &'static str,
    /// 无数据状态的简短文案。
    pub no_data: &'static str,
    /// 通用输入占位文案。
    pub placeholder: &'static str,
    /// 通用确认按钮文案。
    pub ok_text: &'static str,
    /// 通用取消按钮文案。
    pub cancel_text: &'static str,
    /// 通用删除操作文案。
    pub delete_text: &'static str,
    /// 通用菜单无障碍文案。
    pub menu_text: &'static str,
    // ── Calendar/DatePicker ──
    /// 从周一开始的星期短名称。
    pub weekdays_short: [&'static str; 7],
    /// 从周一开始的星期完整名称。
    pub weekdays_long: [&'static str; 7],
    /// 从一月开始的月份短名称。
    pub months_short: [&'static str; 12],
    /// 从一月开始的月份完整名称。
    pub months_long: [&'static str; 12],
    /// 年份显示格式。
    pub year_format: &'static str,
    /// 月份显示格式。
    pub month_format: &'static str,
    /// 年月组合显示格式。
    pub month_year_format: &'static str,
    /// 日期显示格式。
    pub date_format: &'static str,
    /// 今天快捷操作文案。
    pub today: &'static str,
    /// 此刻快捷操作文案。
    pub now: &'static str,
    /// 选择日期提示文案。
    pub date_select: &'static str,
    /// 选择时间提示文案。
    pub time_select: &'static str,
    // ── TimePicker ──
    /// 时间显示格式。
    pub time_format: &'static str,
    // ── Breadcrumb ──
    /// 面包屑默认分隔符。
    pub breadcrumb_separator: &'static str,
    // ── Cascader ──
    /// 级联选择路径分隔符。
    pub cascader_separator: &'static str,
    /// 级联选择占位文案。
    pub cascader_placeholder: &'static str,
    // ── Drawer ──
    /// 抽屉确认按钮文案。
    pub drawer_ok: &'static str,
    // ── Empty ──
    /// 空状态说明文案。
    pub empty_description: &'static str,
    // ── FloatButton ──
    /// 浮动按钮徽标溢出文案。
    pub float_badge_overflow: &'static str,
    /// 返回顶部按钮文案。
    pub float_backtop: &'static str,
    // ── Nav ──
    /// 导航区域版本文案。
    pub nav_version: &'static str,
    // ── Pagination ──
    /// 每页条数单位文案。
    pub items_per_page: &'static str,
    /// 页码跳转提示文案。
    pub jump_to: &'static str,
    /// 页码单位文案。
    pub page: &'static str,
    /// 上一页按钮文案。
    pub prev_page: &'static str,
    /// 下一页按钮文案。
    pub next_page: &'static str,
    /// 总条数格式文案。
    pub pagination_total: &'static str,
    /// 分页每页条数格式文案。
    pub pagination_items_per_page: &'static str,
    // ── Popconfirm ──
    /// 气泡确认框确认按钮文案。
    pub popconfirm_ok: &'static str,
    /// 气泡确认框取消按钮文案。
    pub popconfirm_cancel: &'static str,
    /// 气泡确认框默认标题。
    pub popconfirm_title: &'static str,
    // ── Result ──
    /// 成功结果标题。
    pub result_success: &'static str,
    /// 错误结果标题。
    pub result_error: &'static str,
    /// 信息结果标题。
    pub result_info: &'static str,
    /// 警告结果标题。
    pub result_warning: &'static str,
    /// 未找到结果标题。
    pub result_404: &'static str,
    /// 禁止访问结果标题。
    pub result_403: &'static str,
    /// 服务器错误结果标题。
    pub result_500: &'static str,
    /// 未找到结果说明。
    pub result_404_desc: &'static str,
    /// 禁止访问结果说明。
    pub result_403_desc: &'static str,
    /// 服务器错误结果说明。
    pub result_500_desc: &'static str,
    // ── Select ──
    /// 选择器选项组格式文案。
    pub select_optgroup_format: &'static str,
    // ── Table ──
    /// 表格筛选标题。
    pub filter_title: &'static str,
    /// 表格筛选确认按钮文案。
    pub filter_confirm: &'static str,
    /// 表格筛选重置按钮文案。
    pub filter_reset: &'static str,
    // ── Timeline ──
    /// 时间线待处理状态文案。
    pub timeline_pending: &'static str,
    // ── Transfer ──
    /// 穿梭框来源列表标题。
    pub transfer_source: &'static str,
    /// 穿梭框目标列表标题。
    pub transfer_target: &'static str,
    // ── TreeSelect ──
    /// 树选择器占位文案。
    pub tree_select_placeholder: &'static str,
    // ── Upload ──
    /// 上传组件点击提示文案。
    pub upload_click: &'static str,
    /// 上传组件拖放提示文案。
    pub upload_drag: &'static str,
    // ── QRCode ──
    /// 二维码默认标志替代文案。
    pub qrcode_logo: &'static str,
}

macro_rules! zh { () => { Locale {
    empty_data: "暂无数据", no_data: "无数据", placeholder: "请选择",
    ok_text: "确定", cancel_text: "取消", delete_text: "删除", menu_text: "菜单",
    weekdays_short: ["一","二","三","四","五","六","日"],
    weekdays_long: ["星期一","星期二","星期三","星期四","星期五","星期六","星期日"],
    months_short: ["1月","2月","3月","4月","5月","6月","7月","8月","9月","10月","11月","12月"],
    months_long: ["一月","二月","三月","四月","五月","六月","七月","八月","九月","十月","十一月","十二月"],
    year_format: "{0}年", month_format: "{0}月", month_year_format: "{}年{}月", date_format: "{:04}-{:02}-{:02}",
    today: "今天", now: "此刻", date_select: "选择日期", time_select: "选择时间",
    time_format: "{:02}:{:02}", breadcrumb_separator: "/", cascader_separator: " / ",
    cascader_placeholder: "请选择",
    drawer_ok: "确定", empty_description: "暂无数据",
    float_badge_overflow: "99+", float_backtop: "回到顶部",
    nav_version: "UIX v0.0.2",
    items_per_page: "条/页", jump_to: "跳至", page: "页",
    prev_page: "上一页", next_page: "下一页",
    pagination_total: "共 {0} 条", pagination_items_per_page: "{0} 条/页",
    popconfirm_ok: "确定", popconfirm_cancel: "取消", popconfirm_title: "确认删除？",
    result_success: "操作成功", result_error: "操作失败", result_info: "提示信息", result_warning: "警告",
    result_404: "页面不存在", result_403: "无权限访问", result_500: "服务器错误",
    result_404_desc: "请检查您访问的地址是否正确", result_403_desc: "请联系管理员获取权限", result_500_desc: "请稍后重试",
    select_optgroup_format: "[{0}]",
    filter_title: "筛选", filter_confirm: "确定", filter_reset: "重置",
    timeline_pending: "...",
    transfer_source: "源", transfer_target: "目标",
    tree_select_placeholder: "请选择",
    upload_click: "点击上传", upload_drag: "点击或拖拽上传",
    qrcode_logo: "UIX",
} } }

macro_rules! en {
    () => {
        Locale {
            empty_data: "No data",
            no_data: "No data",
            placeholder: "Please select",
            ok_text: "OK",
            cancel_text: "Cancel",
            delete_text: "Delete",
            menu_text: "Menu",
            weekdays_short: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
            weekdays_long: [
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
                "Sunday",
            ],
            months_short: [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ],
            months_long: [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ],
            year_format: "{0}",
            month_format: "{0}",
            month_year_format: "{} {}",
            date_format: "{:04}-{:02}-{:02}",
            today: "Today",
            now: "Now",
            date_select: "Select date",
            time_select: "Select time",
            time_format: "{:02}:{:02}",
            breadcrumb_separator: "/",
            cascader_separator: " / ",
            cascader_placeholder: "Please select",
            drawer_ok: "OK",
            empty_description: "No data",
            float_badge_overflow: "99+",
            float_backtop: "Back to top",
            nav_version: "UIX v0.0.2",
            items_per_page: "/ page",
            jump_to: "Go to",
            page: "",
            prev_page: "Prev",
            next_page: "Next",
            pagination_total: "Total {0} items",
            pagination_items_per_page: "{0} / page",
            popconfirm_ok: "OK",
            popconfirm_cancel: "Cancel",
            popconfirm_title: "Are you sure?",
            result_success: "Success",
            result_error: "Error",
            result_info: "Info",
            result_warning: "Warning",
            result_404: "Not Found",
            result_403: "Forbidden",
            result_500: "Server Error",
            result_404_desc: "The page you visited does not exist",
            result_403_desc: "You do not have permission",
            result_500_desc: "Please try again later",
            select_optgroup_format: "[{0}]",
            filter_title: "Filter",
            filter_confirm: "OK",
            filter_reset: "Reset",
            timeline_pending: "...",
            transfer_source: "Source",
            transfer_target: "Target",
            tree_select_placeholder: "Please select",
            upload_click: "Click to upload",
            upload_drag: "Click or drag to upload",
            qrcode_logo: "UIX",
        }
    };
}

/// 中文（简体）预设。
pub fn zh_cn() -> Locale {
    zh!()
}

/// 英文预设。
pub fn en_us() -> Locale {
    en!()
}

impl Default for Locale {
    fn default() -> Self {
        zh_cn()
    }
}

/// 获取当前生效的语言配置。
pub fn use_locale() -> Locale {
    crate::ui::widget_runtime::provider_context::current_provider_context()
        .locale()
        .clone()
}

/// 在作用域内使用指定语言配置执行闭包。
pub fn with_locale<T>(locale: &Locale, f: impl FnOnce() -> T) -> T {
    crate::ui::widget_runtime::provider_context::with_widget_locale(locale, f)
}
