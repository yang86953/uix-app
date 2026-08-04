//! Unified public entrypoint with common re-exports grouped by domain.
//!
//! Most apps only need:
//! ```ignore
//! use uix::prelude::*;
//! ```
//!
//! Macro tiers（知识库：`C:\data\note\我的项目\软件\UIX App\使用.md`）：
//! - daily: `keyframe!`, `views!`, `with_cloned!`
//! - advanced: `component!`
//! - framework: `tree!`, `impl_widget_component!`, `semantic_handler!`

// core

pub use crate::core::{
    ComponentId, Constraints, EdgeInsets, Errc, Error, ErrorSeverity, Point, Rect, Size, WindowId,
};

// UI-facing input values and platform graphics selection

pub use crate::native::capabilities::system::StatusLevel;
pub use crate::native::windowing::input::{
    ControlSize, CursorType, KeyCode, KeyMod, MouseButton, ScrollDirection,
};
pub use crate::platform::graphics::GraphicsBackend;
// draw

pub use crate::draw::{
    colors, BlendMode, Color, FillRule, FontService, ImageService, Path, PathBuilder, Radius,
    Renderer, StrokeOptions,
};

// data

pub use crate::data::SettingsService;

// ui

pub use crate::component;
pub use crate::impl_widget_component;
pub use crate::keyframe;
pub use crate::semantic_handler;
pub use crate::t;
pub use crate::t_fmt_arg;
pub use crate::tree;
pub use crate::ui::component::focus_trap::FocusTrap;
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
pub use crate::ui::theme::{
    generate_color_scale, ColorScale, DataVisualizationPalette, DesignTokens, DynTokens,
    FunctionalColorRole, NeutralColorScale, NeutralRole, PrimaryHue, ShadowToken, Theme,
    ThemePrimitives, TokenPatch, DATA_VISUALIZATION_PALETTE, NEUTRAL_PALETTE,
};
pub use crate::ui::{
    copy_to_clipboard, en_us, read_text_from_clipboard, render_empty_for, use_config, use_locale,
    window_control, window_control_named, window_drag_region, with_config, with_locale, zh_cn,
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AppState, AriaAttribute,
    ClickEvent, ComponentConfig, ComponentConfigSnapshot, ComponentHandle, ComponentOverrides,
    ComponentTokenOverrides, Config, ConfigProvider, EmptyContext, EmptyRenderer, EventResult,
    FocusHandle, FocusHandleError, HandlerId, HandlerOptions, HandlerRegistration,
    IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, IntoWidgetNode, Locale,
    LocaleProvider, PaintContext, Placement, SemanticAction, SemanticActionKind, SemanticEvent,
    SemanticKind, SemanticPayload, SnapshotCollapsePanel, SnapshotField, SnapshotFields,
    SnapshotSource, SnapshotTransferItem, SnapshotTreeNode, SnapshotValue, SystemEvent,
    SystemEventKind, ThemeTokens, TokenProvider, WidgetChildren, WidgetComponent, WindowControl,
};
pub use crate::ui::{
    Animated, Animation, AnimationConfig, AnimationGroup, AnimationGroupError, AnimationGroupItem,
    Easing, Keyframe, KeyframeAnimation, KeyframeError, Spring, SpringAnimation, Transition,
};
// 表格 capability 启用时才在 prelude 暴露专属快照列模型。
#[cfg(feature = "table")]
// 保持启用场景下既有的快照导入写法。
pub use crate::ui::{SnapshotTableColumn, SnapshotTableColumnGroup};
pub use crate::Display;
// ui / components
pub use crate::ui::{
    Affix, Alert, Anchor, AnchorItem, AutoComplete, Avatar, BackTop, Badge, BadgeColor,
    BadgeStatus, Breadcrumb, BreadcrumbItem, BreakpointError, Breakpoints, Button, ButtonGroup,
    ButtonGroupPosition, Calendar, CalendarCellInfo, CalendarEvent, Card, Carousel, CarouselEffect,
    Cascader, CascaderOption, CascaderValue, Checkbox, Col, Collapse, CollapsePanel, ColorPicker,
    Container, Content, Date, DatePicker, DateRangePicker, Descriptions, DescriptionsItem, Divider,
    DividerDirection, DividerOrientation, Drawer, DrawerPlacement, DropPosition, Dropdown,
    DropdownItem, Empty, FieldError, FloatButton, FloatButtonBackTop, FloatButtonGroup, Footer,
    Form, FormBuilder, FormCheckboxItem, FormColorPickerItem, FormDatePickerItem,
    FormDateRangePickerItem, FormInitialValues, FormInputItem, FormInputNumberItem, FormItem,
    FormLayout, FormListBuilder, FormListError, FormListFieldError, FormListFields, FormListItemId,
    FormListModel, FormListValues, FormModel, FormRadioItem, FormRateItem, FormSegmentedItem,
    FormSelectItem, FormSliderItem, FormSwitchItem, FormTimePickerItem, Grid, Header, Icon, Image,
    ImageGroup, Input, InputGroup, InputNumber, InputNumberValue, InputSearchExt, InputStatus,
    IntoBadgeColor, IntoFormValue, Label, Layout, List, Mentions, Menu, MenuItem, MenuMode,
    Message, MessageFacade, MessageItem, Modal, ModalBuilder, ModalContext, MoveDirection,
    NavGroup, NavItem, Navigation, Notification, NotificationItem, OptGroup, Pagination,
    PickerMode, Popconfirm, PopconfirmPlacement, Popover, PopoverPlacement, PopoverTrigger,
    PresetDate, ProgressBar, ProgressMode, ProgressType, Radio, RadioDirection, RangeSlider, Rate,
    ResultType, ResultView, ScrollView, Segmented, Select, SelectOptionGroup, SelectableItem,
    SelectableList, SharedActive, Sider, Skeleton, SkeletonShape, Slider, Space, SpaceSize, Spin,
    SpinSize, Splitter, Step, StepStatus, Steps, Switch, Tab, TabPosition, Tabs, Tag, TagColor,
    ThemeToggle, Time, TimePicker, Timeline, TimelineItem, Tooltip, TooltipPlacement, Transfer,
    TransferItem, Tree, TreeNode, TreeSelect, Trigger, TriggerMode, Typography, TypographyType,
    Upload, UploadFile, UploadStatus, ValidateStatus, Values, VirtualScroll, VirtualScrollBuilder,
    Watermark, Weekday,
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

// app

pub use crate::app::Container as DiContainer;
pub use crate::app::{map_ui_event, App, AppHandle, AppMode, TimerHandle, WindowConfig};

// 富文本 capability 启用时才暴露解析与布局辅助函数。
#[cfg(feature = "rich-text")]
// 两个辅助函数与 RichText 组件共享同一能力边界。
pub use crate::ui::{layout_rich_text_segments, parse_rich_text};

// view

pub use crate::ui::{
    button, canvas, column, column_fit, dynamic_label, embed, grid, input, label, message, notify,
    row, scroll, show, space, AccessibilityExt, ButtonBuilder, EventExt, GridBuilder, InputBuilder,
    IntoLabelContent, IntoViewChildren, ScrollBuilder, StyleExt, TransitionExt, View, ViewNode,
};
pub use crate::views;
pub use crate::with_cloned;

#[cfg(feature = "test-harness")]
pub use crate::ui::test_harness::TestApp;
