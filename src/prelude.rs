//! 统一对外入口 — 按功能域分组 re-export 常用符号。
//!
//! 绝大多数场景只需：
//! ```ignore
//! use uix::prelude::*;
//! ```

// ── core ──────────────────────────────────────────────────────

pub use crate::core::{EdgeInsets, Errc, Error, Point, Rect, Size};

// ── native ────────────────────────────────────────────────────

pub use crate::native::{create_platform, KeyCode, KeyMod, MouseButton};

// ── draw ──────────────────────────────────────────────────────

pub use crate::draw::traits::GraphicsEngine;
pub use crate::draw::{colors, Color, FontService, ImageService, NullEngine, SoftwareEngine};

// ── ui ────────────────────────────────────────────────────────

pub use crate::define_widget;
pub use crate::tree;
pub use crate::ui::state::{Computed, State};
pub use crate::ui::style::{Style, StyleVariant};
pub use crate::ui::theme::{DesignTokens, DynTokens, Theme};
pub use crate::ui::{
    EventResult, IntoWidgetNode, RenderContext, WidgetCore, WidgetEvent, WidgetId, WidgetNode,
    WidgetTree,
};
pub use crate::ui::layout::{
    AlignItems, FlexDirection, FlexLayout, GridTrack, JustifyContent, LayoutChild, LayoutEngine,
};

// ui / components
pub use crate::ui::{
    Alert, AlertType, Anchor, AnchorItem, AutoComplete, Avatar, Badge, BarChart, BarData,
    Breadcrumb, BreadcrumbItem, Button, ButtonSize, Calendar, Card, Carousel, Cascader,
    CascaderOption, Checkbox, Collapse, CollapsePanel, ColorPicker, Container, DatePicker,
    DateValue, Descriptions, DescriptionsItem, Divider, Drawer, Dropdown, Empty, Form, Grid, Icon,
    Image, Input, InputNumber, InputSize, Label, LineChart, LineData, List, Mentions, Menu,
    MenuItem, MenuMode, Modal, Navigation, Pagination, PieChart, PieData, Popconfirm, Popover,
    ProgressBar, QRCode, Radio, Rate, Result, ResultType, ScrollDirection, ScrollView,
    Segmented, Select, SharedActive, Skeleton, SkeletonShape, Slider, Space, SpaceSize, Spin,
    Splitter, Step, StepStatus, Steps, Switch, TabPosition, Tabs, Tag, TagColor, ThemeToggle,
    TimePicker, TimeValue, Timeline, TimelineItem, Tooltip, TooltipPlacement, Tree, TreeNode,
    Typography, Watermark,
};

// ── app ───────────────────────────────────────────────────────

pub use crate::app::{map_ui_event, App, AppMode};
pub use crate::app::Container as DiContainer;
pub use crate::app::event_loop::run_widget_loop;

// ── view ──────────────────────────────────────────────────────

pub use crate::ui::view::{
    button, column, dynamic_label, input, label, row, space, StyleExt, Ui, View, ViewNode,
};
