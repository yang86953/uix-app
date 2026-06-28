//! # uix-ui 数据契约
//!
//! 本模块定义 ui 层的共享数据结构（值类型）。

// ── 宏 ──
pub use uix_macros::ui;

// ── 控件核心 ──
pub use crate::children::WidgetChildren;
pub use crate::context::WidgetContext;
pub use crate::widget::{
    BoxedWidget, EventResult, WidgetCapabilities, WidgetComponent, WidgetCore, WidgetEvent,
    WidgetId, WidgetNode, WidgetTree,
};

// ── 配置提供 ──
pub use crate::config_provider::{
    ButtonOverrides, ComponentConfig, ComponentOverrides, ConfigProvider, FormLayout,
    FormOverrides, InputOverrides, SelectOverrides, use_config, with_config,
};

// ── 焦点锁定 ──
pub use crate::focus_trap::FocusTrap;

// ── 国际化 ──
pub use crate::locale::{en_us, zh_cn, Locale, LocaleProvider, use_locale, with_locale};

// ── 虚拟滚动 ──
pub use crate::virtual_scroll::VirtualScroll;

// ── 状态管理 ──
pub use crate::state::{Computed, Effect, State};
pub use crate::style::{Style, StyleVariant};

// ── 剪贴板 ──
pub use crate::clipboard::copy_to_clipboard;

// ── 平台基础类型 ──
pub use uix_platform::{KeyCode, KeyMod, MouseButton};

// ── 渲染上下文 ──
pub use crate::render_context::RenderContext;

// ── 布局引擎 ──
pub use crate::layout::engine::{
    BoxModel, FlexLayout, GridLayout, LayoutChild, LayoutOutput, child_from_tree,
};
pub use crate::layout::{AlignItems, FlexDirection, GridTrack, JustifyContent};

// ── 动画 ──
pub use crate::animation::core::Animation;
pub use crate::animation::driver::{AnimValue, AnimationCallback, AnimationDriver};
pub use crate::animation::easing::Easing;
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
    DesignTokens,
    ShadowToken, Theme,
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
