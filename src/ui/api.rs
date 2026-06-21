// ============================================================================
// ui/api.rs — UI 层的公共 API 出口
//
// 本文件定义 ui 层对外暴露的公共接口。其他层只能通过本文件
// 使用 ui 层的功能，禁止直接引用内部模块。
// ============================================================================

// ── 控件核心 ──
pub use crate::ui::children::WidgetChildren;
pub use crate::ui::context::WidgetContext;
pub use crate::ui::widget::{
    AsAny, BoxedWidget, EventResult, IntoWidgetNode, Widget, WidgetCore, WidgetEvent, WidgetEventHandler,
    WidgetId, WidgetLayout, WidgetLifecycle, WidgetNode, WidgetRender, WidgetTree,
};

// Platform types are used directly by WidgetEvent — re-export from base.
pub use crate::base::{KeyCode, MouseButton};

// ── 状态管理 ──
pub use crate::ui::state::{Computed, Effect, State};
pub use crate::ui::style::Style;

// ── 渲染上下文 ──
pub use crate::ui::render_context::RenderContext;

// ── 布局引擎（Flexbox + Grid）──
pub use crate::ui::layout::flex::compute_flex_layout;
pub use crate::ui::layout::grid::compute_grid_layout;
pub use crate::ui::layout::{
    AlignItems, FlexChild, FlexDirection, FlexInput, FlexOutput, GridChild, GridInput, GridOutput,
    GridTrack, JustifyContent,
};

// ── 动画 ──
pub use crate::ui::animation::core::Animation;
pub use crate::ui::animation::driver::{AnimValue, AnimationCallback, AnimationDriver};
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
pub use crate::ui::widgets::autocomplete::AutoComplete;
pub use crate::ui::widgets::badge::Badge;
pub use crate::ui::widgets::breadcrumb::{Breadcrumb, BreadcrumbItem};
pub use crate::ui::widgets::empty::Empty;
pub use crate::ui::widgets::image::Image;
pub use crate::ui::widgets::message::{Message, MessageItem, MessageType};
pub use crate::ui::widgets::notification::{Notification, NotificationItem, NotificationType};
pub use crate::ui::widgets::result::{Result, ResultType};
pub use crate::ui::widgets::spin::{Spin, SpinSize};
pub use crate::ui::widgets::tag::{Tag, TagColor};
pub use crate::ui::widgets::typography::{Typography, TypographyType};

// ── Phase 2: 中等组件 ──
pub use crate::ui::widgets::collapse::{Collapse, CollapsePanel};
pub use crate::ui::widgets::radio::Radio;
pub use crate::ui::widgets::rate::Rate;
pub use crate::ui::widgets::segmented::Segmented;
pub use crate::ui::widgets::slider::Slider;
pub use crate::ui::widgets::tooltip::{Tooltip, TooltipPlacement};

// ── Phase 3: 复杂组件 ──
pub use crate::ui::widgets::dropdown::Dropdown;
pub use crate::ui::widgets::form::{Form, FormItem, ValidateStatus};
pub use crate::ui::widgets::menu::{Menu, MenuItem, MenuMode};
pub use crate::ui::widgets::pagination::Pagination;
pub use crate::ui::widgets::popconfirm::Popconfirm;
pub use crate::ui::widgets::popover::Popover;
pub use crate::ui::widgets::select::Select;
pub use crate::ui::widgets::skeleton::{Skeleton, SkeletonShape};
pub use crate::ui::widgets::steps::{Step, StepStatus, Steps};
pub use crate::ui::widgets::table::{Table, TableColumn, TableRow};
pub use crate::ui::widgets::tree::{Tree, TreeNode};
pub use crate::ui::widgets::tree_select::TreeSelect;

// ── 已有组件 ──
pub use crate::ui::widgets::affix::Affix;
pub use crate::ui::widgets::anchor::{Anchor, AnchorItem};
pub use crate::ui::widgets::avatar::Avatar;
pub use crate::ui::widgets::button::{Button, ButtonSize, ButtonVariant};
pub use crate::ui::widgets::back_top::BackTop;
pub use crate::ui::widgets::calendar::Calendar;
pub use crate::ui::widgets::card::Card;
pub use crate::ui::widgets::chart::{BarChart, BarData, LineChart, LineData, PieChart, PieData};
pub use crate::ui::widgets::checkbox::Checkbox;
pub use crate::ui::widgets::container::Container;
pub use crate::ui::widgets::descriptions::{Descriptions, DescriptionsItem};
pub use crate::ui::widgets::divider::{Divider, DividerDirection, DividerOrientation};
pub use crate::ui::widgets::drawer::{Drawer, DrawerPlacement};
pub use crate::ui::widgets::carousel::Carousel;
pub use crate::ui::widgets::float_button::FloatButton;
pub use crate::ui::widgets::grid::Grid;
pub use crate::ui::widgets::icon::Icon;
pub use crate::ui::widgets::date_picker::{DatePicker, DateValue};
pub use crate::ui::widgets::input::{Input, InputSize};
pub use crate::ui::widgets::input_number::InputNumber;
pub use crate::ui::widgets::label::Label;
pub use crate::ui::widgets::list::{List, ListSize};
pub use crate::ui::widgets::modal::Modal;
pub use crate::ui::widgets::nav::{NavGroup, NavItem, Navigation, SharedActive};
pub use crate::ui::widgets::progress::{ProgressBar, ProgressMode};
pub use crate::ui::widgets::scroll_view::{ScrollDirection, ScrollView};
pub use crate::ui::widgets::space::{Space, SpaceSize};
pub use crate::ui::widgets::switch::Switch;
pub use crate::ui::widgets::tabs::{Tab, TabPosition, Tabs};
pub use crate::ui::widgets::timeline::{Timeline, TimelineItem};
pub use crate::ui::widgets::cascader::{Cascader, CascaderOption};
pub use crate::ui::widgets::color_picker::ColorPicker;
pub use crate::ui::widgets::layout::{Content, Footer, Header, Layout, Sider};
pub use crate::ui::widgets::mentions::Mentions;
pub use crate::ui::widgets::misc::{QRCode, Transfer, TransferItem, Upload, Watermark};
pub use crate::ui::widgets::splitter::Splitter;
pub use crate::ui::widgets::time_picker::{TimePicker, TimeValue};
