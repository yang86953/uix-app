/// Locale — 组件文案国际化（全覆盖）。
#[derive(Clone, PartialEq, Eq)]
pub struct Locale {
    // ── General ──
    pub empty_data: &'static str,
    pub no_data: &'static str,
    pub placeholder: &'static str,
    pub ok_text: &'static str,
    pub cancel_text: &'static str,
    pub delete_text: &'static str,
    pub menu_text: &'static str,
    // ── Calendar/DatePicker ──
    pub weekdays_short: [&'static str; 7],
    pub weekdays_long: [&'static str; 7],
    pub months_short: [&'static str; 12],
    pub months_long: [&'static str; 12],
    pub year_format: &'static str,
    pub month_format: &'static str,
    pub month_year_format: &'static str,
    pub date_format: &'static str,
    pub today: &'static str,
    pub now: &'static str,
    pub date_select: &'static str,
    pub time_select: &'static str,
    // ── TimePicker ──
    pub time_format: &'static str,
    // ── Breadcrumb ──
    pub breadcrumb_separator: &'static str,
    // ── Cascader ──
    pub cascader_separator: &'static str,
    pub cascader_placeholder: &'static str,
    // ── Drawer ──
    pub drawer_ok: &'static str,
    // ── Empty ──
    pub empty_description: &'static str,
    // ── FloatButton ──
    pub float_badge_overflow: &'static str,
    pub float_backtop: &'static str,
    // ── Nav ──
    pub nav_version: &'static str,
    // ── Pagination ──
    pub items_per_page: &'static str,
    pub jump_to: &'static str,
    pub page: &'static str,
    pub prev_page: &'static str,
    pub next_page: &'static str,
    pub pagination_total: &'static str,
    pub pagination_items_per_page: &'static str,
    // ── Popconfirm ──
    pub popconfirm_ok: &'static str,
    pub popconfirm_cancel: &'static str,
    pub popconfirm_title: &'static str,
    // ── Result ──
    pub result_success: &'static str,
    pub result_error: &'static str,
    pub result_info: &'static str,
    pub result_warning: &'static str,
    pub result_404: &'static str,
    pub result_403: &'static str,
    pub result_500: &'static str,
    pub result_404_desc: &'static str,
    pub result_403_desc: &'static str,
    pub result_500_desc: &'static str,
    // ── Select ──
    pub select_optgroup_format: &'static str,
    // ── Table ──
    pub filter_title: &'static str,
    pub filter_confirm: &'static str,
    pub filter_reset: &'static str,
    // ── Timeline ──
    pub timeline_pending: &'static str,
    // ── Transfer ──
    pub transfer_source: &'static str,
    pub transfer_target: &'static str,
    // ── TreeSelect ──
    pub tree_select_placeholder: &'static str,
    // ── Upload ──
    pub upload_click: &'static str,
    pub upload_drag: &'static str,
    // ── QRCode ──
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
    nav_version: "UIX v0.0.1",
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
            nav_version: "UIX v0.0.1",
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
    crate::ui::foundation::provider_context::current_provider_context().locale
}

/// 在作用域内使用指定语言配置执行闭包。
pub fn with_locale<T>(locale: &Locale, f: impl FnOnce() -> T) -> T {
    crate::ui::foundation::provider_context::with_component_locale(locale, f)
}

use crate::ui::view::{View, ViewNode};

#[doc(hidden)]
pub struct MissingLocaleProviderChild;

/// 为一个 View 子树注入国际化文案。
pub struct LocaleProvider<F = MissingLocaleProviderChild> {
    locale: Locale,
    child: F,
}

impl LocaleProvider<MissingLocaleProviderChild> {
    pub fn new(locale: Locale) -> Self {
        Self {
            locale,
            child: MissingLocaleProviderChild,
        }
    }

    pub fn zh_cn() -> Self {
        Self::new(zh_cn())
    }

    pub fn en_us() -> Self {
        Self::new(en_us())
    }
}

impl<F> LocaleProvider<F> {
    pub fn child<G, V>(self, child: G) -> LocaleProvider<impl FnOnce() -> ViewNode>
    where
        G: FnOnce() -> V + 'static,
        V: View,
    {
        LocaleProvider {
            locale: self.locale,
            child: move || child().build(),
        }
    }
}

impl<F> View for LocaleProvider<F>
where
    F: FnOnce() -> ViewNode + 'static,
{
    fn build(self) -> ViewNode {
        with_locale(&self.locale, self.child)
    }
}
