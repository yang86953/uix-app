// ============================================================================
// ui/api.rs — UI 层的公共 API 出口
//
// 本文件定义 ui 层对外暴露的公共接口。其他层只能通过本文件
// 使用 ui 层的功能，禁止直接引用内部模块。
// ============================================================================

// ── 控件核心 ──
pub use crate::ui::widget::{
    AsAny, BoxedWidget, EventResult, IntoWidgetNode, Widget, WidgetCore, WidgetEvent, WidgetId,
    WidgetNode, WidgetTree,
};
pub use crate::ui::widget_builder::WidgetBuilder;
pub use crate::ui::children::WidgetChildren;
pub use crate::ui::context::WidgetContext;

// ── 兼容性重导出（从 graphics/platform 层透传） ──
pub use crate::graphics::{AlignItems, FlexDirection, JustifyContent};
pub use crate::platform::{KeyCode, MouseButton};

// ── 状态管理 ──
pub use crate::ui::state::{Computed, State};
pub use crate::ui::style::Style;

// ── 渲染上下文 ──
pub use crate::ui::render_context::RenderContext;

// ── 动画 ──
pub use crate::ui::animation::core::Animation;
pub use crate::ui::animation::driver::{AnimationCallback, AnimationDriver, AnimValue};
pub use crate::ui::animation::easing::{Animatable, Easing};

// ── 管理器 ──
pub use crate::ui::managers::{
    AnimationManager, DragEventResult, DragManager, EventManager, FocusManager, ImageManager,
    InteractionManager, LayoutManager, StateManager, StyleManager, TextManager, WidgetManagers,
};

// ── 主题 ──
pub use crate::ui::theme::{
    DesignTokens, IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, ShadowToken,
    Theme, TokenProvider,
};

// ── Phase 1: 简单组件 ──
pub use crate::ui::widgets::alert::{Alert, AlertType};
pub use crate::ui::widgets::badge::Badge;
pub use crate::ui::widgets::breadcrumb::{Breadcrumb, BreadcrumbItem};
pub use crate::ui::widgets::empty::Empty;
pub use crate::ui::widgets::spin::{Spin, SpinSize};
pub use crate::ui::widgets::tag::{Tag, TagColor};

// ── Phase 2: 中等组件 ──
pub use crate::ui::widgets::collapse::{Collapse, CollapsePanel};
pub use crate::ui::widgets::radio::Radio;
pub use crate::ui::widgets::segmented::Segmented;
pub use crate::ui::widgets::slider::Slider;
pub use crate::ui::widgets::tooltip::{Tooltip, TooltipPlacement};
pub use crate::ui::widgets::rate::Rate;

// ── Phase 3: 复杂组件 ──
pub use crate::ui::widgets::dropdown::Dropdown;
pub use crate::ui::widgets::popconfirm::Popconfirm;
pub use crate::ui::widgets::popover::Popover;
pub use crate::ui::widgets::select::Select;
pub use crate::ui::widgets::skeleton::{Skeleton, SkeletonShape};
pub use crate::ui::widgets::table::{Table, TableColumn, TableRow};

// ── 已有组件 ──
pub use crate::ui::widgets::avatar::Avatar;
pub use crate::ui::widgets::chart::{BarChart, BarData, LineChart, LineData, PieChart, PieData};
pub use crate::ui::widgets::icon::Icon;
pub use crate::ui::widgets::checkbox::Checkbox;
pub use crate::ui::widgets::switch::Switch;
pub use crate::ui::widgets::button::{Button, ButtonSize, ButtonVariant};
pub use crate::ui::widgets::card::Card;
pub use crate::ui::widgets::container::Container;
pub use crate::ui::widgets::divider::{Divider, DividerDirection, DividerOrientation};
pub use crate::ui::widgets::grid::Grid;
pub use crate::ui::widgets::input::{Input, InputSize};
pub use crate::ui::widgets::label::Label;
pub use crate::ui::widgets::modal::Modal;
pub use crate::ui::widgets::nav::{NavGroup, NavItem, Navigation, SharedActive};
pub use crate::ui::widgets::progress::{ProgressBar, ProgressMode};
pub use crate::ui::widgets::scroll_view::{ScrollDirection, ScrollView};
pub use crate::ui::widgets::space::{Space, SpaceSize};
pub use crate::ui::widgets::tabs::{Tab, TabPosition, Tabs};