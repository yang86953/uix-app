// ============================================================================
// ui/api.rs — UI 层的公共 API 出口
//
// 本文件定义 ui 层对外暴露的公共接口。其他层只能通过本文件
// 使用 ui 层的功能，禁止直接引用内部模块。
// ============================================================================

// ── 控件核心 ──
pub use crate::children::WidgetChildren;
pub use crate::context::WidgetContext;
pub use crate::widget::{
    AsAny, BoxedWidget, EventResult, IntoWidgetNode, Widget, WidgetCore, WidgetEvent, WidgetEventHandler,
    WidgetId, WidgetLayout, WidgetLifecycle, WidgetNode, WidgetRender, WidgetTree,
};

// Platform types are used directly by WidgetEvent — re-export from base.
pub use uix_core::{KeyCode, MouseButton};

// ── 状态管理 ──
pub use crate::state::{Computed, Effect, State};
pub use crate::style::Style;

// ── 渲染上下文 ──
pub use crate::render_context::RenderContext;

// ── 布局引擎（Flexbox + Grid）──
pub use crate::layout::flex::compute_flex_layout;
pub use crate::layout::grid::compute_grid_layout;
pub use crate::layout::{
    AlignItems, FlexChild, FlexDirection, FlexInput, FlexOutput, GridChild, GridInput, GridOutput,
    GridTrack, JustifyContent,
};

// ── 动画 ──
pub use crate::animation::core::Animation;
pub use crate::animation::driver::{AnimValue, AnimationCallback, AnimationDriver};
pub use crate::animation::easing::{Animatable, Easing};

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

// ── Phase 1: 简单组件 ──
pub use crate::widgets::alert::{Alert, AlertType};
pub use crate::widgets::autocomplete::AutoComplete;
pub use crate::widgets::badge::Badge;
pub use crate::widgets::breadcrumb::{Breadcrumb, BreadcrumbItem};
pub use crate::widgets::empty::Empty;
pub use crate::widgets::image::Image;
pub use crate::widgets::message::{Message, MessageItem, MessageType};
pub use crate::widgets::notification::{Notification, NotificationItem, NotificationType};
pub use crate::widgets::result::{Result, ResultType};
pub use crate::widgets::spin::{Spin, SpinSize};
pub use crate::widgets::tag::{Tag, TagColor};
pub use crate::widgets::typography::{Typography, TypographyType};

// ── Phase 2: 中等组件 ──
pub use crate::widgets::collapse::{Collapse, CollapsePanel};
pub use crate::widgets::radio::Radio;
pub use crate::widgets::rate::Rate;
pub use crate::widgets::segmented::Segmented;
pub use crate::widgets::slider::Slider;
pub use crate::widgets::tooltip::{Tooltip, TooltipPlacement};

// ── Phase 3: 复杂组件 ──
pub use crate::widgets::dropdown::Dropdown;
pub use crate::widgets::form::{Form, FormItem, ValidateStatus};
pub use crate::widgets::menu::{Menu, MenuItem, MenuMode};
pub use crate::widgets::pagination::Pagination;
pub use crate::widgets::popconfirm::Popconfirm;
pub use crate::widgets::popover::Popover;
pub use crate::widgets::select::Select;
pub use crate::widgets::skeleton::{Skeleton, SkeletonShape};
pub use crate::widgets::steps::{Step, StepStatus, Steps};
pub use crate::widgets::table::{Table, TableColumn, TableRow};
pub use crate::widgets::tree::{Tree, TreeNode};
pub use crate::widgets::tree_select::TreeSelect;

// ── 已有组件 ──
pub use crate::widgets::affix::Affix;
pub use crate::widgets::anchor::{Anchor, AnchorItem};
pub use crate::widgets::avatar::Avatar;
pub use crate::widgets::button::{Button, ButtonSize, ButtonVariant};
pub use crate::widgets::back_top::BackTop;
pub use crate::widgets::calendar::Calendar;
pub use crate::widgets::card::Card;
pub use crate::widgets::chart::{BarChart, BarData, LineChart, LineData, PieChart, PieData};
pub use crate::widgets::checkbox::Checkbox;
pub use crate::widgets::container::Container;
pub use crate::widgets::descriptions::{Descriptions, DescriptionsItem};
pub use crate::widgets::divider::{Divider, DividerDirection, DividerOrientation};
pub use crate::widgets::drawer::{Drawer, DrawerPlacement};
pub use crate::widgets::carousel::Carousel;
pub use crate::widgets::float_button::FloatButton;
pub use crate::widgets::grid::Grid;
pub use crate::widgets::icon::Icon;
pub use crate::widgets::date_picker::{DatePicker, DateValue};
pub use crate::widgets::input::{Input, InputSize};
pub use crate::widgets::input_number::InputNumber;
pub use crate::widgets::label::Label;
pub use crate::widgets::list::{List, ListSize};
pub use crate::widgets::modal::Modal;
pub use crate::widgets::nav::{NavGroup, NavItem, Navigation, SharedActive};
pub use crate::widgets::progress::{ProgressBar, ProgressMode};
pub use crate::widgets::scroll_view::{ScrollDirection, ScrollView};
pub use crate::widgets::space::{Space, SpaceSize};
pub use crate::widgets::switch::Switch;
pub use crate::widgets::theme_toggle::ThemeToggle;
pub use crate::widgets::tabs::{Tab, TabPosition, Tabs};
pub use crate::widgets::timeline::{Timeline, TimelineItem};
pub use crate::widgets::cascader::{Cascader, CascaderOption};
pub use crate::widgets::color_picker::ColorPicker;
pub use crate::widgets::layout::{Content, Footer, Header, Layout, Sider};
pub use crate::widgets::mentions::Mentions;
pub use crate::widgets::misc::{QRCode, Transfer, TransferItem, Upload, Watermark};
pub use crate::widgets::splitter::Splitter;
pub use crate::widgets::time_picker::{TimePicker, TimeValue};
