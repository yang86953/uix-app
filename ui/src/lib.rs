//! UIX UI — 响应式 Widget 框架。
//!
//! # 快速开始
//!
//! ```ignore
//! use uix::ui::*;
//!
//! // 1. 定义 widget
//! define_widget! {
//!     pub MyWidget {
//!         label: String,
//!     }
//!     @new -> Self { Self { label: "Hello".into() } }
//!
//!     render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
//!         ctx.fill_rect(frame, ctx.tokens().color_primary_bg(), None);
//!         ctx.draw_text(&self.label, frame.origin(), ctx.tokens().color_text(), 14.0);
//!     }
//! }
//!
//! // 2. 组合 widget 树
//! let node = tree! {
//!     Container::new().size(800.0, 600.0) => [
//!         MyWidget::new(),
//!         Button::new("Click").primary(),
//!     ]
//! };
//!
//! // 3. 构建并渲染
//! let mut widget_tree = WidgetTree::new();
//! widget_tree.build(node);
//! ```
//!
//! # 核心模块
//!
//! - [`widget`] — Widget trait、WidgetTree、事件系统
//! - [`widgets`] — 内置组件库（Button、Input、Modal 等 60+）
//! - [`layout`] — Flexbox + Grid 布局引擎
//! - [`render_context`] — 渲染上下文（文本/图形/主题令牌）
//! - [`spatial`] — 空间坐标系统（`SpatialContext`、物理单位、3D 变换）
//!   通过 `ctx.spatial()` 访问，支持：
//!   - 物理单位：`10.mm()`, `5.cm()`, `12.pt()`
//!   - 3D 变换：`translate()`, `rotate_z()`, `scale()`
//!   - 透视投影：`set_perspective()`, `set_camera_look_at()`
//! - [`state`] — 响应式状态管理（State/Computed/Effect）
//! - [`theme`] — 设计令牌系统
//! - [`animation`] — 动画与过渡系统
//! - [`macros`] — `define_widget!` 与 `tree!` 宏
//! - [`managers`] — 事件焦点/拖拽/动画管理器
//!
//! # 布局方式
//!
//! ```ignore
//! use uix::ui::layout::engine::{FlexLayout, LayoutEngine, LayoutChild};
//! ```

pub mod animation;
pub mod children;
pub mod clipboard;
pub mod config_provider;
pub mod context;
pub mod focus_trap;
pub mod layout;
pub mod layer;
pub mod locale;
pub mod macros;
pub mod managers;
pub mod render_context;
pub mod render_loop;
pub mod state;
pub mod style;
pub mod theme;
pub mod virtual_scroll;
pub mod widget;
pub mod widgets;

pub use uix_macros::ui;

// ── 控件核心 ──
pub use crate::children::WidgetChildren;
pub use crate::context::WidgetContext;
pub use crate::widget::{
    BoxedWidget, EventResult, IntoWidgetNode, Widget, WidgetCore, WidgetEvent, WidgetEventHandler,
    WidgetId, WidgetLayout, WidgetLifecycle, WidgetNode, WidgetRender, WidgetTree,
};
pub use uix_platform::{KeyCode, MouseButton};

// ── 配置提供 ──
pub use crate::config_provider::{
    ComponentConfig, ComponentOverrides, ConfigProvider, FormLayout, use_config, with_config,
    ButtonOverrides, InputOverrides, SelectOverrides, FormOverrides,
};

// ── 焦点锁定 ──
pub use crate::focus_trap::FocusTrap;

// ── 国际化 ──
pub use crate::locale::{en_us, zh_cn, Locale, LocaleProvider, use_locale, with_locale};

// ── 虚拟滚动 ──
pub use crate::virtual_scroll::VirtualScroll;

// ── 状态管理 ──
pub use crate::state::{Computed, Effect, State};
pub use crate::style::Style;

// ── 空间坐标系统（来自 uix-graphics） ──
pub use uix_graphics::spatial::{
    AABB3D, DirtyRegion3D, IntoAABB3D, Mat4, Orientation, PhysicalBox,
    PhysicalUnit, PhysicalUnitExt, Quad2D, Ray3D, SpatialContext, Vec2, Vec3, Vec4,
};
pub use uix_graphics::spatial::unit::AngleExt;

// ── 渲染上下文 ──
pub use crate::render_context::RenderContext;

// ── 布局引擎 ──
pub use crate::layout::{
    AlignItems, FlexDirection, GridTrack, JustifyContent,
};
pub use crate::layout::engine::{
    BoxModel, FlexLayout, GridLayout, LayoutChild, LayoutEngine, LayoutOutput, child_from_tree,
};

// ── 动画 ──
pub use crate::animation::core::Animation;
pub use crate::animation::driver::{AnimValue, AnimationCallback, AnimationDriver};
pub use crate::animation::easing::{Animatable, Easing};
pub use crate::animation::transition::{
    presets, SlideDirection, Transition, TransitionPlayer,
};

// ── 管理器 ──
pub use crate::managers::{
    AnimationManager, DragEventResult, DragManager, EventManager, FocusManager, ImageManager,
    InteractionManager, StateManager, StyleManager, TextManager, WidgetManagers,
};

// ── 主题 ──
pub use crate::theme::{
    DesignTokens, IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, ShadowToken,
    Theme, TokenProvider,
};

// ── 所有组件 ──
pub use crate::widgets::affix::Affix;
pub use crate::widgets::alert::{Alert, AlertType};
pub use crate::widgets::anchor::{Anchor, AnchorItem};
pub use crate::widgets::autocomplete::AutoComplete;
pub use crate::widgets::avatar::Avatar;
pub use crate::widgets::back_top::BackTop;
pub use crate::widgets::badge::Badge;
pub use crate::widgets::breadcrumb::{Breadcrumb, BreadcrumbItem};
pub use crate::widgets::button::{Button, ButtonSize, ButtonVariant};
pub use crate::widgets::calendar::Calendar;
pub use crate::widgets::card::Card;
pub use crate::widgets::carousel::Carousel;
pub use crate::widgets::cascader::{Cascader, CascaderOption};
pub use crate::widgets::chart::{BarChart, BarData, LineChart, LineData, PieChart, PieData};
pub use crate::widgets::checkbox::Checkbox;
pub use crate::widgets::collapse::{Collapse, CollapsePanel};
pub use crate::widgets::color_picker::ColorPicker;
pub use crate::widgets::container::Container;
pub use crate::widgets::date_picker::{DatePicker, DateValue};
pub use crate::widgets::descriptions::{Descriptions, DescriptionsItem};
pub use crate::widgets::divider::{Divider, DividerDirection, DividerOrientation};
pub use crate::widgets::drawer::{Drawer, DrawerPlacement};
pub use crate::widgets::dropdown::Dropdown;
pub use crate::widgets::empty::Empty;
pub use crate::widgets::float_button::FloatButton;
pub use crate::widgets::form::{Form, FormItem, ValidateStatus};
pub use crate::widgets::grid::Grid;
pub use crate::widgets::icon::Icon;
pub use crate::widgets::image::Image;
pub use crate::widgets::input::{Input, InputSize};
pub use crate::widgets::input_number::InputNumber;
pub use crate::widgets::label::Label;
pub use crate::widgets::layout::{Content, Footer, Header, Layout, Sider};
pub use crate::widgets::list::{List, ListSize};
pub use crate::widgets::mentions::Mentions;
pub use crate::widgets::menu::{Menu, MenuItem, MenuMode};
pub use crate::widgets::message::{Message, MessageItem, MessageType};
pub use crate::widgets::misc::{QRCode, Transfer, TransferItem, Upload, Watermark};
pub use crate::widgets::modal::Modal;
pub use crate::widgets::nav::{NavGroup, NavItem, Navigation, SharedActive};
pub use crate::widgets::notification::{Notification, NotificationItem, NotificationType};
pub use crate::widgets::pagination::Pagination;
pub use crate::widgets::popconfirm::Popconfirm;
pub use crate::widgets::popover::Popover;
pub use crate::widgets::progress::{ProgressBar, ProgressMode};
pub use crate::widgets::radio::Radio;
pub use crate::widgets::rate::Rate;
pub use crate::widgets::result::{Result, ResultType};
pub use crate::widgets::rich_text::{RichText, RichTextSegment, RichTextStyle, layout_rich_text_segments, parse_rich_text};
pub use crate::widgets::scroll_view::{ScrollDirection, ScrollView};
pub use crate::widgets::segmented::Segmented;
pub use crate::widgets::select::Select;
pub use crate::widgets::selectable_list::{SelectableList, SelectableItem};
pub use crate::widgets::skeleton::{Skeleton, SkeletonShape};
pub use crate::widgets::slider::Slider;
pub use crate::widgets::space::{Space, SpaceSize};
pub use crate::widgets::spin::{Spin, SpinSize};
pub use crate::widgets::splitter::Splitter;
pub use crate::widgets::steps::{Step, StepStatus, Steps};
pub use crate::widgets::switch::Switch;
pub use crate::widgets::table::{Table, TableColumn, TableRow};
pub use crate::widgets::tabs::{Tab, TabPosition, Tabs};
pub use crate::widgets::tag::{Tag, TagColor};
pub use crate::widgets::theme_toggle::ThemeToggle;
pub use crate::widgets::time_picker::{TimePicker, TimeValue};
pub use crate::widgets::timeline::{Timeline, TimelineItem};
pub use crate::widgets::tooltip::{Tooltip, TooltipPlacement};
pub use crate::widgets::tree::{Tree, TreeNode};
pub use crate::widgets::tree_select::TreeSelect;
pub use crate::widgets::typography::{Typography, TypographyType};
