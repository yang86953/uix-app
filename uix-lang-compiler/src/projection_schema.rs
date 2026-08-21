//! UIX Lang 可枚举 UI 投影事实的唯一登记入口。

/// 当前 schema 的独立版本；缓存与工具协议必须把它计入身份。
pub const SCHEMA_VERSION: u32 = 1;
/// 当前语言基线版本。
pub const LANGUAGE_VERSION: &str = "0.0.1";

/// 声明一个登记项当前是否可用于正式 AOT。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationStatus {
    /// 已有确定 Rust 映射并允许生成。
    Available,
    /// 已保留语言身份，但尚未开放当前入口。
    Planned,
}

/// 声明内置组件所属的参考文档类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentCategory {
    General,
    Layout,
    Input,
    Display,
    Feedback,
    Navigation,
    Chart,
    Application,
}

impl ComponentCategory {
    /// 返回面向诊断和文档的稳定中文类别名。
    pub const fn label(self) -> &'static str {
        match self {
            Self::General => "通用组件",
            Self::Layout => "布局组件",
            Self::Input => "输入组件",
            Self::Display => "展示组件",
            Self::Feedback => "反馈组件",
            Self::Navigation => "导航组件",
            Self::Chart => "图表组件",
            Self::Application => "标签语法 / App 应用入口",
        }
    }
}

/// 描述一个内置标签的稳定身份、生成入口与 capability。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub category: ComponentCategory,
    pub status: RegistrationStatus,
    pub emitter: Option<&'static str>,
    pub capability: Option<&'static str>,
}

/// 描述一个事件名当前公开的 `$event` 顶层字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub fields: &'static [&'static str],
}

/// 描述一个语言数据构造器到公开 Rust API 的确定映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataConstructorSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub rust_path: &'static str,
    pub method: &'static str,
    pub normalize_numbers: bool,
    pub capability: Option<&'static str>,
}

/// 提供 Compiler System 唯一拥有的只读 UI 投影 schema。
#[derive(Debug, Clone, Copy, Default)]
pub struct UiProjectionSchema;

/// 全部入口共享的 schema 实例。
pub const UI_PROJECTION_SCHEMA: UiProjectionSchema = UiProjectionSchema;

impl UiProjectionSchema {
    /// 返回当前 schema 版本。
    pub const fn version(self) -> u32 {
        SCHEMA_VERSION
    }

    /// 返回当前语言版本。
    pub const fn language_version(self) -> &'static str {
        LANGUAGE_VERSION
    }

    /// 返回完整组件登记表。
    pub const fn components(self) -> &'static [ComponentSpec] {
        COMPONENTS
    }

    /// 按精确标签名查询组件登记。
    pub fn component(self, name: &str) -> Option<&'static ComponentSpec> {
        COMPONENTS.iter().find(|entry| entry.name == name)
    }

    /// 生成当前 AOT 可用标签的确定性诊断清单。
    pub fn supported_component_hint(self) -> String {
        COMPONENTS
            .iter()
            .filter(|entry| entry.status == RegistrationStatus::Available)
            .map(|entry| entry.name)
            .collect::<Vec<_>>()
            .join("、")
    }

    /// 返回完整事件载荷登记表。
    pub const fn events(self) -> &'static [EventSpec] {
        EVENTS
    }

    /// 按精确事件属性名查询载荷字段。
    pub fn event(self, name: &str) -> Option<&'static EventSpec> {
        EVENTS.iter().find(|entry| entry.name == name)
    }

    /// 返回完整数据构造登记表。
    pub const fn data_constructors(self) -> &'static [DataConstructorSpec] {
        DATA_CONSTRUCTORS
    }

    /// 按精确语言类型名查询数据构造映射。
    pub fn data_constructor(self, name: &str) -> Option<&'static DataConstructorSpec> {
        DATA_CONSTRUCTORS.iter().find(|entry| entry.name == name)
    }
}

macro_rules! component {
    ($name:literal, $category:ident, $emitter:literal) => {
        component!($name, $category, $emitter, None)
    };
    ($name:literal, $category:ident, $emitter:literal, $capability:expr) => {
        ComponentSpec {
            id: concat!("component.", $name),
            name: $name,
            category: ComponentCategory::$category,
            status: RegistrationStatus::Available,
            emitter: Some($emitter),
            capability: $capability,
        }
    };
}

// 顺序保持现有诊断清单与参考分类稳定。
const COMPONENTS: &[ComponentSpec] = &[
    component!("Text", General, "generate_text"),
    component!("Label", General, "generate_text"),
    component!("Button", General, "generate_button"),
    component!("Container", Layout, "generate_container"),
    component!("Row", Layout, "generate_row"),
    component!("Column", Layout, "generate_column"),
    component!("Grid", Layout, "generate_grid"),
    component!("ScrollView", Layout, "generate_scroll_view"),
    component!("VirtualScroll", Layout, "generate_virtual_scroll"),
    component!("Affix", Layout, "generate_affix"),
    component!("BackTop", Layout, "generate_back_top"),
    component!("FloatButtonBackTop", Layout, "generate_back_top"),
    component!("Splitter", Layout, "generate_splitter"),
    component!("Layout", Layout, "generate_app_layout"),
    component!("Sider", Layout, "generate_app_layout"),
    component!("Header", Layout, "generate_app_layout"),
    component!("Content", Layout, "generate_app_layout"),
    component!("Footer", Layout, "generate_app_layout"),
    component!("Input", Input, "generate_input"),
    component!("InputNumber", Input, "generate_input_number"),
    component!("InputGroup", Input, "generate_input_group"),
    component!("Slider", Input, "generate_slider"),
    component!("RangeSlider", Input, "generate_range_slider"),
    component!("Rate", Input, "generate_rate"),
    component!("Checkbox", Input, "generate_checkbox"),
    component!("Switch", Input, "generate_switch"),
    component!("Radio", Input, "generate_radio"),
    component!("Segmented", Input, "generate_segmented"),
    component!("Select", Input, "generate_select"),
    component!("Cascader", Input, "generate_cascader", Some("tree-widgets")),
    component!(
        "TreeSelect",
        Input,
        "generate_tree_select",
        Some("tree-widgets")
    ),
    component!("AutoComplete", Input, "generate_autocomplete"),
    component!("Mentions", Input, "generate_mentions"),
    component!("DatePicker", Input, "generate_date_picker"),
    component!("DateRangePicker", Input, "generate_date_range_picker"),
    component!("TimePicker", Input, "generate_time_picker"),
    component!("ColorPicker", Input, "generate_color_picker"),
    component!("Avatar", Display, "generate_avatar", Some("image-codecs")),
    component!("Badge", Display, "generate_badge"),
    component!("Image", Display, "generate_image", Some("image-codecs")),
    component!(
        "ImageGroup",
        Display,
        "generate_image_group",
        Some("image-codecs")
    ),
    component!("List", Display, "generate_list"),
    component!("SelectableList", Display, "generate_selectable_list"),
    component!("Collapse", Display, "generate_collapse"),
    component!("Upload", Input, "generate_upload"),
    component!(
        "Message",
        Feedback,
        "generate_message_declaration",
        Some("feedback")
    ),
    component!(
        "Notification",
        Feedback,
        "generate_notification_declaration",
        Some("feedback")
    ),
    component!("Skeleton", Display, "generate_skeleton"),
    component!("Empty", Display, "generate_empty"),
    component!("ResultView", Display, "generate_result_view"),
    component!("Tag", Display, "generate_tag"),
    component!("Card", Display, "generate_card"),
    component!("Descriptions", Display, "generate_descriptions"),
    component!("Timeline", Display, "generate_timeline"),
    component!("Calendar", Display, "generate_calendar"),
    component!("Carousel", Display, "generate_carousel"),
    component!("Tree", Display, "generate_tree", Some("tree-widgets")),
    component!("Table", Display, "generate_table", Some("table")),
    component!("Tabs", Navigation, "generate_tabs", Some("navigation")),
    component!("Menu", Navigation, "generate_menu", Some("navigation")),
    component!(
        "Dropdown",
        Navigation,
        "generate_dropdown",
        Some("navigation")
    ),
    component!(
        "Navigation",
        Navigation,
        "generate_navigation",
        Some("navigation")
    ),
    component!("Steps", Navigation, "generate_steps", Some("navigation")),
    component!(
        "Pagination",
        Navigation,
        "generate_pagination",
        Some("navigation")
    ),
    component!(
        "Breadcrumb",
        Navigation,
        "generate_breadcrumb",
        Some("navigation")
    ),
    component!("Anchor", Navigation, "generate_anchor", Some("navigation")),
    component!("QRCode", Display, "generate_qrcode", Some("qrcode")),
    component!("Watermark", Display, "generate_watermark"),
    component!("RichText", Display, "generate_rich_text", Some("rich-text")),
    component!("Alert", Feedback, "generate_alert", Some("feedback")),
    component!(
        "ProgressBar",
        Feedback,
        "generate_progress_bar",
        Some("feedback")
    ),
    component!("BarChart", Chart, "generate_basic_chart", Some("charts")),
    component!("LineChart", Chart, "generate_basic_chart", Some("charts")),
    component!("PieChart", Chart, "generate_basic_chart", Some("charts")),
    component!("AreaChart", Chart, "generate_static_chart", Some("charts")),
    component!(
        "ScatterChart",
        Chart,
        "generate_static_chart",
        Some("charts")
    ),
    component!(
        "FunnelChart",
        Chart,
        "generate_static_chart",
        Some("charts")
    ),
    component!(
        "Treemap",
        Chart,
        "generate_hierarchy_gauge_chart",
        Some("charts")
    ),
    component!(
        "Gauge",
        Chart,
        "generate_hierarchy_gauge_chart",
        Some("charts")
    ),
    component!(
        "Heatmap",
        Chart,
        "generate_matrix_delta_chart",
        Some("charts")
    ),
    component!(
        "WaterfallChart",
        Chart,
        "generate_matrix_delta_chart",
        Some("charts")
    ),
    component!("RadarChart", Chart, "generate_series_chart", Some("charts")),
    component!("ComboChart", Chart, "generate_series_chart", Some("charts")),
    component!(
        "Popconfirm",
        Feedback,
        "generate_popconfirm",
        Some("feedback")
    ),
    component!("Modal", Feedback, "generate_modal", Some("feedback")),
    component!("Drawer", Feedback, "generate_drawer", Some("feedback")),
    component!("Tooltip", Feedback, "generate_tooltip", Some("feedback")),
    component!("Popover", Feedback, "generate_popover", Some("feedback")),
    component!(
        "FocusTrap",
        Feedback,
        "generate_focus_trap",
        Some("feedback")
    ),
    component!("Spin", Feedback, "generate_spin", Some("feedback")),
    component!("Form", Input, "generate_form"),
    component!("FormInputItem", Input, "generate_orphan_form_input_item"),
    component!("FormSelectItem", Input, "generate_orphan_form_select_item"),
    component!(
        "FormCheckboxItem",
        Input,
        "generate_orphan_form_checkbox_item"
    ),
    component!("FormRadioItem", Input, "generate_orphan_form_radio_item"),
    component!("FormSwitchItem", Input, "generate_orphan_form_switch_item"),
    component!("FormSliderItem", Input, "generate_orphan_form_slider_item"),
    component!("Col", Layout, "generate_orphan_col"),
    component!("Icon", General, "generate_icon"),
    component!("Divider", General, "generate_divider"),
    component!("Space", General, "generate_space"),
    component!("Typography", General, "generate_typography"),
    component!("ThemeToggle", General, "generate_theme_toggle"),
    component!("ButtonGroup", General, "generate_button_group"),
    component!("WindowControl", General, "generate_window_control"),
    component!("WindowDragRegion", General, "generate_window_drag_region"),
    component!("FloatButton", General, "generate_float_button"),
    component!("FloatButtonGroup", General, "generate_float_button_group"),
    ComponentSpec {
        id: "component.App",
        name: "App",
        category: ComponentCategory::Application,
        status: RegistrationStatus::Planned,
        emitter: None,
        capability: None,
    },
];

const EVENTS: &[EventSpec] = &[
    EventSpec {
        id: "event.click",
        name: "@click",
        fields: &["x", "y"],
    },
    EventSpec {
        id: "event.change",
        name: "@change",
        fields: &["value"],
    },
    EventSpec {
        id: "event.select",
        name: "@select",
        fields: &["value"],
    },
    EventSpec {
        id: "event.submit",
        name: "@submit",
        fields: &["value"],
    },
    EventSpec {
        id: "event.close",
        name: "@close",
        fields: &["value"],
    },
    EventSpec {
        id: "event.keyDown",
        name: "@keyDown",
        fields: &["key", "code"],
    },
    EventSpec {
        id: "event.keyUp",
        name: "@keyUp",
        fields: &["key", "code"],
    },
];

macro_rules! data_constructor {
    ($name:literal, $path:literal) => {
        data_constructor!($name, $path, "new", false, None)
    };
    ($name:literal, $path:literal, $method:literal, $normalize:expr, $capability:expr) => {
        DataConstructorSpec {
            id: concat!("data.", $name),
            name: $name,
            rust_path: $path,
            method: $method,
            normalize_numbers: $normalize,
            capability: $capability,
        }
    };
}

const DATA_CONSTRUCTORS: &[DataConstructorSpec] = &[
    data_constructor!("SelectOption", "::uix::prelude::SelectOption"),
    data_constructor!(
        "CascaderOption",
        "::uix::prelude::CascaderOption",
        "new",
        false,
        Some("tree-widgets")
    ),
    data_constructor!(
        "TreeNode",
        "::uix::prelude::TreeNode",
        "new",
        false,
        Some("tree-widgets")
    ),
    data_constructor!("TimelineItem", "::uix::prelude::TimelineItem"),
    data_constructor!("SelectableItem", "::uix::prelude::SelectableItem"),
    data_constructor!("CollapsePanel", "::uix::prelude::CollapsePanel"),
    data_constructor!(
        "Step",
        "::uix::prelude::Step",
        "new",
        false,
        Some("navigation")
    ),
    data_constructor!("DescriptionsItem", "::uix::prelude::DescriptionsItem"),
    data_constructor!(
        "BreadcrumbItem",
        "::uix::prelude::BreadcrumbItem",
        "new",
        false,
        Some("navigation")
    ),
    data_constructor!(
        "AnchorItem",
        "::uix::prelude::AnchorItem",
        "new",
        false,
        Some("navigation")
    ),
    data_constructor!(
        "Tab",
        "::uix::prelude::Tab",
        "new",
        false,
        Some("navigation")
    ),
    data_constructor!(
        "MenuItem",
        "::uix::prelude::MenuItem",
        "from_text",
        false,
        Some("navigation")
    ),
    data_constructor!(
        "DropdownItem",
        "::uix::prelude::DropdownItem",
        "from_text",
        false,
        Some("navigation")
    ),
    data_constructor!(
        "BarData",
        "::uix::prelude::BarData",
        "new",
        true,
        Some("charts")
    ),
    data_constructor!(
        "LineData",
        "::uix::prelude::LineData",
        "new",
        true,
        Some("charts")
    ),
    data_constructor!(
        "PieData",
        "::uix::prelude::PieData",
        "new",
        true,
        Some("charts")
    ),
    data_constructor!(
        "ScatterData",
        "::uix::prelude::ScatterData",
        "new",
        true,
        Some("charts")
    ),
    data_constructor!(
        "BubbleData",
        "::uix::prelude::BubbleData",
        "new",
        true,
        Some("charts")
    ),
    data_constructor!(
        "FunnelData",
        "::uix::prelude::FunnelData",
        "new",
        true,
        Some("charts")
    ),
    data_constructor!(
        "TreemapNode",
        "::uix::prelude::TreemapNode",
        "new",
        true,
        Some("charts")
    ),
    data_constructor!(
        "GaugeRange",
        "::uix::prelude::GaugeRange",
        "new",
        true,
        Some("charts")
    ),
    data_constructor!(
        "HeatmapCell",
        "::uix::prelude::HeatmapCell",
        "new",
        false,
        Some("charts")
    ),
    data_constructor!(
        "WaterfallData",
        "::uix::prelude::WaterfallData",
        "new",
        false,
        Some("charts")
    ),
    data_constructor!(
        "ChartSeries",
        "::uix::prelude::ChartSeries",
        "new",
        false,
        Some("charts")
    ),
    data_constructor!(
        "ComboSeries",
        "::uix::prelude::ComboSeries",
        "new",
        false,
        Some("charts")
    ),
    data_constructor!(
        "RadarAxis",
        "::uix::prelude::RadarAxis",
        "new",
        false,
        Some("charts")
    ),
    data_constructor!(
        "RadarData",
        "::uix::prelude::RadarData",
        "new",
        true,
        Some("charts")
    ),
    data_constructor!("Date", "::uix::prelude::Date"),
    data_constructor!("Time", "::uix::prelude::Time"),
    data_constructor!("Color", "::uix::prelude::Color", "hex", false, None),
    data_constructor!("Point", "::uix::prelude::Point", "new", true, None),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{RegistrationStatus, UI_PROJECTION_SCHEMA};

    #[test]
    fn schema_entries_have_unique_stable_identities() {
        let component_ids = UI_PROJECTION_SCHEMA
            .components()
            .iter()
            .map(|entry| entry.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(component_ids.len(), UI_PROJECTION_SCHEMA.components().len());

        let event_ids = UI_PROJECTION_SCHEMA
            .events()
            .iter()
            .map(|entry| entry.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(event_ids.len(), UI_PROJECTION_SCHEMA.events().len());

        let data_ids = UI_PROJECTION_SCHEMA
            .data_constructors()
            .iter()
            .map(|entry| entry.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            data_ids.len(),
            UI_PROJECTION_SCHEMA.data_constructors().len()
        );
    }

    #[test]
    fn schema_matches_current_aot_registration_counts() {
        let available = UI_PROJECTION_SCHEMA
            .components()
            .iter()
            .filter(|entry| entry.status == RegistrationStatus::Available)
            .count();
        assert_eq!(available, 108);
        assert_eq!(UI_PROJECTION_SCHEMA.data_constructors().len(), 31);
        assert_eq!(UI_PROJECTION_SCHEMA.events().len(), 7);
        assert_eq!(
            UI_PROJECTION_SCHEMA.component("App").unwrap().status,
            RegistrationStatus::Planned
        );
    }
}
