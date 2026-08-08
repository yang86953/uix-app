//! 组件级视觉测试清单；每个条目必须有独立测试场景。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseKind {
    Widget,
    Composite,
    Provider,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentVisualCase {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub kind: CaseKind,
    pub states: &'static [&'static str],
}

const fn widget(
    id: &'static str,
    name: &'static str,
    category: &'static str,
    states: &'static [&'static str],
) -> ComponentVisualCase {
    ComponentVisualCase {
        id,
        name,
        category,
        kind: CaseKind::Widget,
        states,
    }
}

const fn composite(
    id: &'static str,
    name: &'static str,
    category: &'static str,
    states: &'static [&'static str],
) -> ComponentVisualCase {
    ComponentVisualCase {
        id,
        name,
        category,
        kind: CaseKind::Composite,
        states,
    }
}

const fn provider(
    id: &'static str,
    name: &'static str,
    states: &'static [&'static str],
) -> ComponentVisualCase {
    ComponentVisualCase {
        id,
        name,
        category: "Provider",
        kind: CaseKind::Provider,
        states,
    }
}

// 条目分组宏定义拆入独立文件，保持库存声明文件处于行数上限内。
#[macro_use]
#[path = "manifest_cases_core.rs"]
mod manifest_cases_core;
#[macro_use]
#[path = "manifest_cases_display.rs"]
mod manifest_cases_display;
#[macro_use]
#[path = "manifest_cases_extra.rs"]
mod manifest_cases_extra;

// 分组库存由独立文件宏展开，避免单一文件超出可审阅尺寸。
pub const CASES_CORE: &[ComponentVisualCase] = manifest_cases_core!();
pub const CASES_DISPLAY: &[ComponentVisualCase] = manifest_cases_display!();
pub const CASES_EXTRA: &[ComponentVisualCase] = manifest_cases_extra!();

// 总清单按分组展平为二维常量，页面与测试结果按线性索引访问。
pub const COMPONENT_VISUAL_CASES: &[&[ComponentVisualCase]] =
    &[CASES_CORE, CASES_DISPLAY, CASES_EXTRA];

/// 组件测试的唯一冻结分母；页面、无窗测试与真窗测试都从 manifest 推导。
pub const COMPONENT_VISUAL_CASE_COUNT: usize =
    CASES_CORE.len() + CASES_DISPLAY.len() + CASES_EXTRA.len();
