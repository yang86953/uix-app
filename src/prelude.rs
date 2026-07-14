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
    ComponentId, Constraints, EdgeInsets, Errc, Error, Point, Rect, Size, WindowId,
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
pub use crate::draw::{colors, Color, FontService, ImageService, NullEngine, SoftwareEngine};

// ui

pub use crate::component;
pub use crate::impl_widget_component;
pub use crate::semantic_handler;
pub use crate::tree;
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
    DesignTokens, DynTokens, NeutralRole, ShadowToken, Theme, ThemePrimitives,
};
pub use crate::ui::{
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AppState, AriaAttribute,
    ClickEvent, ComponentConfigSnapshot, ComponentHandle, EventResult, HandlerId, HandlerOptions,
    HandlerRegistration, IntoWidgetNode, PaintContext, SemanticEvent, SemanticKind,
    SemanticPayload, SnapshotCollapsePanel, SnapshotField, SnapshotFields, SnapshotSource,
    SnapshotTableColumn, SnapshotTransferItem, SnapshotTreeNode, SnapshotValue, SystemEvent,
    SystemEventKind, WidgetChildren,
};
pub use crate::ui::{Animation, Easing};
// ui / components
pub use crate::ui::{
    Affix, Alert, Anchor, AnchorItem, AutoComplete, Avatar, BackTop, Badge, BadgeStatus, BarChart,
    BarData, Breadcrumb, BreadcrumbItem, Button, Calendar, Card, Carousel, Cascader,
    CascaderOption, CascaderValue, Checkbox, Collapse, CollapsePanel, ColorPicker, Container,
    Content, Date, DatePicker, Descriptions, DescriptionsItem, Divider, DividerDirection,
    DividerOrientation, Drawer, DrawerPlacement, Dropdown, Empty, FieldDef, FloatButton,
    FloatButtonBackTop, Footer, Form, FormItem, FormLayout, FormValidationError, FormValidatorKey,
    FormValidatorTable, Grid, Header, Icon, Image, Input, InputNumber, Label, Layout, LineChart,
    LineData, List, Mentions, Menu, MenuItem, MenuMode, Message, MessageItem, MessagePlacement,
    Modal, NavGroup, NavItem, Navigation, NotifPlacement, Notification, NotificationItem, OptGroup,
    Pagination, PieChart, PieData, Popconfirm, PopconfirmPlacement, Popover, PopoverPlacement,
    PopoverTrigger, ProgressBar, ProgressMode, ProgressType, QRCode, Radio, RadioDirection, Rate,
    ResultType, ResultView, RichText, RichTextSegment, RichTextStyle, ScrollView, Segmented,
    Select, SelectableItem, SelectableList, SharedActive, Sider, Skeleton, SkeletonShape, Slider,
    SortDirection, Space, SpaceSize, Spin, SpinSize, Splitter, Step, StepStatus, Steps, Switch,
    Tab, TabPosition, Table, TableBuilder, TableChange, TableColumn, TableRow, Tabs, Tag, TagColor,
    ThemeToggle, TimePicker, TimeValue, Timeline, TimelineItem, Tooltip, TooltipPlacement,
    Transfer, TransferItem, Tree, TreeNode, TreeSelect, TriggerMode, Typography, TypographyType,
    Upload, UploadFile, UploadStatus, ValidateStatus, ValidationResult, ValidationRule, Watermark,
};

// app

pub use crate::app::Container as DiContainer;
pub use crate::app::{map_ui_event, App, AppHandle, AppMode, TimerHandle, WindowConfig};

// view

pub use crate::ui::view::{
    button, canvas, column, column_fit, dynamic_label, embed, grid, input, label, row, scroll,
    show, space, ButtonBuilder, GridBuilder, InputBuilder, IntoLabelContent, IntoViewChildren,
    ScrollBuilder, StyleExt, Ui, View, ViewNode,
};
pub use crate::views;
pub use crate::with_cloned;
