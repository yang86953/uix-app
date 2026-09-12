//! 样式集合 — 旧 [`StyleSet`] 五态公开壳与 [`DeclaredStyleSet`]
//! 显式六态声明表示共享同一个逐字段解析引擎。
//!
//! 叠加次序固定为 hover → focus → focus-visible → active(pressed) →
//! checked → disabled；同一字段后层优先，未声明字段保留低层结果，
//! disabled 节点不进入 active。
//!
//! 公开兼容性：[`StyleSet`] 保持既有五个公开字段的名称与类型
//! （`normal: Style` 与 `Option<Style>` 状态层），旧完整结构体字面量
//! 构造与旧方法调用无需修改即可编译；解析经集中转换委托给
//! [`DeclaredStyleSet`]，不维护第二套解析引擎。旧 [`Style`] 状态层的
//! 声明性经由 `From<Style> for StyleDiff` 与旧 `Style::apply` 值推断
//! 一致；需要显式恢复零、false、none、auto 的声明应改用
//! [`DeclaredStyleSet`] 与 [`StyleDiff`]。

use super::Style;
use super::StyleDiff;
use super::{ColorValue, TypographyToken};
use crate::core::EdgeInsets;

/// 组件交互态，用于从样式集合中解析最终样式。
///
/// 保留既有四字段公开形状；键盘焦点可见与选中事实经由
/// [`StateFlags`] 表达，[`StyleState`] 值可直接传入两个样式集合的
/// `resolve` 并等价于这两个事实为假。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StyleState {
    /// 指示指针当前是否悬停在组件上。
    pub hovered: bool,
    /// 指示组件当前是否被按压（active 层的事实来源）。
    pub pressed: bool,
    /// 指示组件当前是否拥有输入焦点（任意输入方式）。
    pub focused: bool,
    /// 指示组件当前是否不可交互。
    pub disabled: bool,
}

/// 六态交互事实标志；[`StyleState`] 的加法超集。
///
/// 新代码直接构造本类型以表达 focus-visible 与 checked 事实；
/// 旧 [`StyleState`] 通过 [`From`] 转换为这两个事实为假的六态值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StateFlags {
    /// 指示指针当前是否悬停在组件上。
    pub hovered: bool,
    /// 指示组件当前是否被按压（active 层的事实来源）。
    pub pressed: bool,
    /// 指示组件当前是否拥有输入焦点（任意输入方式）。
    pub focused: bool,
    /// 指示焦点是否来自键盘等应显露焦点指示的输入方式。
    pub focus_visible: bool,
    /// 指示有选中事实的控件当前是否处于选中态。
    pub checked: bool,
    /// 指示组件当前是否不可交互。
    pub disabled: bool,
}

impl From<StyleState> for StateFlags {
    fn from(state: StyleState) -> Self {
        Self {
            hovered: state.hovered,
            pressed: state.pressed,
            focused: state.focused,
            focus_visible: false,
            checked: false,
            disabled: state.disabled,
        }
    }
}

/// 显式六态声明样式集合 — 唯一的逐字段解析引擎。
///
/// 状态层只保存显式声明的差异字段（[`StyleDiff`]），解析时按固定次序
/// 逐字段叠加到 `normal` 上；未声明字段继承较低层结果。内部消费与
/// 新增入口使用本类型；旧 [`StyleSet`] 只做边界适配。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeclaredStyleSet {
    /// 未命中任何交互态时使用的基础样式。
    pub normal: Style,
    /// 悬停态叠加到基础样式上的差异样式。
    pub hover: Option<StyleDiff>,
    /// 焦点态叠加到基础样式上的差异样式（任意输入方式）。
    pub focus: Option<StyleDiff>,
    /// 键盘焦点态叠加到基础样式上的差异样式。
    pub focus_visible: Option<StyleDiff>,
    /// 按压/激活态叠加到基础样式上的差异样式。
    pub active: Option<StyleDiff>,
    /// 选中态叠加到基础样式上的差异样式。
    pub checked: Option<StyleDiff>,
    /// 禁用态叠加到基础样式上的差异样式。
    pub disabled: Option<StyleDiff>,
}

impl DeclaredStyleSet {
    /// 使用给定的基础样式创建不含交互态覆盖的样式集合。
    pub fn new(base: Style) -> Self {
        Self {
            normal: base,
            ..Self::default()
        }
    }

    /// 设置悬停态的差异样式。
    ///
    /// 接受 [`StyleDiff`] 或完整 [`Style`]；完整样式的声明性与旧
    /// `Style::apply` 值推断一致，旧调用行为不变。
    pub fn hover(mut self, s: impl Into<StyleDiff>) -> Self {
        self.hover = Some(s.into());
        self
    }

    /// 设置焦点态的差异样式（任意输入方式）。
    pub fn focus(mut self, s: impl Into<StyleDiff>) -> Self {
        self.focus = Some(s.into());
        self
    }

    /// 设置键盘焦点态的差异样式。
    pub fn focus_visible(mut self, s: impl Into<StyleDiff>) -> Self {
        self.focus_visible = Some(s.into());
        self
    }

    /// 设置按压/激活态的差异样式。
    pub fn active(mut self, s: impl Into<StyleDiff>) -> Self {
        self.active = Some(s.into());
        self
    }

    /// 将按压态设置为激活层的别名。
    pub fn pressed(self, s: impl Into<StyleDiff>) -> Self {
        self.active(s)
    }

    /// 兼容旧名称的焦点层设置；等价于 [`Self::focus`]。
    pub fn focused(self, s: impl Into<StyleDiff>) -> Self {
        self.focus(s)
    }

    /// 设置选中态的差异样式。
    pub fn checked(mut self, s: impl Into<StyleDiff>) -> Self {
        self.checked = Some(s.into());
        self
    }

    /// 设置禁用态的差异样式。
    pub fn disabled(mut self, s: impl Into<StyleDiff>) -> Self {
        self.disabled = Some(s.into());
        self
    }

    /// 按 hover → focus → focus-visible → active → checked → disabled
    /// 的次序逐字段叠加解析最终样式。
    ///
    /// 每层只覆盖自己显式声明的字段，未声明字段保留较低层结果；
    /// disabled 节点不进入 active。接受六态 [`StateFlags`] 或既有
    /// 四态 [`StyleState`]。
    pub fn resolve(&self, state: impl Into<StateFlags>) -> Style {
        let state = state.into();
        let mut style = self.normal.clone();
        if state.hovered
            && let Some(diff) = &self.hover
        {
            diff.clone().apply_to(&mut style);
        }
        if state.focused
            && let Some(diff) = &self.focus
        {
            diff.clone().apply_to(&mut style);
        }
        if state.focus_visible
            && let Some(diff) = &self.focus_visible
        {
            diff.clone().apply_to(&mut style);
        }
        if state.pressed
            && !state.disabled
            && let Some(diff) = &self.active
        {
            diff.clone().apply_to(&mut style);
        }
        if state.checked
            && let Some(diff) = &self.checked
        {
            diff.clone().apply_to(&mut style);
        }
        if state.disabled
            && let Some(diff) = &self.disabled
        {
            diff.clone().apply_to(&mut style);
        }
        style
    }

    /// 根据各交互态标志解析最终样式。
    ///
    /// 当多个标志同时为真时，采用与 [`Self::resolve`] 相同的逐字段叠加次序。
    pub fn resolve_flags(
        &self,
        hovered: bool,
        pressed: bool,
        focused: bool,
        disabled: bool,
    ) -> Style {
        self.resolve(StyleState {
            hovered,
            pressed,
            focused,
            disabled,
        })
    }
}

impl From<&StyleSet> for DeclaredStyleSet {
    /// 把旧五态壳集中转换为显式六态声明；完整 [`Style`] 状态层的
    /// 声明性与旧 `Style::apply` 值推断一致，解析结果逐字段不变。
    fn from(set: &StyleSet) -> Self {
        Self {
            normal: set.normal.clone(),
            hover: set.hover.clone().map(StyleDiff::from),
            focus: set.focused.clone().map(StyleDiff::from),
            focus_visible: None,
            active: set.pressed.clone().map(StyleDiff::from),
            checked: None,
            disabled: set.disabled.clone().map(StyleDiff::from),
        }
    }
}

/// 五态样式集合 — 旧公开兼容壳。
///
/// 字段名称与类型保持既有公开形状（完整 [`Style`] 状态层），旧完整
/// 结构体字面量构造与旧方法调用不变；解析委托给
/// [`DeclaredStyleSet`] 的唯一逐字段引擎。需要 focus-visible、
/// checked 或显式零/none/auto 声明的调用应改用 [`DeclaredStyleSet`]。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleSet {
    /// 未命中任何交互态时使用的基础样式。
    pub normal: Style,
    /// 悬停态叠加到基础样式上的差异样式。
    pub hover: Option<Style>,
    /// 按压态叠加到基础样式上的差异样式。
    pub pressed: Option<Style>,
    /// 焦点态叠加到基础样式上的差异样式。
    pub focused: Option<Style>,
    /// 禁用态叠加到基础样式上的差异样式。
    pub disabled: Option<Style>,
}

impl StyleSet {
    /// 使用给定的基础样式创建不含交互态覆盖的样式集合。
    pub fn new(base: Style) -> Self {
        Self {
            normal: base,
            ..Self::default()
        }
    }

    /// 设置悬停态的差异样式。
    pub fn hover(mut self, s: Style) -> Self {
        self.hover = Some(s);
        self
    }

    /// 设置按压态的差异样式。
    pub fn pressed(mut self, s: Style) -> Self {
        self.pressed = Some(s);
        self
    }

    /// 设置焦点态的差异样式。
    pub fn focused(mut self, s: Style) -> Self {
        self.focused = Some(s);
        self
    }

    /// 设置禁用态的差异样式。
    pub fn disabled(mut self, s: Style) -> Self {
        self.disabled = Some(s);
        self
    }

    /// 将活动态作为按压态的别名进行设置。
    pub fn active(self, s: Style) -> Self {
        self.pressed(s)
    }

    /// 按固定次序逐字段叠加解析最终样式。
    ///
    /// 完整 [`Style`] 状态层先经集中转换取得与旧 `Style::apply` 值推断
    /// 一致的声明性，再交给 [`DeclaredStyleSet`] 的唯一引擎解析；
    /// disabled 节点不进入 active。接受六态 [`StateFlags`] 或既有
    /// 四态 [`StyleState`]。
    pub fn resolve(&self, state: impl Into<StateFlags>) -> Style {
        DeclaredStyleSet::from(self).resolve(state)
    }

    /// 根据各交互态标志解析最终样式。
    ///
    /// 当多个标志同时为真时，采用与 [`Self::resolve`] 相同的叠加次序。
    pub fn resolve_flags(
        &self,
        hovered: bool,
        pressed: bool,
        focused: bool,
        disabled: bool,
    ) -> Style {
        self.resolve(StyleState {
            hovered,
            pressed,
            focused,
            disabled,
        })
    }
}
