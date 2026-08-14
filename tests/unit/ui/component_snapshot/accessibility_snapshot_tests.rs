// 组件快照无障碍语义专项测试。
// 覆盖角色双命名映射（ARIA 规范名 / 自动化稳定名）、aria 属性序列化、
// 快照字段到无障碍快照的收敛分支与组件配置快照的字段覆盖。

// 引入被测的公开快照类型。
use super::*;
// 引入字段覆盖测试所需的样式默认值与共享指针。
use crate::ui::theme::style::{Style, StyleSet};
use std::sync::Arc;
// 引入组件 trait 以构造最小测试桩。
use crate::ui::component::traits::{WidgetCapabilities, WidgetComponent};
// 引入快照枚举所需的控件尺寸。
use crate::platform::windowing::ControlSize;

/// 最小组件桩：只为配置快照提供类型标识，不承载任何组件逻辑。
struct ComponentMock;

impl WidgetComponent for ComponentMock {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::default()
    }
}

/// 构造一个最小的按钮组件配置快照（直接使用字段路径，不触碰组件树）。
fn config_with_button() -> ComponentConfigSnapshot {
    let fields = button_fields("提交", false, false, "");
    ComponentConfigSnapshot::from_component_fields(ComponentId::new(1), &ComponentMock, fields)
}

/// 构造一个带默认样式集的按钮快照字段。
fn button_fields(text: &str, disabled: bool, loading: bool, icon: &str) -> SnapshotFields {
    SnapshotFields::Button {
        text: text.to_owned(),
        disabled,
        block: false,
        loading,
        icon: icon.to_owned(),
        group_position: None,
        style_set: Arc::new(StyleSet::default()),
        style: Arc::new(Style::default()),
    }
}

/// 构造一个最小输入框快照字段。
fn input_fields(search: bool, password: bool, textarea: bool) -> SnapshotFields {
    SnapshotFields::Input {
        placeholder: "请输入".to_owned(),
        input_size: ControlSize::Medium,
        disabled: false,
        prefix: String::new(),
        suffix: String::new(),
        addon_before: String::new(),
        addon_after: String::new(),
        password,
        password_visible: false,
        clearable: false,
        search,
        status: None,
        status_message: String::new(),
        textarea,
        textarea_rows: 3,
        max_length: None,
    }
}

// ── 角色双命名映射（ARIA 规范名 / 自动化稳定名） ──────────────────────────

// aria_role 必须输出 WAI-ARIA 规范拼写；无语义角色返回 None。
#[test]
fn aria_role_maps_to_wai_aria_spellings() {
    // 逐个断言 ARIA 规范名映射。
    let cases = [
        (AccessibilityRole::Alert, Some("alert")),
        (AccessibilityRole::Button, Some("button")),
        (AccessibilityRole::Checkbox, Some("checkbox")),
        (AccessibilityRole::Combobox, Some("combobox")),
        (AccessibilityRole::Dialog, Some("dialog")),
        (AccessibilityRole::Group, Some("group")),
        (AccessibilityRole::Heading, Some("heading")),
        (AccessibilityRole::Image, Some("img")),
        (AccessibilityRole::List, Some("list")),
        (AccessibilityRole::Menu, Some("menu")),
        (AccessibilityRole::Navigation, Some("navigation")),
        (AccessibilityRole::ProgressBar, Some("progressbar")),
        (AccessibilityRole::RadioGroup, Some("radiogroup")),
        (AccessibilityRole::Separator, Some("separator")),
        (AccessibilityRole::Slider, Some("slider")),
        (AccessibilityRole::SpinButton, Some("spinbutton")),
        (AccessibilityRole::Status, Some("status")),
        (AccessibilityRole::Switch, Some("switch")),
        (AccessibilityRole::Table, Some("table")),
        (AccessibilityRole::TabList, Some("tablist")),
        (AccessibilityRole::TextBox, Some("textbox")),
        (AccessibilityRole::Tree, Some("tree")),
    ];
    // 断言全部带 ARIA 规范名的角色。
    for (role, expected) in cases {
        assert_eq!(role.aria_role(), expected, "aria_role 映射错误");
    }
    // 纯展示角色不得输出 ARIA 规范名。
    assert_eq!(AccessibilityRole::None.aria_role(), None);
    assert_eq!(AccessibilityRole::Generic.aria_role(), None);
    assert_eq!(AccessibilityRole::Text.aria_role(), None);
}

// automation_name 必须输出 snake_case 自动化/线协议稳定名，与 ARIA 名互不替换。
#[test]
fn automation_name_maps_to_snake_case_stable_names() {
    // 逐个断言自动化稳定名映射。
    let cases = [
        (AccessibilityRole::None, "none"),
        (AccessibilityRole::Generic, "generic"),
        (AccessibilityRole::Alert, "alert"),
        (AccessibilityRole::Button, "button"),
        (AccessibilityRole::Checkbox, "checkbox"),
        (AccessibilityRole::Combobox, "combobox"),
        (AccessibilityRole::Dialog, "dialog"),
        (AccessibilityRole::Group, "group"),
        (AccessibilityRole::Heading, "heading"),
        (AccessibilityRole::Image, "image"),
        (AccessibilityRole::List, "list"),
        (AccessibilityRole::Menu, "menu"),
        (AccessibilityRole::Navigation, "navigation"),
        (AccessibilityRole::ProgressBar, "progress_bar"),
        (AccessibilityRole::RadioGroup, "radio_group"),
        (AccessibilityRole::Separator, "separator"),
        (AccessibilityRole::Slider, "slider"),
        (AccessibilityRole::SpinButton, "spin_button"),
        (AccessibilityRole::Status, "status"),
        (AccessibilityRole::Switch, "switch"),
        (AccessibilityRole::Table, "table"),
        (AccessibilityRole::TabList, "tab_list"),
        (AccessibilityRole::Text, "text"),
        (AccessibilityRole::TextBox, "text_box"),
        (AccessibilityRole::Tree, "tree"),
    ];
    // 断言全部 25 个角色的稳定名。
    assert_eq!(cases.len(), 25);
    for (role, expected) in cases {
        assert_eq!(role.automation_name(), expected, "automation_name 映射错误");
    }
    // 双命名必须互不相同（多词角色 ARIA 与 snake_case 拼写差异）。
    assert_ne!(
        AccessibilityRole::ProgressBar.automation_name(),
        AccessibilityRole::ProgressBar.aria_role().unwrap()
    );
}

// ── AccessibilitySnapshot 构造与 aria 属性序列化 ──────────────────────────

// 构造器必须填充角色、可选名称与默认状态。
#[test]
fn snapshot_constructors_build_role_name_and_state() {
    // 无名称构造。
    let snapshot = AccessibilitySnapshot::new(AccessibilityRole::Slider);
    assert_eq!(snapshot.role, AccessibilityRole::Slider);
    assert_eq!(snapshot.name, None);
    assert_eq!(snapshot.state, AccessibilityState::default());
    // 带名称构造（空名称折叠为 None）。
    let named = AccessibilitySnapshot::named(AccessibilityRole::Button, "确定");
    assert_eq!(named.name.as_deref(), Some("确定"));
    // 空名称必须折叠为 None。
    let empty = AccessibilitySnapshot::named(AccessibilityRole::Button, "");
    assert_eq!(empty.name, None);
    // 状态构造必须整体替换。
    let state = AccessibilityState::disabled(true);
    let with_state =
        AccessibilitySnapshot::new(AccessibilityRole::Button).with_state(state.clone());
    assert_eq!(with_state.state, state);
}

// aria_attributes 必须按稳定顺序输出可访问性属性。
#[test]
fn aria_attributes_serialize_name_and_state() {
    // 构造带名称与完整状态的快照。
    let snapshot = AccessibilitySnapshot::named(AccessibilityRole::Slider, "音量").with_state(
        AccessibilityState {
            disabled: true,
            checked: Some(true),
            expanded: Some(false),
            selected: Some(true),
            value_text: Some("60".to_owned()),
            value_now: Some(60.0),
            value_min: Some(0.0),
            value_max: Some(100.0),
            multiline: true,
            password: true,
            required: true,
            ..AccessibilityState::default()
        },
    );
    let attributes = snapshot.aria_attributes();
    // 名称必须输出为 aria-label。
    assert!(attributes.contains(&AriaAttribute::new("aria-label", "音量")));
    // 布尔状态必须输出为字符串。
    assert!(attributes.contains(&AriaAttribute::new("aria-disabled", "true")));
    assert!(attributes.contains(&AriaAttribute::new("aria-checked", "true")));
    assert!(attributes.contains(&AriaAttribute::new("aria-expanded", "false")));
    assert!(attributes.contains(&AriaAttribute::new("aria-selected", "true")));
    assert!(attributes.contains(&AriaAttribute::new("aria-valuenow", "60")));
    assert!(attributes.contains(&AriaAttribute::new("aria-valuemin", "0")));
    assert!(attributes.contains(&AriaAttribute::new("aria-valuemax", "100")));
    assert!(attributes.contains(&AriaAttribute::new("aria-valuetext", "60")));
    assert!(attributes.contains(&AriaAttribute::new("aria-multiline", "true")));
    assert!(attributes.contains(&AriaAttribute::new("aria-required", "true")));
    // password 状态不是 ARIA 属性（由 value_text 隐藏承载）。
    assert!(
        !attributes
            .iter()
            .any(|attribute| attribute.name == "aria-password")
    );
    // 默认状态不得输出多余属性。
    let plain = AccessibilitySnapshot::new(AccessibilityRole::Button).aria_attributes();
    assert!(plain.is_empty());
}

// 自定义属性必须追加，同名时覆盖内置属性。
#[test]
fn aria_attributes_merge_custom_attributes_with_override() {
    // 内置 label 与自定义同名属性。
    let snapshot = AccessibilitySnapshot::named(AccessibilityRole::Button, "内置名")
        .with_attribute(AriaAttribute::new("aria-label", "自定义名"));
    let attributes = snapshot.aria_attributes();
    // 同名自定义属性必须覆盖内置值。
    assert_eq!(
        attributes
            .iter()
            .find(|attribute| attribute.name == "aria-label")
            .map(|attribute| attribute.value.as_str()),
        Some("自定义名")
    );
    // 追加无关自定义属性。
    let snapshot = snapshot.with_attribute(AriaAttribute::new("data-testid", "btn-1"));
    let attributes = snapshot.aria_attributes();
    assert!(
        attributes
            .iter()
            .any(|attribute| attribute.name == "data-testid" && attribute.value == "btn-1")
    );
}

// with_attributes 必须批量合并并去重。
#[test]
fn with_attributes_merges_and_deduplicates() {
    // 批量传入两个同名属性。
    let snapshot = AccessibilitySnapshot::new(AccessibilityRole::Button).with_attributes([
        AriaAttribute::new("aria-label", "第一次"),
        AriaAttribute::new("aria-label", "第二次"),
    ]);
    // 同名属性只保留最后一次。
    let attributes = snapshot.aria_attributes();
    let labels = attributes
        .iter()
        .filter(|attribute| attribute.name == "aria-label")
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].value, "第二次");
}

// ── SnapshotFields → 无障碍快照的关键收敛分支 ─────────────────────────────

// 按钮快照必须优先文本名称并合并禁用/加载状态。
#[test]
fn button_fields_expose_text_name_and_merged_disabled() {
    // 文本与图标并存时文本优先。
    let snapshot = button_fields("确定", false, false, "check").accessibility();
    assert_eq!(snapshot.role, AccessibilityRole::Button);
    assert_eq!(snapshot.name.as_deref(), Some("确定"));
    assert!(!snapshot.state.disabled);
    // 图标回退为名称。
    let snapshot = button_fields("", false, false, "check").accessibility();
    assert_eq!(snapshot.name.as_deref(), Some("check"));
    // 加载态必须并入禁用。
    let snapshot = button_fields("确定", false, true, "").accessibility();
    assert!(snapshot.state.disabled);
    // 显式禁用直接生效。
    let snapshot = button_fields("确定", true, false, "").accessibility();
    assert!(snapshot.state.disabled);
}

// 输入框快照必须区分搜索框角色与密码/多行状态。
#[test]
fn input_fields_expose_role_and_state() {
    // 普通输入框使用文本框角色。
    let snapshot = input_fields(false, false, false).accessibility();
    assert_eq!(snapshot.role, AccessibilityRole::TextBox);
    assert_eq!(snapshot.name.as_deref(), Some("请输入"));
    // 搜索框使用组合框角色。
    let snapshot = input_fields(true, false, false).accessibility();
    assert_eq!(snapshot.role, AccessibilityRole::Combobox);
    // 密码与多行状态必须写入快照。
    let snapshot = input_fields(false, true, true).accessibility();
    assert!(snapshot.state.password);
    assert!(snapshot.state.multiline);
    assert!(!snapshot.state.disabled);
}

// 复选框与开关快照必须暴露选中与禁用状态。
#[test]
fn checkbox_and_switch_fields_expose_checked_state() {
    // 复选框状态映射。
    let checkbox = SnapshotFields::Checkbox {
        checked: true,
        disabled: true,
        label: "同意".to_owned(),
    }
    .accessibility();
    assert_eq!(checkbox.role, AccessibilityRole::Checkbox);
    assert_eq!(checkbox.name.as_deref(), Some("同意"));
    assert_eq!(checkbox.state.checked, Some(true));
    assert!(checkbox.state.disabled);
    // 开关状态映射。
    let switch = SnapshotFields::Switch {
        label: "深色模式".to_owned(),
        checked: false,
        disabled: false,
        size: 20.0,
    }
    .accessibility();
    assert_eq!(switch.role, AccessibilityRole::Switch);
    // 显式开关标签必须成为可读的无障碍名称。
    assert_eq!(switch.name.as_deref(), Some("深色模式"));
    assert_eq!(switch.state.checked, Some(false));
}

// 选择器与标签页必须公开当前展开状态和活动位置。
#[cfg(feature = "navigation")]
#[test]
fn select_and_tabs_fields_expose_interaction_state() {
    // 构造打开的单选选择器快照。
    let select = SnapshotFields::Select {
        // 提供两项纯文本选项。
        options: vec!["中文".to_owned(), "English".to_owned()],
        // 本用例不使用分组选项。
        optgroups: Vec::new(),
        // 当前选中第二项。
        selected: 1,
        // 单选模式不保存多选索引。
        selected_multi: Vec::new(),
        // 选择器处于打开状态。
        open: true,
        // 选择器保持可用。
        disabled: false,
        // 选择器没有异步加载状态。
        loading: false,
        // 提供可读占位名称。
        placeholder: "语言".to_owned(),
        // 使用单选模式。
        multiple: false,
        // 本用例不启用搜索输入。
        search: false,
        // 搜索文本保持为空。
        search_query: String::new(),
        // 本用例使用内置选项绘制。
        custom_options: false,
    }
    // 转换为统一无障碍快照。
    .accessibility();
    // Select 必须公开打开的选项层。
    assert_eq!(select.state.expanded, Some(true));
    // 当前展示值必须指向第二项。
    assert_eq!(select.state.value_text.as_deref(), Some("English"));

    // 构造第二项活动的标签页快照。
    let tabs = SnapshotFields::Tabs {
        // 提供两个带稳定 key 的标签页。
        tabs: vec![
            // 首项作为非活动页签。
            crate::ui::widgets::Tab::new("概览").key("overview"),
            // 第二项作为当前活动页签。
            crate::ui::widgets::Tab::new("设置").key("settings"),
        ],
        // 当前活动索引指向第二项。
        active_index: 1,
        // 保存当前活动页签的稳定 key。
        active_key: Some("settings".to_owned()),
        // 使用默认顶部标签布局。
        position: crate::ui::widgets::TabPosition::Top,
        // 提供稳定的页签栏高度。
        tab_height: 40.0,
        // 本用例不固定整体宽度。
        fixed_width: None,
        // 本用例不固定整体高度。
        fixed_height: None,
    }
    // 转换为统一无障碍快照。
    .accessibility();
    // 活动位置必须采用从一开始的第二项。
    assert_eq!(tabs.state.value_now, Some(2.0));
    // 活动位置值域必须覆盖两个标签。
    assert_eq!(tabs.state.value_min, Some(1.0));
    // 活动位置上限必须等于标签总数。
    assert_eq!(tabs.state.value_max, Some(2.0));
}

// 滑块快照必须暴露值域与当前值。
#[test]
fn slider_fields_expose_value_range() {
    // 构造带值域的滑块快照。
    let slider = SnapshotFields::Slider {
        min: 0.0,
        max: 100.0,
        step: 1.0,
        value: 42.5,
        marks: Vec::new(),
        tooltip: None,
    }
    .accessibility();
    // 滑块角色必须正确。
    assert_eq!(slider.role, AccessibilityRole::Slider);
    // 值域与当前值必须完整暴露。
    assert_eq!(slider.state.value_now, Some(42.5));
    assert_eq!(slider.state.value_min, Some(0.0));
    assert_eq!(slider.state.value_max, Some(100.0));
}

// 单选框组快照必须暴露组名与当前选中文本。
#[test]
fn radio_fields_expose_group_and_selection_text() {
    // 构造三个选项的单选框组。
    let radio = SnapshotFields::Radio {
        group_name: "颜色".to_owned(),
        options: vec!["红".to_owned(), "绿".to_owned(), "蓝".to_owned()],
        selected: 1,
        disabled: false,
        direction: crate::ui::widgets::RadioDirection::Horizontal,
        item_h: 24.0,
    }
    .accessibility();
    // 单选框组角色必须正确。
    assert_eq!(radio.role, AccessibilityRole::RadioGroup);
    // 当前选中项必须作为值文本暴露。
    assert_eq!(radio.state.value_text.as_deref(), Some("绿"));
}

// 文本标签快照必须使用文本角色。
#[test]
fn label_fields_expose_text_role() {
    // 构造文本标签快照。
    let label = SnapshotFields::Label {
        text: "欢迎".to_owned(),
        font_size: 14.0,
        font_size_unit: None,
        color: None,
        fixed_width: None,
        fixed_height: None,
        style: None,
    }
    .accessibility();
    // 文本角色与名称必须正确。
    assert_eq!(label.role, AccessibilityRole::Text);
    assert_eq!(label.name.as_deref(), Some("欢迎"));
}

// 主题开关快照必须暴露暗色选中状态。
#[test]
fn theme_toggle_fields_expose_dark_checked_state() {
    // 暗色主题开关。
    let dark = SnapshotFields::ThemeToggle { dark: true }.accessibility();
    assert_eq!(dark.role, AccessibilityRole::Button);
    assert_eq!(dark.state.checked, Some(true));
    assert_eq!(dark.state.value_text.as_deref(), Some("dark"));
    // 亮色主题开关。
    let light = SnapshotFields::ThemeToggle { dark: false }.accessibility();
    assert_eq!(light.state.checked, Some(false));
    assert_eq!(light.state.value_text.as_deref(), Some("light"));
}

// 对话框快照必须随开合切换角色与名称（反馈能力）。
#[cfg(feature = "feedback")]
#[test]
fn modal_fields_switch_role_between_dialog_and_button() {
    // 打开状态必须使用对话框角色与标题名称。
    let open = SnapshotFields::Modal {
        title: "确认".to_owned(),
        open: true,
        width: 400.0,
        height: 200.0,
        modal_size: ControlSize::Medium,
        closable: true,
        mask_closable: true,
        footer_visible: true,
        centered: false,
        overlay: true,
        backdrop_blur: None,
    }
    .accessibility();
    assert_eq!(open.role, AccessibilityRole::Dialog);
    assert_eq!(open.name.as_deref(), Some("确认"));
    assert_eq!(open.state.expanded, Some(true));
    // 关闭状态必须使用按钮角色与打开提示名称。
    let closed = SnapshotFields::Modal {
        title: "确认".to_owned(),
        open: false,
        width: 400.0,
        height: 200.0,
        modal_size: ControlSize::Medium,
        closable: true,
        mask_closable: true,
        footer_visible: true,
        centered: false,
        overlay: true,
        backdrop_blur: None,
    }
    .accessibility();
    assert_eq!(closed.role, AccessibilityRole::Button);
    assert_eq!(closed.name.as_deref(), Some("打开 确认"));
    assert_eq!(closed.state.expanded, Some(false));
}

// 悬浮按钮快照必须按 说明 > 提示 > 图标 优先级选择名称。
#[test]
fn float_button_fields_prioritize_description_over_tooltip_over_icon() {
    // 三个名称源同时存在时说明优先。
    let snapshot = SnapshotFields::FloatButton {
        icon: "plus".to_owned(),
        description: "添加".to_owned(),
        tooltip: "新建".to_owned(),
        badge_count: 0,
        badge_dot: false,
        size: 48.0,
        x: 0.0,
        y: 0.0,
        placement: None,
        reserve_layout_space: false,
    }
    .accessibility();
    assert_eq!(snapshot.role, AccessibilityRole::Button);
    assert_eq!(snapshot.name.as_deref(), Some("添加"));
    // 说明为空时提示接管。
    let snapshot = SnapshotFields::FloatButton {
        icon: "plus".to_owned(),
        description: String::new(),
        tooltip: "新建".to_owned(),
        badge_count: 3,
        badge_dot: false,
        size: 48.0,
        x: 0.0,
        y: 0.0,
        placement: None,
        reserve_layout_space: false,
    }
    .accessibility();
    assert_eq!(snapshot.name.as_deref(), Some("新建"));
    // 数字徽标必须作为值文本暴露。
    assert_eq!(snapshot.state.value_text.as_deref(), Some("3 条通知"));
}

// 未知快照必须回退为通用角色。
#[test]
fn unknown_fields_fall_back_to_generic_role() {
    // 未知变体必须产生通用角色快照。
    let snapshot = SnapshotFields::Unknown.accessibility();
    assert_eq!(snapshot.role, AccessibilityRole::Generic);
    assert_eq!(snapshot.name, None);
}

// ── ComponentConfigSnapshot 字段覆盖 ──────────────────────────────────────

// 无障碍覆盖必须优先于字段派生快照。
#[test]
fn accessibility_override_wins_over_derived_fields() {
    // 构造无覆盖的配置快照。
    let config = config_with_button();
    assert_eq!(config.accessibility().name.as_deref(), Some("提交"));
    assert_eq!(config.aria_role(), Some("button"));
    // 注入覆盖快照。
    let overridden = config.with_accessibility(AccessibilitySnapshot::named(
        AccessibilityRole::Dialog,
        "覆盖名",
    ));
    // 覆盖快照必须取代字段派生结果。
    assert_eq!(overridden.accessibility().role, AccessibilityRole::Dialog);
    assert_eq!(overridden.aria_role(), Some("dialog"));
    // 覆盖快照的名称与属性必须一致。
    let attributes = overridden.aria_attributes();
    assert!(
        attributes
            .iter()
            .any(|attribute| attribute.name == "aria-label" && attribute.value == "覆盖名")
    );
}

// 选择类字段必须能从配置快照提取选择状态。
#[test]
fn config_snapshot_extracts_selection_state() {
    // 构造单选框组配置快照。
    let fields = SnapshotFields::Radio {
        group_name: "等级".to_owned(),
        options: vec!["低".to_owned(), "中".to_owned(), "高".to_owned()],
        selected: 2,
        disabled: false,
        direction: crate::ui::widgets::RadioDirection::Horizontal,
        item_h: 24.0,
    };
    let config =
        ComponentConfigSnapshot::from_component_fields(ComponentId::new(2), &ComponentMock, fields);
    // 选择状态必须包含选项与选中索引。
    let selection = config.selection().expect("单选框必须有选择状态");
    assert_eq!(selection.options, vec!["低", "中", "高"]);
    assert_eq!(selection.selected_indices, vec![2]);
    assert!(!selection.multiple);
}
