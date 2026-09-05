use crate::ui::widgets::{
    BadgeStatus, Date, OptGroup, ResultType, SelectableItem, TimelineItem, TypographyType,
    UploadFile, UploadStatus,
};
// 反馈 capability 启用时才引入进度快照专属模式类型。
#[cfg(feature = "feedback")]
// 该类型只服务同步门控的进度条无障碍转换。
use crate::ui::widgets::ProgressMode;
// 导航 capability 启用时才引入专属条目模型。
#[cfg(feature = "navigation")]
// 这些类型仅供同步门控的导航无障碍转换使用。
use crate::ui::widgets::{AnchorItem, BreadcrumbItem, DropdownItem, MenuItem, Step, Tab};
// 富文本 capability 启用时才引入其内容模型。
#[cfg(feature = "rich-text")]
// 该类型仅供富文本无障碍快照转换使用。
use crate::ui::widgets::RichTextSegment;

use super::{
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AriaAttribute,
    SnapshotTransferItem,
};
// 树组件 capability 启用时才引入树节点快照模型。
#[cfg(feature = "tree-widgets")]
// 该类型只服务同步门控的树无障碍转换。
use super::SnapshotTreeNode;

pub(super) fn timeline_accessibility(
    items: &[TimelineItem],
    pending: bool,
    reverse: bool,
) -> AccessibilitySnapshot {
    let mut entries = Vec::with_capacity(items.len() + usize::from(pending));
    for index in 0..items.len() {
        let item = if reverse {
            &items[items.len() - 1 - index]
        } else {
            &items[index]
        };
        let label = item.label.trim();
        let description = item.description.trim();
        let entry = match (label.is_empty(), description.is_empty()) {
            (false, false) => format!("{label}: {description}"),
            (false, true) => label.to_string(),
            (true, false) => description.to_string(),
            (true, true) => continue,
        };
        entries.push(entry);
    }
    if pending {
        entries.push(
            crate::ui::widget_runtime::locale::use_locale()
                .timeline_pending
                .to_string(),
        );
    }
    AccessibilitySnapshot::new(AccessibilityRole::List).with_state(AccessibilityState {
        value_text: (!entries.is_empty()).then(|| entries.join("; ")),
        ..AccessibilityState::default()
    })
}

// 树组件 capability 关闭时不编译专属无障碍转换逻辑。
#[cfg(feature = "tree-widgets")]
pub(super) fn tree_accessibility(
    nodes: &[SnapshotTreeNode],
    selected_key: &str,
    expanded_keys: &[String],
) -> AccessibilitySnapshot {
    fn append_visible<'a>(
        nodes: &'a [SnapshotTreeNode],
        expanded_keys: &[String],
        visible: &mut Vec<&'a SnapshotTreeNode>,
    ) {
        for node in nodes {
            visible.push(node);
            if expanded_keys.iter().any(|key| key == &node.key) {
                append_visible(&node.children, expanded_keys, visible);
            }
        }
    }

    let mut visible = Vec::new();
    append_visible(nodes, expanded_keys, &mut visible);
    let selected_index = visible.iter().position(|node| node.key == selected_key);
    AccessibilitySnapshot::new(AccessibilityRole::Tree).with_state(AccessibilityState {
        value_text: selected_index.map(|index| visible[index].title.clone()),
        value_now: selected_index.map(|index| (index + 1) as f64),
        value_min: (!visible.is_empty()).then_some(1.0),
        value_max: (!visible.is_empty()).then_some(visible.len() as f64),
        ..AccessibilityState::default()
    })
}

// 终端 capability 关闭时不编译终端无障碍转换逻辑。
#[cfg(feature = "terminal")]
pub(super) fn terminal_accessibility(
    prompt: &str,
    lines: &[String],
    input: &str,
) -> AccessibilitySnapshot {
    // 无障碍观察通道有界：只保留最近 8 行非空输出与当前输入。
    const MAX_TAIL_LINES: usize = 8;
    let start = lines.len().saturating_sub(MAX_TAIL_LINES);
    let mut entries: Vec<&str> = lines[start..]
        .iter()
        .map(String::as_str)
        .filter(|line| !line.trim().is_empty())
        .collect();
    let input = input.trim();
    if !input.is_empty() {
        entries.push(input);
    }
    AccessibilitySnapshot::new(AccessibilityRole::List).with_state(AccessibilityState {
        value_text: (!entries.is_empty())
            .then(|| entries.join("; "))
            .or_else(|| (!prompt.trim().is_empty()).then(|| prompt.trim().to_string())),
        ..AccessibilityState::default()
    })
}

// 真实终端屏幕的无障碍快照：网格行数本身有界，保留全部非空行，
// 超出上限时取尾部；行文本去掉网格尾随空格。
#[cfg(feature = "terminal")]
pub(super) fn terminal_screen_accessibility(
    rows: &[String],
    running: bool,
    exit_code: Option<i32>,
) -> AccessibilitySnapshot {
    const MAX_LINES: usize = 64;
    let mut entries: Vec<&str> = rows
        .iter()
        .map(|line| line.trim_end())
        .filter(|line| !line.is_empty())
        .collect();
    if entries.len() > MAX_LINES {
        entries.drain(0..entries.len() - MAX_LINES);
    }
    let state_text = if running {
        None
    } else {
        exit_code.map(|code| format!("会话已退出，代码 {code}"))
    };
    AccessibilitySnapshot::new(AccessibilityRole::TextBox).with_state(AccessibilityState {
        value_text: (!entries.is_empty())
            .then(|| entries.join("\n"))
            .or(state_text),
        ..AccessibilityState::default()
    })
}

// 图表 capability 关闭时不编译专属无障碍摘要辅助函数。
#[cfg(feature = "charts")]
pub(super) fn chart_accessibility<'a>(
    name: &'static str,
    values: impl Iterator<Item = (&'a str, f32)>,
) -> AccessibilitySnapshot {
    let value_text = values
        .map(|(label, value)| {
            let value = if value.is_finite() { value } else { 0.0 };
            let value = if value == value.trunc() {
                format!("{value:.0}")
            } else {
                format!("{value:.1}")
            };
            if label.trim().is_empty() {
                value
            } else {
                format!("{}: {value}", label.trim())
            }
        })
        .collect::<Vec<_>>();
    AccessibilitySnapshot::named(AccessibilityRole::Image, name).with_state(AccessibilityState {
        value_text: (!value_text.is_empty()).then(|| value_text.join("; ")),
        ..AccessibilityState::default()
    })
}

pub(super) fn avatar_accessibility(text: &str, src: &str) -> AccessibilitySnapshot {
    AccessibilitySnapshot::named(
        AccessibilityRole::Image,
        first_non_empty([text, src, "Avatar"]),
    )
}

// 导航 capability 启用时才编译步骤条无障碍转换。
#[cfg(feature = "navigation")]
pub(super) fn steps_accessibility(steps: &[Step], current: usize) -> AccessibilitySnapshot {
    AccessibilitySnapshot::new(AccessibilityRole::Navigation).with_state(AccessibilityState {
        value_text: steps.get(current).map(|step| step.title.clone()),
        value_now: Some((current + 1) as f64),
        value_min: Some(1.0),
        value_max: Some(steps.len().max(1) as f64),
        ..AccessibilityState::default()
    })
}

pub(super) fn card_accessibility(
    title: Option<&str>,
    actions: &[String],
    focused_action: Option<usize>,
) -> AccessibilitySnapshot {
    let snapshot = if let Some(title) = title.filter(|title| !title.trim().is_empty()) {
        AccessibilitySnapshot::named(AccessibilityRole::Group, title)
    } else {
        AccessibilitySnapshot::new(AccessibilityRole::Group)
    };
    snapshot.with_state(AccessibilityState {
        value_text: focused_action.and_then(|index| actions.get(index)).cloned(),
        value_now: focused_action.map(|index| (index + 1) as f64),
        value_min: (!actions.is_empty()).then_some(1.0),
        value_max: (!actions.is_empty()).then_some(actions.len() as f64),
        ..AccessibilityState::default()
    })
}

// 导航 capability 启用时才编译菜单无障碍转换。
#[cfg(feature = "navigation")]
pub(super) fn menu_accessibility(items: &[MenuItem], active_key: &str) -> AccessibilitySnapshot {
    AccessibilitySnapshot::new(AccessibilityRole::Menu).with_state(AccessibilityState {
        value_text: items
            .iter()
            .find(|item| item.key == active_key)
            .map(|item| item.label.clone()),
        ..AccessibilityState::default()
    })
}

// 导航 capability 启用时才编译下拉菜单无障碍转换。
#[cfg(feature = "navigation")]
pub(super) fn dropdown_accessibility(
    label: &str,
    items: &[DropdownItem],
    open: bool,
    selected_index: Option<usize>,
) -> AccessibilitySnapshot {
    // Dropdown is a focusable trigger that opens a menu, not the menu surface
    // itself. Exposing the trigger as a button keeps semantic Invoke aligned
    // with its Enter/Space activation path while expanded reports popup state.
    AccessibilitySnapshot::named(AccessibilityRole::Button, label).with_state(AccessibilityState {
        expanded: Some(open),
        // 无障碍值继续公开展示 label，稳定 key 只承担内部身份。
        value_text: selected_index
            // 查找当前可见选择项。
            .and_then(|index| items.get(index))
            // 克隆人类可读的展示文本。
            .map(|item| item.label.clone()),
        ..AccessibilityState::default()
    })
}

// 导航 capability 启用时才编译菜单栏无障碍转换。
#[cfg(feature = "navigation")]
pub(super) fn menu_bar_accessibility(
    menus: &[crate::ui::widgets::MenuBarMenu],
    open_index: Option<usize>,
) -> AccessibilitySnapshot {
    // MenuBar 复用 Menu 容器角色；expanded 反映当前是否有菜单展开。
    let label = menus
        .iter()
        .map(|menu| menu.label.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    AccessibilitySnapshot::named(AccessibilityRole::Menu, label).with_state(AccessibilityState {
        expanded: Some(open_index.is_some()),
        ..AccessibilityState::default()
    })
}

// 反馈 capability 启用时才编译气泡确认框无障碍转换。
#[cfg(feature = "feedback")]
pub(super) fn popconfirm_accessibility(
    title: &str,
    confirm_text: &str,
    cancel_text: &str,
    visible: bool,
    focused_action: Option<usize>,
) -> AccessibilitySnapshot {
    let action = match focused_action {
        Some(0) => Some(confirm_text),
        Some(1) => Some(cancel_text),
        _ => None,
    };
    AccessibilitySnapshot::named(
        AccessibilityRole::Button,
        first_non_empty([title, "Confirm action"]),
    )
    .with_state(AccessibilityState {
        expanded: Some(visible),
        value_text: action.map(str::to_owned),
        value_now: action.and_then(|_| focused_action.map(|index| index as f64 + 1.0)),
        value_min: visible.then_some(1.0),
        value_max: visible.then_some(2.0),
        ..AccessibilityState::default()
    })
}

// 导航 capability 启用时才编译标签页无障碍转换。
#[cfg(feature = "navigation")]
pub(super) fn tabs_accessibility(tabs: &[Tab], active_index: usize) -> AccessibilitySnapshot {
    AccessibilitySnapshot::new(AccessibilityRole::TabList).with_state(AccessibilityState {
        value_text: tabs.get(active_index).map(|tab| tab.label.clone()),
        // 活动标签位置以 1 起计数（与 Breadcrumb 的 value_now 语义一致）。
        value_now: (!tabs.is_empty()).then_some((active_index + 1).min(tabs.len()) as f64),
        value_min: (!tabs.is_empty()).then_some(1.0),
        value_max: (!tabs.is_empty()).then_some(tabs.len() as f64),
        ..AccessibilityState::default()
    })
}

pub(super) fn upload_accessibility(files: &[UploadFile]) -> AccessibilitySnapshot {
    let value_text = files
        .iter()
        .map(|file| {
            let status = match file.status {
                UploadStatus::Pending => "pending".to_string(),
                UploadStatus::Uploading => format!("uploading {:.0}%", file.progress * 100.0),
                UploadStatus::Done => "done".to_string(),
                UploadStatus::Error => "error".to_string(),
            };
            format!("{}: {status}", file.name)
        })
        .collect::<Vec<_>>();
    AccessibilitySnapshot::named(AccessibilityRole::List, "Upload queue").with_state(
        AccessibilityState {
            value_text: (!value_text.is_empty()).then(|| value_text.join("; ")),
            ..AccessibilityState::default()
        },
    )
}

// 富文本 capability 关闭时不编译专属无障碍转换逻辑。
#[cfg(feature = "rich-text")]
pub(super) fn rich_text_accessibility(
    segments: &[RichTextSegment],
    focused_link: Option<usize>,
) -> AccessibilitySnapshot {
    // 图片能力开启时，单一图片 RichText 直接暴露图片语义与 alt 名称。
    #[cfg(feature = "image-codecs")]
    if segments
        // 必须且只能包含一个图片段。
        .iter()
        // 统计图片段数量。
        .filter(|segment| matches!(segment, RichTextSegment::Image { .. }))
        // 单图片组件才能映射为唯一 Image 节点。
        .count()
        == 1
        // 其他段只能是该图片源码行的换行。
        && segments.iter().all(|segment| {
            // 拒绝把混合正文吞并进图片节点。
            matches!(segment, RichTextSegment::Image { .. } | RichTextSegment::NewLine)
        })
    {
        // 从唯一图片段提取替代文本。
        let alt = segments.iter().find_map(|segment| match segment {
            // 返回图片的公开 alt。
            RichTextSegment::Image { alt, .. } => Some(alt.clone()),
            // 源码换行没有图片名称。
            _ => None,
        });
        // 构造标准图片语义快照，named 会把空 alt 规范为无名称。
        return AccessibilitySnapshot::named(
            // 使用标准图片角色。
            AccessibilityRole::Image,
            // 唯一图片分支必定拥有 alt 字段。
            alt.unwrap_or_default(),
        );
    }
    // 单一组件只含主题分隔线和其源码换行时，直接暴露 separator 语义。
    let is_separator = segments
        // 遍历完整公开段列表。
        .iter()
        // 至少一个主题分隔线才能形成 separator 节点。
        .any(|segment| matches!(segment, RichTextSegment::ThematicBreak))
        // 同时要求不存在会被一个节点吞并的可见文本或链接。
        && segments.iter().all(|segment| {
            // 分隔线自身与紧随源码换行都属于同一块语义。
            matches!(
                segment,
                RichTextSegment::ThematicBreak | RichTextSegment::NewLine
            )
        });
    // 分隔线节点无名称、无动作，也不会进入 Tab 顺序。
    if is_separator {
        // 使用新增公开角色形成稳定语义快照。
        return AccessibilitySnapshot::new(AccessibilityRole::Separator);
    }
    let plain_text = segments
        .iter()
        .map(|segment| match segment {
            RichTextSegment::Text { content, .. }
            | RichTextSegment::Code { content }
            | RichTextSegment::Link { content, .. } => content.as_str(),
            // 混合文档把图片 alt 纳入单一文本节点名称。
            #[cfg(feature = "image-codecs")]
            RichTextSegment::Image { alt, .. } => alt.as_str(),
            RichTextSegment::ThematicBreak => "",
            RichTextSegment::NewLine => "\n",
        })
        .collect::<String>();
    let links = segments
        .iter()
        .filter_map(|segment| match segment {
            RichTextSegment::Link { content, url } if !url.trim().is_empty() => {
                Some((content.as_str(), url.as_str()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let Some((content, url)) = focused_link.and_then(|index| links.get(index)).copied() else {
        return AccessibilitySnapshot::named(AccessibilityRole::Text, plain_text);
    };
    AccessibilitySnapshot::named(
        AccessibilityRole::Button,
        if content.trim().is_empty() {
            url
        } else {
            content
        },
    )
    .with_state(AccessibilityState {
        value_text: Some(url.to_string()),
        ..AccessibilityState::default()
    })
}

pub(super) fn typography_accessibility(
    content: &str,
    type_: TypographyType,
    disabled: bool,
    copyable: bool,
) -> AccessibilitySnapshot {
    let heading_level = match type_ {
        TypographyType::Heading1 => Some(1),
        TypographyType::Heading2 => Some(2),
        TypographyType::Heading3 => Some(3),
        TypographyType::Heading4 => Some(4),
        TypographyType::Heading5 => Some(5),
        TypographyType::Paragraph | TypographyType::Text => None,
    };
    let role = if heading_level.is_some() {
        AccessibilityRole::Heading
    } else if copyable {
        AccessibilityRole::Button
    } else {
        AccessibilityRole::Text
    };
    let mut snapshot = AccessibilitySnapshot::named(role, content)
        .with_state(AccessibilityState::disabled(disabled));
    if let Some(level) = heading_level {
        snapshot = snapshot.with_attribute(AriaAttribute::new("aria-level", level.to_string()));
    }
    snapshot
}

pub(super) fn transfer_accessibility(
    source: &[SnapshotTransferItem],
    target: &[SnapshotTransferItem],
) -> AccessibilitySnapshot {
    let selected = source
        .iter()
        .chain(target)
        .filter(|item| item.selected)
        .count();
    AccessibilitySnapshot::new(AccessibilityRole::List).with_state(AccessibilityState {
        value_text: Some(format!(
            "{} source; {} target; {selected} selected",
            source.len(),
            target.len()
        )),
        ..AccessibilityState::default()
    })
}

pub(super) fn splitter_accessibility(
    ratios: &[f32],
    active_handle: usize,
) -> AccessibilitySnapshot {
    let boundary = ratios
        .iter()
        .take(active_handle.saturating_add(1))
        .copied()
        .sum::<f32>()
        .clamp(0.0, 1.0);
    AccessibilitySnapshot::new(AccessibilityRole::Slider).with_state(AccessibilityState {
        value_text: Some(
            ratios
                .iter()
                .map(|ratio| format!("{ratio:.6}"))
                .collect::<Vec<_>>()
                .join(","),
        ),
        value_now: Some(f64::from(boundary)),
        value_min: Some(0.0),
        value_max: Some(1.0),
        ..AccessibilityState::default()
    })
}

pub(super) fn carousel_accessibility(current: usize, slide_count: usize) -> AccessibilitySnapshot {
    AccessibilitySnapshot::new(AccessibilityRole::Group).with_state(AccessibilityState {
        value_text: (slide_count > 0).then(|| format!("Slide {} of {slide_count}", current + 1)),
        value_now: (slide_count > 0).then_some((current + 1) as f64),
        value_min: (slide_count > 0).then_some(1.0),
        value_max: (slide_count > 0).then_some(slide_count as f64),
        ..AccessibilityState::default()
    })
}

pub(super) fn calendar_accessibility(
    year: i32,
    month: usize,
    selected: Option<Date>,
    focused_day: usize,
) -> AccessibilitySnapshot {
    let focused = Date::new(year, month, focused_day);
    let value_text = selected.map_or_else(
        || focused.format(),
        |selected| format!("{}; focused {}", selected.format(), focused.format()),
    );
    AccessibilitySnapshot::new(AccessibilityRole::Group).with_state(AccessibilityState {
        value_text: Some(value_text),
        value_now: Some(focused.day as f64),
        value_min: Some(1.0),
        value_max: Some(crate::ui::widgets::input::date_picker::days_in_month(year, month) as f64),
        ..AccessibilityState::default()
    })
}

pub(super) fn theme_toggle_accessibility(dark: bool) -> AccessibilitySnapshot {
    AccessibilitySnapshot::named(AccessibilityRole::Button, "Theme").with_state(
        AccessibilityState {
            checked: Some(dark),
            value_text: Some(if dark { "dark" } else { "light" }.to_string()),
            ..AccessibilityState::default()
        },
    )
}

// 导航 capability 启用时才编译面包屑无障碍转换。
#[cfg(feature = "navigation")]
pub(super) fn breadcrumb_accessibility(items: &[BreadcrumbItem]) -> AccessibilitySnapshot {
    let active = items
        .iter()
        .find(|item| item.active)
        .or_else(|| items.first());
    AccessibilitySnapshot::new(AccessibilityRole::Navigation).with_state(AccessibilityState {
        value_text: active.map(|item| item.title.clone()),
        value_now: active
            .and_then(|active| items.iter().position(|item| item == active))
            .map(|index| (index + 1) as f64),
        value_min: (!items.is_empty()).then_some(1.0),
        value_max: (!items.is_empty()).then_some(items.len() as f64),
        ..AccessibilityState::default()
    })
}

// 导航 capability 启用时才编译分页无障碍转换。
#[cfg(feature = "navigation")]
pub(super) fn pagination_accessibility(
    current: usize,
    total: usize,
    page_size: usize,
) -> AccessibilitySnapshot {
    let page_count = total.div_ceil(page_size);
    AccessibilitySnapshot::new(AccessibilityRole::Navigation).with_state(AccessibilityState {
        value_text: Some(format!(
            "Page {current} of {page_count}; {page_size} per page"
        )),
        value_now: Some(current as f64),
        value_min: Some(1.0),
        value_max: Some(page_count.max(1) as f64),
        ..AccessibilityState::default()
    })
}

// 导航 capability 启用时才编译锚点无障碍转换。
#[cfg(feature = "navigation")]
pub(super) fn anchor_accessibility(
    items: &[AnchorItem],
    active_index: usize,
) -> AccessibilitySnapshot {
    AccessibilitySnapshot::new(AccessibilityRole::Navigation).with_state(AccessibilityState {
        value_text: items.get(active_index).map(|item| item.label.clone()),
        value_now: (!items.is_empty()).then_some((active_index + 1) as f64),
        value_min: (!items.is_empty()).then_some(1.0),
        value_max: (!items.is_empty()).then_some(items.len() as f64),
        ..AccessibilityState::default()
    })
}

pub(super) fn selectable_list_accessibility(
    items: &[SelectableItem],
    active_index: usize,
) -> AccessibilitySnapshot {
    // 仅有效活动索引才能提供选择文本与一基序号。
    let active = items.get(active_index);
    AccessibilitySnapshot::new(AccessibilityRole::List).with_state(AccessibilityState {
        // 空值或失效受控 id 不伪装成任何活动条目。
        value_text: active.map(|item| item.text.clone()),
        // 内部无选择哨兵不得溢出或泄漏为可访问性序号。
        value_now: active.map(|_| (active_index + 1) as f64),
        value_min: (!items.is_empty()).then_some(1.0),
        value_max: (!items.is_empty()).then_some(items.len() as f64),
        ..AccessibilityState::default()
    })
}

pub(super) fn back_top_accessibility(visible: bool) -> AccessibilitySnapshot {
    AccessibilitySnapshot::named(AccessibilityRole::Button, "Back to top").with_state(
        AccessibilityState {
            disabled: !visible,
            ..AccessibilityState::default()
        },
    )
}

pub(super) fn image_accessibility(
    alt: &str,
    fallback: &str,
    src: &str,
    preview: bool,
    preview_open: bool,
) -> AccessibilitySnapshot {
    let role = if preview {
        AccessibilityRole::Button
    } else {
        AccessibilityRole::Image
    };
    AccessibilitySnapshot::named(role, first_non_empty([alt, fallback, src])).with_state(
        AccessibilityState {
            expanded: preview.then_some(preview_open),
            ..AccessibilityState::default()
        },
    )
}

pub(super) fn badge_accessibility(
    count: i32,
    max: i32,
    dot: bool,
    status: Option<BadgeStatus>,
    show_zero: bool,
    text: &str,
) -> AccessibilitySnapshot {
    let visible =
        dot || status.is_some() || !text.is_empty() || count > 0 || (count == 0 && show_zero);
    if !visible {
        return AccessibilitySnapshot::new(AccessibilityRole::None);
    }

    // 纯视觉圆点没有可发布名称时不创建空播报节点。
    if dot && text.is_empty() && status.is_none() {
        // 真实子节点语义仍由组件树独立暴露。
        return AccessibilitySnapshot::new(AccessibilityRole::None);
    }

    let numeric = !dot && status.is_none() && text.is_empty();
    let name = if !text.is_empty() {
        text.to_owned()
    } else if numeric {
        format!("{}{}", count.min(max), if count > max { "+" } else { "" })
    } else if let Some(status) = status {
        // 无显式文字时为状态 marker 发布稳定状态名称。
        match status {
            // 成功状态使用简洁播报名称。
            BadgeStatus::Success => "成功".to_owned(),
            // 处理中状态使用简洁播报名称。
            BadgeStatus::Processing => "处理中".to_owned(),
            // 默认状态使用中性播报名称。
            BadgeStatus::Default => "状态".to_owned(),
            // 错误状态使用简洁播报名称。
            BadgeStatus::Error => "错误".to_owned(),
            // 警告状态使用简洁播报名称。
            BadgeStatus::Warning => "警告".to_owned(),
        }
    } else {
        String::new()
    };
    AccessibilitySnapshot::named(AccessibilityRole::Status, name).with_state(AccessibilityState {
        value_now: numeric.then_some(count as f64),
        value_min: numeric.then_some(0.0),
        value_max: numeric.then_some(max as f64),
        ..AccessibilityState::default()
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select_accessibility(
    plain_options: &[String],
    optgroups: &[OptGroup],
    selected: usize,
    selected_multi: &[usize],
    placeholder: &str,
    disabled: bool,
    multiple: bool,
    open: bool,
    search_query: &str,
) -> AccessibilitySnapshot {
    let options = if optgroups.is_empty() {
        plain_options.iter().map(String::as_str).collect::<Vec<_>>()
    } else {
        optgroups
            .iter()
            .flat_map(|group| group.options.iter().map(String::as_str))
            .collect()
    };
    let selected_value = if multiple {
        let selected = selected_multi
            .iter()
            .filter_map(|index| options.get(*index).copied())
            .collect::<Vec<_>>();
        (!selected.is_empty()).then(|| selected.join(", "))
    } else {
        options.get(selected).map(|option| (*option).to_owned())
    };
    let value_text = if !open || search_query.is_empty() {
        selected_value
    } else {
        Some(search_query.to_owned())
    };
    AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder).with_state(
        AccessibilityState {
            disabled,
            value_text,
            // 选项层打开时对外广播展开状态。
            expanded: Some(open),
            ..AccessibilityState::default()
        },
    )
}

pub(super) fn date_range_accessibility(
    placeholder: &str,
    start: Option<&String>,
    end: Option<&String>,
    open: bool,
) -> AccessibilitySnapshot {
    AccessibilitySnapshot::named(AccessibilityRole::Combobox, placeholder).with_state(
        AccessibilityState {
            expanded: Some(open),
            value_text: start
                .zip(end)
                .map(|(start, end)| format!("{start} / {end}")),
            ..AccessibilityState::default()
        },
    )
}

pub(super) fn input_number_accessibility(
    value: f64,
    min: f64,
    max: f64,
    placeholder: &str,
    disabled: bool,
    display_value: Option<&str>,
) -> AccessibilitySnapshot {
    AccessibilitySnapshot::named(AccessibilityRole::SpinButton, placeholder).with_state(
        AccessibilityState {
            disabled,
            value_now: Some(value),
            value_min: Some(min),
            value_max: Some(max),
            value_text: display_value.map(str::to_owned),
            ..AccessibilityState::default()
        },
    )
}

pub(super) fn result_accessibility(
    result_type: ResultType,
    title: &str,
    subtitle: &str,
    extra_text: &str,
) -> AccessibilitySnapshot {
    let title = if title.is_empty() {
        result_type.localized_title()
    } else {
        title
    };
    let subtitle = if subtitle.is_empty() {
        result_type.localized_subtitle()
    } else {
        subtitle
    };
    let action = !extra_text.is_empty();
    AccessibilitySnapshot::named(
        if action {
            AccessibilityRole::Button
        } else {
            AccessibilityRole::Status
        },
        if action {
            extra_text.to_owned()
        } else {
            title.to_owned()
        },
    )
    .with_state(AccessibilityState {
        value_text: if action {
            Some(
                [title, subtitle]
                    .into_iter()
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .join("; "),
            )
            .filter(|value| !value.is_empty())
        } else {
            (!subtitle.is_empty()).then(|| subtitle.to_owned())
        },
        ..AccessibilityState::default()
    })
}

pub(super) fn tag_accessibility(
    text: &str,
    closable: bool,
    checkable: bool,
    checked: bool,
) -> AccessibilitySnapshot {
    let role = if checkable {
        AccessibilityRole::Checkbox
    } else if closable {
        AccessibilityRole::Button
    } else {
        AccessibilityRole::Generic
    };
    AccessibilitySnapshot::named(role, text).with_state(AccessibilityState {
        checked: checkable.then_some(checked),
        ..AccessibilityState::default()
    })
}

// 反馈 capability 启用时才编译进度条无障碍转换。
#[cfg(feature = "feedback")]
pub(super) fn progress_accessibility(mode: ProgressMode) -> AccessibilitySnapshot {
    let state = match mode {
        ProgressMode::Determinate(progress) => {
            let progress = if progress.is_finite() {
                progress.clamp(0.0, 1.0)
            } else {
                0.0
            };
            AccessibilityState {
                value_now: Some(progress as f64),
                value_min: Some(0.0),
                value_max: Some(1.0),
                ..AccessibilityState::default()
            }
        }
        ProgressMode::Indeterminate => AccessibilityState::default(),
    };
    AccessibilitySnapshot::new(AccessibilityRole::ProgressBar).with_state(state)
}

pub(super) fn first_non_empty<const N: usize>(values: [&str; N]) -> String {
    values
        .into_iter()
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}
