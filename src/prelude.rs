//! Unified public entrypoint with common re-exports grouped by domain.
//!
//! Most apps only need:
//! ```ignore
//! use uix::prelude::*;
//! ```
//!
//! Macro tiers（公开用法见仓库 `docs/使用.md`）：
//! - daily: `keyframe!`, `views!`, `with_cloned!`
//! - advanced: `widget!`
//! - framework: `tree!`, `impl_widget!`, `semantic_handler!`

// 直接导出推荐的 uix-lang 编译期入口宏，避开同名 crate 自引用。
pub use uix_derive::{uix, uix_app, uix_items};

// core

pub use crate::core::{
    Constraints, EdgeInsets, Errc, Error, ErrorSeverity, Point, Rect, Size, WidgetId, WindowId,
};

// UI-facing input values and platform graphics selection

pub use crate::platform::capabilities::StatusLevel;
pub use crate::platform::graphics::GraphicsBackend;
pub use crate::platform::windowing::{
    ControlSize, CursorType, KeyCode, KeyMod, MouseButton, ScrollDirection,
};
// draw

pub use crate::draw::{
    BlendMode, Color, FillRule, FontBundle, FontService, ImageService, Path, PathBuilder, Radius,
    Renderer, StrokeOptions, colors,
};

// data

pub use crate::data::SettingsService;

// ui

pub use crate::impl_widget;
pub use crate::keyframe;
pub use crate::semantic_handler;
pub use crate::t;
pub use crate::t_fmt_arg;
pub use crate::tree;
pub use crate::ui::i18n::{register_translations, set_translations, t_lookup, t_lookup_fmt};
pub use crate::ui::layout::{
    AlignItems, BoxModel, FlexDirection, FlexLayout, GridLayout, GridTrack, JustifyContent,
    LayoutChild, LayoutEngine, LayoutOutput,
};
pub use crate::ui::reactive::state::{Computed, Effect, State, StateSlotId};
pub use crate::ui::theme::style::{
    BoxShadowDef, ColorValue, DisplayMode, PaletteColor, Style, StyleSet, StyleState,
    TypographyToken,
};
pub use crate::ui::widget_runtime::focus_trap::FocusTrap;
pub use crate::widget;
// 公开 UIX 生成代码与手写消费者共享的背景图层值契约。
pub use crate::ui::theme::style::{
    BackgroundAxisPosition, BackgroundImage, BackgroundPosition, BackgroundRepeat,
};
// 公开 UIX 生成代码与手写消费者共享的边框线型契约。
pub use crate::ui::theme::style::BorderStyle;
// 公开 UIX 生成代码与手写消费者共享的字体族列表契约。
pub use crate::ui::theme::style::FontFamily;
// 公开 UIX 生成代码与手写消费者共享的字体粗细契约。
pub use crate::ui::theme::style::FontWeight;
// 公开 UIX 生成代码与手写消费者共享的行高契约。
pub use crate::ui::theme::style::LineHeight;
// 公开 UIX 生成代码与手写消费者共享的文本水平对齐契约。
pub use crate::ui::theme::style::TextAlign;
// 公开 UIX 生成代码与手写消费者共享的文本装饰契约。
pub use crate::ui::theme::style::TextDecoration;
pub use crate::ui::theme::{
    ColorScale, DATA_VISUALIZATION_PALETTE, DataVisualizationPalette, DesignTokens, DynTokens,
    FunctionalColorRole, NEUTRAL_PALETTE, NeutralColorScale, NeutralRole, PrimaryHue, ShadowToken,
    Theme, ThemePrimitives, TokenPatch, generate_color_scale,
};
pub use crate::ui::{
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AppState, AriaAttribute,
    ClickEvent, Config, ConfigProvider, EmptyContext, EmptyRenderer, EventResult, FocusHandle,
    FocusHandleError, HandlerId, HandlerOptions, HandlerRegistration, IBoxShadowTokens,
    IColorTokens, ISpacingTokens, ITypographyTokens, IntoWidgetNode, Locale, LocaleProvider,
    PaintContext, Placement, PositionInsets, PositionMode, SemanticAction, SemanticActionKind,
    SemanticEvent, SemanticKind, SemanticPayload, SnapshotCollapsePanel, SnapshotField,
    SnapshotFields, SnapshotSource, SnapshotTransferItem, SnapshotValue, SystemEvent,
    SystemEventKind, ThemeTokens, TokenProvider, TransformOrigin, TransformOriginValue, UserSelect,
    Widget, WidgetChildren, WidgetConfig, WidgetConfigSnapshot, WidgetHandle, WidgetOverrides,
    WidgetTokenOverrides, WindowControl, WindowResizeEdge, copy_to_clipboard, en_us,
    read_text_from_clipboard, render_empty_for, use_config, use_locale, window_control,
    window_control_named, window_drag_region, window_resize_region, with_config, with_locale,
    zh_cn,
};
// 在便捷导入面公开标准窗口控制组合构造器。
pub use crate::ui::window_controls;
pub use crate::ui::{
    Animated, Animation, AnimationConfig, AnimationGroup, AnimationGroupError, AnimationGroupItem,
    Easing, Keyframe, KeyframeAnimation, KeyframeDirection, KeyframeError, KeyframeFillMode,
    KeyframePlayback, Spring, SpringAnimation, Transition,
};
// 表格 capability 启用时才在 prelude 暴露专属快照列模型。
pub use crate::Display;
#[cfg(feature = "table")]
// 保持启用场景下既有的快照导入写法。
pub use crate::ui::{SnapshotTableColumn, SnapshotTableColumnGroup};
// ui / components
pub use crate::ui::{
    Affix, AutoComplete, Avatar, BackTop, Badge, BadgeColor, BadgeStatus, BreakpointError,
    Breakpoints, Button, ButtonGroup, ButtonGroupPosition, Calendar, CalendarCellInfo,
    CalendarEvent, Card, Carousel, CarouselEffect, Cascader, CascaderOption, CascaderValue,
    Checkbox, Col, Collapse, CollapsePanel, ColorPicker, Container, Content, Date, DatePicker,
    DateRangePicker, Descriptions, DescriptionsItem, Divider, DividerDirection, DividerOrientation,
    Empty, FieldError, FloatButton, FloatButtonBackTop, FloatButtonGroup, Footer, Form,
    FormBuilder, FormCheckboxItem, FormColorPickerItem, FormDatePickerItem,
    FormDateRangePickerItem, FormInitialValues, FormInputItem, FormInputNumberItem, FormItem,
    FormLayout, FormListBuilder, FormListError, FormListFieldError, FormListFields, FormListItemId,
    FormListModel, FormListValues, FormModel, FormRadioItem, FormRateItem, FormSegmentedItem,
    FormSelectItem, FormSliderItem, FormSwitchItem, FormTimePickerItem, Grid, Header, Icon, Image,
    ImageGroup, Input, InputGroup, InputNumber, InputNumberValue, InputSearchExt, InputStatus,
    IntoBadgeColor, IntoFormValue, Label, Layout, List, Mentions, MoveDirection, OptGroup,
    PickerMode, PresetDate, Radio, RadioDirection, RangeSlider, Rate, ResultType, ResultView,
    ScrollView, Segmented, Select, SelectOption, SelectOptionGroup, SelectValueMode,
    SelectableItem, SelectableList, Sider, Skeleton, SkeletonShape, Slider, Space, SpaceSize,
    Splitter, Switch, Tag, TagColor, ThemeToggle, Time, TimePicker, Timeline, TimelineItem,
    TooltipPlacement, Transfer, TransferItem, Trigger, TriggerMode, Typography, TypographyType,
    Upload, UploadAcceptError, UploadChange, UploadFile, UploadFileId, UploadFileIdError,
    UploadQueueResult, UploadRejectReason, UploadRejection, UploadStatus, UploadUpdateError,
    ValidateStatus, Values, VirtualScroll, VirtualScrollBuilder, Watermark, Weekday,
};
// 反馈 capability 启用时才在 prelude 暴露组件与专属模型。
#[cfg(feature = "feedback")]
// 共享的 TriggerMode 与 TooltipPlacement 已留在基础导入面。
pub use crate::ui::{
    Alert, Drawer, DrawerPlacement, FeedbackCloseReason, FeedbackClosed, Message,
    MessageDeclaration, MessageItem, Modal, ModalBuilder, ModalContext, Notification,
    NotificationDeclaration, NotificationItem, Popconfirm, PopconfirmPlacement, Popover,
    PopoverPlacement, PopoverTrigger, ProgressBar, ProgressMode, ProgressType, Spin, SpinSize,
    Tooltip,
};
// 导航 capability 启用时才在 prelude 暴露组件、条目模型与状态类型。
#[cfg(feature = "navigation")]
// 该列表保持导航组件族既有的便捷导入面。
pub use crate::ui::{
    Anchor, AnchorItem, Breadcrumb, BreadcrumbItem, Dropdown, DropdownItem, Menu, MenuBar,
    MenuBarItem, MenuBarMenu, MenuItem, MenuMode, NavGroup, NavItem, Navigation, Pagination,
    SharedActive, Step, StepStatus, Steps, Tab, TabPosition, Tabs,
};
// 表格 capability 启用时才在 prelude 暴露组件、构建器与数据模型。
#[cfg(feature = "table")]
// 该列表覆盖基础表格、泛型数据表与分页/列配置公开面。
pub use crate::ui::{
    DataTable, Fixed, SortDirection, Table, TableBuilder, TableChange, TableColumn,
    TableColumnGroup, TableDataColumn, TableDataError, TablePagination, TableRow,
};
// 图表 capability 启用时才在 prelude 暴露组件、数据模型与交互配置。
#[cfg(feature = "charts")]
// 该列表覆盖基础图表与高级图表的既有公开导入面。
pub use crate::ui::{
    AreaChart, AxisSide, BarChart, BarData, BrushConfig, BubbleData, ChartPlaceholder, ChartSeries,
    ChartType, ComboChart, ComboSeries, FunnelAlign, FunnelChart, FunnelData, FunnelShape, Gauge,
    GaugeRange, GaugeType, Heatmap, HeatmapCell, InteractionConfig, LabelPosition, LegendPosition,
    LineChart, LineData, LineStyle, PieChart, PieData, PointStyle, RadarAxis, RadarChart,
    RadarData, RadarShape, RoseStyle, ScatterChart, ScatterData, TooltipConfig, TooltipDatum,
    TooltipTrigger, Treemap, TreemapNode, WaterfallChart, WaterfallData, WaterfallKind,
};
// 二维码 capability 启用时才在 prelude 暴露组件类型。
#[cfg(feature = "qrcode")]
// 保持启用场景下既有的 QRCode 导入写法。
pub use crate::ui::QRCode;
// 富文本 capability 启用时才在 prelude 暴露组件与内容模型。
#[cfg(feature = "rich-text")]
// 保持启用场景下既有的富文本类型导入写法。
pub use crate::ui::{RichText, RichTextSegment, RichTextStyle};
// 树组件 capability 启用时才在 prelude 暴露组件、节点与快照模型。
#[cfg(feature = "tree-widgets")]
// 该列表覆盖展示树与树选择器的完整公开模型。
pub use crate::ui::{DropPosition, SnapshotTreeNode, Tree, TreeNode, TreeSelect};

// app

pub use crate::app::Container as DiContainer;
pub use crate::app::{
    App, AppHandle, AppMode, DesktopLayerConfig, TimerHandle, WindowConfig, WindowSurfaceRole,
    map_ui_event,
};
pub use crate::platform::windowing::{DesktopAnchor, DesktopKeyboardInteractivity, DesktopLayer};

// 富文本 capability 启用时才暴露解析与布局辅助函数。
#[cfg(feature = "rich-text")]
// 两个辅助函数与 RichText 组件共享同一能力边界。
pub use crate::ui::{layout_rich_text_segments, parse_rich_text};

// view

pub use crate::ui::{
    AccessibilityExt, ButtonBuilder, EventExt, GridBuilder, InputBuilder, IntoLabelContent,
    IntoViewChildren, ScrollBuilder, StyleExt, TransitionExt, View, ViewNode, button, canvas,
    column, column_fit, dynamic_label, embed, grid, input, label, row, scoped, scroll, show, space,
};
pub use crate::views;
pub use crate::with_cloned;

#[cfg(feature = "test-harness")]
pub use crate::ui::test_harness::TestApp;
