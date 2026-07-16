//! Unified public entrypoint with common re-exports grouped by domain.
//!
//! Most apps only need:
//! ```ignore
//! use uix::prelude::*;
//! ```
//!
//! Macro tiers ([使用](docs/使用.md)):
//! - daily: `views!`, `with_cloned!`
//! - advanced: `component!`
//! - framework: `tree!`, `impl_widget_component!`, `semantic_handler!`

// core

pub use crate::core::{
    ComponentId, Constraints, EdgeInsets, Errc, Error, ErrorSeverity, Point, Rect, Size, WindowId,
};

// native

pub use crate::native::create_platform;
pub use crate::native::traits::input::{
    ControlSize, CursorType, KeyCode, KeyMod, MouseButton, ScrollDirection,
};
pub use crate::native::traits::present::GraphicsBackend;
pub use crate::native::traits::system::StatusLevel;
// draw

pub use crate::draw::traits::GraphicsEngine;
pub use crate::draw::{
    colors, Color, FillRule, FontService, ImageService, NullEngine, Path, PathBuilder, Radius,
    SoftwareEngine, StrokeOptions,
};

// data

pub use crate::data::SettingsService;

// ui

pub use crate::component;
pub use crate::impl_widget_component;
pub use crate::semantic_handler;
pub use crate::tree;
pub use crate::ui::foundation::FocusTrap;
pub use crate::ui::layout::{
    AlignItems, BoxModel, FlexDirection, FlexLayout, GridLayout, GridTrack, JustifyContent,
    LayoutChild, LayoutEngine, LayoutOutput,
};
pub use crate::ui::state::{Computed, Effect, State, StateSlotId};
pub use crate::ui::style::{
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
    FocusHandle, FocusHandleError, HandlerId, HandlerOptions, HandlerRegistration, IntoWidgetNode,
    Locale, LocaleProvider, PaintContext, Placement, SemanticEvent, SemanticKind, SemanticPayload,
    SnapshotCollapsePanel, SnapshotField, SnapshotFields, SnapshotSource, SnapshotTableColumn,
    SnapshotTableColumnGroup, SnapshotTransferItem, SnapshotTreeNode, SnapshotValue, SystemEvent,
    SystemEventKind, WidgetChildren, WidgetComponent, WindowControl,
};
pub use crate::ui::{Animated, Animation, AnimationConfig, Easing, Spring, SpringAnimation};
// ui / components
pub use crate::ui::{
    Affix, Alert, Anchor, AnchorItem, AutoComplete, Avatar, BackTop, Badge, BadgeStatus, BarChart,
    BarData, Breadcrumb, BreadcrumbItem, BreakpointError, Breakpoints, Button, Calendar, Card,
    Carousel, Cascader, CascaderOption, CascaderValue, Checkbox, Col, Collapse, CollapsePanel,
    ColorPicker, Container, Content, Date, DatePicker, DateRangePicker, Descriptions,
    DescriptionsItem, Divider, DividerDirection, DividerOrientation, Drawer, DrawerPlacement,
    Dropdown, Empty, FieldError, Fixed, FloatButton, FloatButtonBackTop, Footer, Form, FormBuilder,
    FormCheckboxItem, FormColorPickerItem, FormDatePickerItem, FormDateRangePickerItem,
    FormInputItem, FormInputNumberItem, FormItem, FormLayout, FormListBuilder, FormListError,
    FormListFieldError, FormListFields, FormListItemId, FormListModel, FormListValues, FormModel,
    FormRadioItem, FormRateItem, FormSegmentedItem, FormSelectItem, FormSliderItem, FormSwitchItem,
    FormTimePickerItem, Grid, Header, Icon, Image, Input, InputNumber, InputNumberValue,
    IntoFormValue, Label, Layout, LineChart, LineData, List, Mentions, Menu, MenuItem, MenuMode,
    Message, MessageItem, Modal, ModalBuilder, ModalContext, NavGroup, NavItem, Navigation,
    Notification, NotificationItem, OptGroup, Pagination, PickerMode, PieChart, PieData,
    Popconfirm, PopconfirmPlacement, Popover, PopoverPlacement, PopoverTrigger, PresetDate,
    ProgressBar, ProgressMode, ProgressType, QRCode, Radio, RadioDirection, Rate, ResultType,
    ResultView, RichText, RichTextSegment, RichTextStyle, ScrollView, Segmented, Select,
    SelectableItem, SelectableList, SharedActive, Sider, Skeleton, SkeletonShape, Slider,
    SortDirection, Space, SpaceSize, Spin, SpinSize, Splitter, Step, StepStatus, Steps, Switch,
    Tab, TabPosition, Table, TableBuilder, TableChange, TableColumn, TableColumnGroup, TableRow,
    Tabs, Tag, TagColor, ThemeToggle, Time, TimePicker, Timeline, TimelineItem, Tooltip,
    TooltipPlacement, Transfer, TransferItem, Tree, TreeNode, TreeSelect, Trigger, TriggerMode,
    Typography, TypographyType, Upload, UploadFile, UploadStatus, ValidateStatus, Values,
    VirtualScroll, VirtualScrollBuilder, Watermark, Weekday,
};

// app

pub use crate::app::Container as DiContainer;
pub use crate::app::{map_ui_event, App, AppHandle, AppMode, TimerHandle, WindowConfig};

// view

pub use crate::ui::view::{
    button, canvas, column, column_fit, dynamic_label, embed, grid, input, label, row, scroll,
    show, space, AccessibilityExt, ButtonBuilder, EventExt, GridBuilder, InputBuilder,
    IntoLabelContent, IntoViewChildren, ScrollBuilder, StyleExt, Ui, View, ViewNode,
};
pub use crate::views;
pub use crate::with_cloned;

#[cfg(feature = "test-harness")]
pub use crate::ui::test_harness::TestApp;
