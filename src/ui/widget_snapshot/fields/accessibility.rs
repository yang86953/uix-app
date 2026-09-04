use crate::ui::widgets::*;

use super::super::accessibility::{
    avatar_accessibility, back_top_accessibility, badge_accessibility, calendar_accessibility,
    card_accessibility, carousel_accessibility, date_range_accessibility, first_non_empty,
    image_accessibility, input_number_accessibility, result_accessibility, select_accessibility,
    selectable_list_accessibility, splitter_accessibility, tag_accessibility,
    theme_toggle_accessibility, timeline_accessibility, transfer_accessibility,
    typography_accessibility, upload_accessibility,
};
// 反馈 capability 启用时才引入专属无障碍转换函数。
#[cfg(feature = "feedback")]
// 两个函数只处理同步门控的进度条与气泡确认框快照。
use super::super::accessibility::{popconfirm_accessibility, progress_accessibility};
// 导航 capability 启用时才引入专属无障碍转换函数。
#[cfg(feature = "navigation")]
// 这些函数只处理同步门控的八种导航快照变体。
use super::super::accessibility::{
    anchor_accessibility, breadcrumb_accessibility, dropdown_accessibility,
    menu_accessibility, menu_bar_accessibility, pagination_accessibility, steps_accessibility,
    tabs_accessibility,
};
// 图表 capability 启用时才引入专属无障碍摘要辅助函数。
#[cfg(feature = "charts")]
// 该函数只处理同步门控的四种图表快照变体。
use super::super::accessibility::chart_accessibility;
// 富文本 capability 启用时才引入专属无障碍转换函数。
#[cfg(feature = "rich-text")]
// 该函数只处理同步门控的 RichText 快照变体。
use super::super::accessibility::rich_text_accessibility;
// 树组件 capability 启用时才引入专属无障碍转换函数。
#[cfg(feature = "tree-widgets")]
// 该函数只处理同步门控的展示树快照变体。
use super::super::accessibility::tree_accessibility;
// 终端 capability 启用时才引入专属无障碍转换函数。
#[cfg(feature = "terminal")]
// 该函数只处理同步门控的终端快照变体。
use super::super::accessibility::terminal_accessibility;
use super::super::{AccessibilityRole, AccessibilitySnapshot, AccessibilityState};
use super::SnapshotFields;

impl SnapshotFields {
    /// 从组件字段快照派生默认无障碍角色、名称、状态与属性。
    pub fn accessibility(&self) -> AccessibilitySnapshot {
        match self {
            Self::Button {
                text,
                icon,
                disabled,
                loading,
                ..
            } => AccessibilitySnapshot::named(
                AccessibilityRole::Button,
                first_non_empty([text.as_str(), icon.as_str()]),
            )
            .with_state(AccessibilityState::disabled(*disabled || *loading)),
            Self::WindowControl {
                accessible_name, ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Button, accessible_name.clone()),
            Self::Label { text, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::Text, text.clone())
            }
            Self::Avatar { text, src, .. } => avatar_accessibility(text, src),
            Self::Input {
                placeholder,
                disabled,
                password,
                search,
                textarea,
                ..
            } => {
                let role = if *search {
                    AccessibilityRole::Combobox
                } else {
                    AccessibilityRole::TextBox
                };
                AccessibilitySnapshot::named(role, placeholder.clone()).with_state(
                    AccessibilityState {
                        disabled: *disabled,
                        multiline: *textarea,
                        password: *password,
                        ..AccessibilityState::default()
                    },
                )
            }
            Self::Typography {
                content,
                type_,
                disabled,
                copyable,
                ..
            } => typography_accessibility(content, *type_, *disabled, *copyable),
            Self::Checkbox {
                checked,
                disabled,
                label,
            } => AccessibilitySnapshot::named(AccessibilityRole::Checkbox, label.clone())
                .with_state(AccessibilityState {
                    disabled: *disabled,
                    checked: Some(*checked),
                    ..AccessibilityState::default()
                }),
            Self::Radio {
                group_name,
                options,
                selected,
                disabled,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::RadioGroup, group_name.clone())
                .with_state(AccessibilityState {
                    disabled: *disabled,
                    value_text: options.get(*selected).cloned(),
                    ..AccessibilityState::default()
                }),
            Self::Switch {
                checked,
                disabled,
                label,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Switch, label.clone()).with_state(
                AccessibilityState {
                    disabled: *disabled,
                    checked: Some(*checked),
                    ..AccessibilityState::default()
                },
            ),
            Self::Slider {
                min, max, value, ..
            } => AccessibilitySnapshot::new(AccessibilityRole::Slider).with_state(
                AccessibilityState {
                    value_now: Some(*value),
                    value_min: Some(*min),
                    value_max: Some(*max),
                    ..AccessibilityState::default()
                },
            ),
            Self::RangeSlider {
                min,
                max,
                start,
                end,
                active_thumb,
                ..
            } => AccessibilitySnapshot::new(AccessibilityRole::Slider).with_state(
                AccessibilityState {
                    value_now: Some(match active_thumb {
                        RangeSliderThumb::Start => *start,
                        RangeSliderThumb::End => *end,
                    }),
                    value_min: Some(*min),
                    value_max: Some(*max),
                    value_text: Some(format!("{start}..{end}")),
                    ..AccessibilityState::default()
                },
            ),
            Self::Splitter {
                ratios,
                active_handle,
                ..
            } => splitter_accessibility(ratios, *active_handle),
            Self::Rate {
                count,
                value,
                half,
                disabled,
                ..
            } => AccessibilitySnapshot::new(AccessibilityRole::Slider).with_state(
                AccessibilityState {
                    disabled: *disabled,
                    value_now: Some(*value as f64),
                    value_min: Some(0.0),
                    value_max: Some(if *half {
                        count.saturating_mul(2) as f64
                    } else {
                        *count as f64
                    }),
                    ..AccessibilityState::default()
                },
            ),
            Self::InputNumber {
                value,
                min,
                max,
                placeholder,
                disabled,
                formatted,
                display_value,
                ..
            } => {
                let accessible_display = if *formatted {
                    display_value.as_deref()
                } else {
                    None
                };
                input_number_accessibility(
                    *value,
                    *min,
                    *max,
                    placeholder,
                    *disabled,
                    accessible_display,
                )
            }
            Self::Empty { description, .. } => AccessibilitySnapshot::named(
                AccessibilityRole::Status,
                if description.is_empty() {
                    crate::ui::widget_runtime::locale::use_locale()
                        .empty_description
                        .to_string()
                } else {
                    description.clone()
                },
            ),
            Self::Image {
                alt,
                fallback,
                src,
                preview,
                preview_open,
                ..
            } => image_accessibility(alt, fallback, src, *preview, *preview_open),
            Self::ImageGroup {
                images,
                current,
                preview_open,
                ..
            } => {
                let count = images.len();
                let index = (*current).min(count.saturating_sub(1));
                let role = if *preview_open {
                    AccessibilityRole::Dialog
                } else if count > 0 {
                    AccessibilityRole::Button
                } else {
                    AccessibilityRole::Group
                };
                let value_text = images.get(index).map(|path| {
                    if path.is_empty() {
                        format!("图片 {} / {count}", index + 1)
                    } else {
                        format!("{path}，图片 {} / {count}", index + 1)
                    }
                });
                AccessibilitySnapshot::named(
                    role,
                    if *preview_open {
                        "图片预览"
                    } else {
                        "图片组"
                    },
                )
                .with_state(AccessibilityState {
                    expanded: (count > 0).then_some(*preview_open),
                    value_text,
                    value_now: (count > 0).then_some((index + 1) as f64),
                    value_min: (count > 0).then_some(1.0),
                    value_max: (count > 0).then_some(count as f64),
                    ..AccessibilityState::default()
                })
            }
            Self::Tag {
                text,
                closable,
                checkable,
                checked,
                ..
            } => tag_accessibility(text, *closable, *checkable, *checked),
            Self::Timeline {
                items,
                pending,
                reverse,
            } => timeline_accessibility(items, *pending, *reverse),
            Self::FloatButton {
                // 借用图标名称作为最终语义回退。
                icon,
                // 借用展开说明作为首选语义名称。
                description,
                // 借用提示文字作为次选语义名称。
                tooltip,
                // 借用数字徽标状态。
                badge_count,
                // 借用圆点徽标状态。
                badge_dot,
                // 忽略不影响无障碍语义的几何字段。
                ..
            } => {
                // 圆点徽标优先表达未读状态。
                let value_text = if *badge_dot {
                    // 使用不依赖视觉形状的状态描述。
                    Some("有新通知".to_string())
                } else if *badge_count > 0 {
                    // 数字徽标暴露确定数量。
                    Some(format!("{badge_count} 条通知"))
                } else {
                    // 无徽标时不暴露额外状态。
                    None
                };
                // 构造带可读名称与徽标状态的按钮快照。
                AccessibilitySnapshot::named(
                    // FloatButton 继续使用按钮角色。
                    AccessibilityRole::Button,
                    // 说明优先于提示，图标名称只作为最后回退。
                    first_non_empty([
                        // 首选展开说明。
                        description.as_str(),
                        // 次选提示文字。
                        tooltip.as_str(),
                        // 最后使用 Lucide 图标名称。
                        icon.as_str(),
                    ]),
                )
                // 附加可选徽标状态文本。
                .with_state(AccessibilityState {
                    // 保存数字或圆点状态描述。
                    value_text,
                    // 其他按钮状态保持默认。
                    ..AccessibilityState::default()
                })
            }
            Self::FloatButtonGroup {
                button_count,
                expanded,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Button, "浮动按钮组").with_state(
                AccessibilityState {
                    expanded: Some(*expanded),
                    value_text: Some(format!("{button_count} 个操作")),
                    ..AccessibilityState::default()
                },
            ),
            Self::Badge {
                count,
                max,
                dot,
                status,
                show_zero,
                text,
                ..
            } => badge_accessibility(*count, *max, *dot, *status, *show_zero, text),
            // 反馈 capability 启用时才匹配同步存在的进度条快照变体。
            #[cfg(feature = "feedback")]
            Self::ProgressBar { mode, .. } => progress_accessibility(*mode),
            // 反馈 capability 启用时才匹配同步存在的警告提示快照变体。
            #[cfg(feature = "feedback")]
            Self::Alert {
                message,
                description,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Alert, message.clone())
                .with_state(AccessibilityState {
                    value_text: (!description.is_empty()).then(|| description.clone()),
                    ..AccessibilityState::default()
                }),
            // 反馈 capability 启用时才匹配同步存在的全局消息快照变体。
            #[cfg(feature = "feedback")]
            Self::Message { contents, .. } if !contents.is_empty() => {
                let name = contents
                    .iter()
                    .filter(|content| !content.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("；");
                AccessibilitySnapshot::named(
                    AccessibilityRole::Alert,
                    if name.is_empty() {
                        "Message".to_owned()
                    } else {
                        name
                    },
                )
            }
            // 反馈 capability 启用时才匹配同步存在的通知快照变体。
            #[cfg(feature = "feedback")]
            Self::Notification {
                titles,
                descriptions,
                ..
            } if !titles.is_empty() || !descriptions.is_empty() => {
                let name = titles
                    .iter()
                    .filter(|title| !title.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("；");
                let value_text = descriptions
                    .iter()
                    .filter(|description| !description.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("；");
                AccessibilitySnapshot::named(
                    AccessibilityRole::Alert,
                    if name.is_empty() {
                        "Notification".to_owned()
                    } else {
                        name
                    },
                )
                .with_state(AccessibilityState {
                    value_text: (!value_text.is_empty()).then_some(value_text),
                    ..AccessibilityState::default()
                })
            }
            // 反馈 capability 启用时才匹配同步存在的气泡卡片快照变体。
            #[cfg(feature = "feedback")]
            Self::Popover {
                title,
                content,
                visible,
                ..
            } => AccessibilitySnapshot::named(
                AccessibilityRole::Button,
                first_non_empty([title, content, "Popover"]),
            )
            .with_state(AccessibilityState {
                expanded: Some(*visible),
                value_text: (!content.is_empty()).then_some(content.clone()),
                ..AccessibilityState::default()
            }),
            // 反馈 capability 启用时才匹配同步存在的气泡确认框快照变体。
            #[cfg(feature = "feedback")]
            Self::Popconfirm(popconfirm) => popconfirm_accessibility(
                &popconfirm.title,
                &popconfirm.confirm_text,
                &popconfirm.cancel_text,
                popconfirm.visible,
                popconfirm.focused_action,
            ),
            // 反馈 capability 启用时才匹配同步存在的对话框快照变体。
            #[cfg(feature = "feedback")]
            Self::Modal { title, open, .. } => {
                let role = if *open {
                    AccessibilityRole::Dialog
                } else {
                    AccessibilityRole::Button
                };
                let name = if *open && title.is_empty() {
                    "Modal".to_owned()
                } else if *open {
                    title.clone()
                } else if title.is_empty() {
                    "打开 Modal".to_owned()
                } else {
                    format!("打开 {title}")
                };
                AccessibilitySnapshot::named(role, name).with_state(AccessibilityState {
                    expanded: Some(*open),
                    ..AccessibilityState::default()
                })
            }
            // 反馈 capability 启用时才匹配同步存在的抽屉快照变体。
            #[cfg(feature = "feedback")]
            Self::Drawer { title, open, .. } => {
                let role = if *open {
                    AccessibilityRole::Dialog
                } else {
                    AccessibilityRole::Button
                };
                let name = if *open && title.is_empty() {
                    "Drawer".to_owned()
                } else if *open {
                    title.clone()
                } else if title.is_empty() {
                    "打开 Drawer".to_owned()
                } else {
                    format!("打开 {title}")
                };
                AccessibilitySnapshot::named(role, name).with_state(AccessibilityState {
                    expanded: Some(*open),
                    ..AccessibilityState::default()
                })
            }
            // 导航 capability 启用时才匹配同步存在的分页快照变体。
            #[cfg(feature = "navigation")]
            Self::Pagination {
                current,
                total,
                page_size,
                ..
            } => pagination_accessibility(*current, *total, *page_size),
            // 导航 capability 启用时才匹配同步存在的锚点快照变体。
            #[cfg(feature = "navigation")]
            Self::Anchor {
                items,
                active_index,
                ..
            } => anchor_accessibility(items, *active_index),
            // 导航 capability 启用时才匹配同步存在的面包屑快照变体。
            #[cfg(feature = "navigation")]
            Self::Breadcrumb { items, .. } => breadcrumb_accessibility(items),
            // 导航 capability 启用时才匹配同步存在的菜单快照变体。
            #[cfg(feature = "navigation")]
            Self::Menu {
                items, active_key, ..
            } => menu_accessibility(items, active_key),
            // 导航 capability 启用时才匹配同步存在的下拉菜单快照变体。
            #[cfg(feature = "navigation")]
            Self::Dropdown {
                label,
                items,
                open,
                selected_index,
                ..
            } => dropdown_accessibility(label, items, *open, *selected_index),
            // 导航 capability 启用时才匹配同步存在的菜单栏快照变体。
            #[cfg(feature = "navigation")]
            Self::MenuBar {
                menus, open_index, ..
            } => menu_bar_accessibility(menus, *open_index),
            // 导航 capability 启用时才匹配同步存在的标签页快照变体。
            #[cfg(feature = "navigation")]
            Self::Tabs {
                tabs, active_index, ..
            } => tabs_accessibility(tabs, *active_index),
            // 导航 capability 启用时才匹配同步存在的步骤条快照变体。
            #[cfg(feature = "navigation")]
            Self::Steps { steps, current, .. } => steps_accessibility(steps, *current),
            // 导航 capability 启用时才匹配同步存在的导航项快照变体。
            #[cfg(feature = "navigation")]
            Self::NavItem { label, active, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::Navigation, label.clone())
                    .with_state(AccessibilityState {
                        selected: Some(*active),
                        ..AccessibilityState::default()
                    })
            }
            // Navigation 外壳公开标题、折叠与可选版本事实。
            #[cfg(feature = "navigation")]
            Self::Navigation {
                title,
                version,
                collapsed,
            } => {
                // 侧栏整体使用 Navigation 角色并公开展开状态。
                AccessibilitySnapshot::named(AccessibilityRole::Navigation, title.clone())
                    // 版本作为可选人类可读值文本。
                    .with_state(AccessibilityState {
                        // collapsed=false 表示侧栏处于展开状态。
                        expanded: Some(!*collapsed),
                        // 精确保留调用方版本元数据。
                        value_text: version.clone(),
                        // 其余状态使用默认值。
                        ..AccessibilityState::default()
                    })
            }
            // 树组件 capability 启用时才转换展示树无障碍快照。
            #[cfg(feature = "tree-widgets")]
            Self::Tree {
                nodes,
                selected_key,
                expanded_keys,
                ..
            } => tree_accessibility(nodes, selected_key, expanded_keys),
            // 终端 capability 启用时才转换终端无障碍快照。
            #[cfg(feature = "terminal")]
            Self::Terminal {
                prompt,
                lines,
                input,
                ..
            } => terminal_accessibility(prompt, lines, input),
            Self::Calendar {
                year,
                month,
                selected,
                focused_day,
                ..
            } => calendar_accessibility(*year, *month, *selected, *focused_day),
            Self::Descriptions { title, items, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::Group, title.clone()).with_state(
                    AccessibilityState {
                        value_text: (!items.is_empty()).then(|| {
                            items
                                .iter()
                                .map(|item| format!("{}: {}", item.label, item.value))
                                .collect::<Vec<_>>()
                                .join("; ")
                        }),
                        ..AccessibilityState::default()
                    },
                )
            }
            Self::Collapse {
                panels,
                focused_header,
                ..
            } => {
                let focused_panel = panels.get(*focused_header);
                AccessibilitySnapshot::named(
                    AccessibilityRole::Button,
                    focused_panel
                        .map(|panel| panel.header.clone())
                        .unwrap_or_default(),
                )
                .with_state(AccessibilityState {
                    value_text: panels
                        .get(*focused_header)
                        .map(|panel| panel.header.clone()),
                    expanded: focused_panel.map(|panel| panel.expanded),
                    ..AccessibilityState::default()
                })
            }
            Self::Carousel {
                current,
                slide_count,
                ..
            } => carousel_accessibility(*current, *slide_count),
            // 树组件 capability 启用时才转换树选择器无障碍快照。
            #[cfg(feature = "tree-widgets")]
            Self::TreeSelect {
                placeholder,
                value,
                open,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    expanded: Some(*open),
                    value_text: (!value.is_empty()).then(|| value.clone()),
                    ..AccessibilityState::default()
                }),
            Self::SelectableList {
                items,
                active_index,
                ..
            } => selectable_list_accessibility(items, *active_index),
            Self::List { header, items, .. } => {
                AccessibilitySnapshot::named(AccessibilityRole::List, header.clone()).with_state(
                    AccessibilityState {
                        value_text: (!items.is_empty()).then(|| items.join("; ")),
                        ..AccessibilityState::default()
                    },
                )
            }
            Self::Card {
                title,
                actions,
                focused_action,
                ..
            } => card_accessibility(title.as_deref(), actions, *focused_action),
            Self::Transfer { source, target } => transfer_accessibility(source, target),
            Self::Upload { files, .. } => upload_accessibility(files),
            // 富文本 capability 启用时才匹配同步存在的快照变体。
            #[cfg(feature = "rich-text")]
            Self::RichText {
                segments,
                focused_link,
                ..
            } => rich_text_accessibility(segments, *focused_link),
            // 表格 capability 启用时才匹配同步存在的表格快照变体。
            #[cfg(feature = "table")]
            Self::Table { .. } => AccessibilitySnapshot::new(AccessibilityRole::Table),
            // 图表 capability 启用时才匹配柱状图快照变体。
            #[cfg(feature = "charts")]
            Self::BarChart { data, .. } => chart_accessibility(
                "Bar chart",
                data.iter().map(|item| (item.label.as_str(), item.value)),
            ),
            // 图表 capability 启用时才匹配折线图快照变体。
            #[cfg(feature = "charts")]
            Self::LineChart { data, .. } => chart_accessibility(
                "Line chart",
                data.iter().map(|item| (item.label.as_str(), item.value)),
            ),
            // 图表 capability 启用时才匹配饼图快照变体。
            #[cfg(feature = "charts")]
            Self::PieChart { data, .. } => chart_accessibility(
                "Pie chart",
                data.iter()
                    .filter(|item| item.value.is_finite() && item.value > 0.0)
                    .map(|item| (item.label.as_str(), item.value)),
            ),
            // 图表 capability 启用时才匹配高级图表快照变体。
            #[cfg(feature = "charts")]
            Self::ChartPlaceholder {
                title, kind_name, ..
            } => {
                let name = if title.trim().is_empty() {
                    kind_name.to_string()
                } else {
                    title.clone()
                };
                AccessibilitySnapshot::named(AccessibilityRole::Image, name)
            }
            Self::Select {
                options,
                optgroups,
                selected,
                selected_multi,
                placeholder,
                disabled,
                multiple,
                open,
                search_query,
                ..
            } => select_accessibility(
                options,
                optgroups,
                *selected,
                selected_multi,
                placeholder,
                *disabled,
                *multiple,
                *open,
                search_query,
            ),
            Self::DatePicker {
                placeholder,
                value,
                open,
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    expanded: Some(*open),
                    value_text: value.clone(),
                    ..AccessibilityState::default()
                }),
            Self::DateRangePicker {
                placeholder,
                start,
                end,
                open,
            } => date_range_accessibility(placeholder, start.as_ref(), end.as_ref(), *open),
            Self::TimePicker {
                placeholder,
                value,
                open,
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    value_text: value.clone(),
                    expanded: Some(*open),
                    ..AccessibilityState::default()
                }),
            Self::ColorPicker { value, open, .. } => AccessibilitySnapshot::new(
                AccessibilityRole::Combobox,
            )
            .with_state(AccessibilityState {
                value_text: Some(value.to_string()),
                expanded: Some(*open),
                ..AccessibilityState::default()
            }),
            Self::AutoComplete {
                placeholder,
                value,
                open,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    expanded: Some(*open),
                    value_text: (!value.is_empty()).then(|| value.clone()),
                    ..AccessibilityState::default()
                }),
            Self::Mentions {
                placeholder,
                value,
                suggesting,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    expanded: Some(*suggesting),
                    value_text: (!value.is_empty()).then(|| value.clone()),
                    ..AccessibilityState::default()
                }),
            Self::Cascader {
                placeholder,
                selected_labels,
                open,
                ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder.clone())
                .with_state(AccessibilityState {
                    expanded: Some(*open),
                    value_text: (!selected_labels.is_empty()).then(|| selected_labels.join(" / ")),
                    ..AccessibilityState::default()
                }),
            Self::Segmented {
                disabled,
                options,
                selected,
                ..
            } => AccessibilitySnapshot::new(AccessibilityRole::RadioGroup).with_state(
                AccessibilityState {
                    disabled: *disabled,
                    value_text: options.get(*selected).cloned(),
                    ..AccessibilityState::default()
                },
            ),
            Self::FormItem {
                label, required, ..
            } => AccessibilitySnapshot::named(AccessibilityRole::Group, label.clone()).with_state(
                AccessibilityState {
                    required: *required,
                    ..AccessibilityState::default()
                },
            ),
            Self::Result {
                result_type,
                title,
                subtitle,
                extra_text,
                ..
            } => result_accessibility(*result_type, title, subtitle, extra_text),
            // 反馈 capability 启用时才匹配同步存在的加载指示器快照变体。
            #[cfg(feature = "feedback")]
            Self::Spin { tip, spinning, .. } => AccessibilitySnapshot::named(
                AccessibilityRole::Status,
                if tip.is_empty() {
                    if *spinning { "加载中" } else { "加载" }.to_string()
                } else {
                    tip.clone()
                },
            ),
            Self::ThemeToggle { dark } => theme_toggle_accessibility(*dark),
            Self::BackTop { visible, .. } => back_top_accessibility(*visible),
            _ => AccessibilitySnapshot::new(AccessibilityRole::Generic),
        }
    }
}
