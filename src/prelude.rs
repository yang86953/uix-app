//! Unified public entrypoint with common re-exports grouped by domain.
//!
//! Most apps only need:
//! ```ignore
//! use uix::prelude::*;
//! ```

// core

pub use crate::core::{
    ComponentId, Constraints, EdgeInsets, Errc, Error, Point, Rect, Size, WindowId,
};

// native

pub use crate::native::create_platform;
pub use crate::native::traits::input::{ControlSize, KeyCode, KeyMod, MouseButton};
pub use crate::native::traits::system::StatusLevel;
// draw

pub use crate::draw::traits::GraphicsEngine;
pub use crate::draw::{colors, Color, FontService, ImageService, NullEngine, SoftwareEngine};

// ui

pub use crate::component;
pub use crate::impl_widget_component;
pub use crate::tree;
pub use crate::ui::layout::{
    AlignItems, FlexDirection, FlexLayout, GridTrack, JustifyContent, LayoutChild, LayoutEngine,
};
pub use crate::ui::state::{Computed, State};
pub use crate::ui::style::{ColorValue, PaletteColor, Style, StyleSet, TypographyToken};
pub use crate::ui::theme::{DesignTokens, DynTokens, NeutralRole, Theme};
pub use crate::ui::{
    AppState, ClickEvent, ComponentHandle, EventResult, HandlerId, HandlerOptions,
    HandlerRegistration, IntoWidgetNode, PaintContext, SemanticEvent, SemanticKind,
    SemanticPayload, SystemEvent, SystemEventKind, WidgetCore, WidgetId, WidgetNode, WidgetTree,
};
pub use crate::ui::{
    DragManager, FocusManager, InteractionManager, StateManager, StyleManager, TextManager,
    WidgetManagers,
};

// ui / components
pub use crate::ui::{
    Affix, Alert, Anchor, AnchorItem, AutoComplete, Avatar, BackTop, Badge, BadgeStatus, BarChart,
    BarData, Breadcrumb, BreadcrumbItem, Button, Calendar, Card, Carousel, Cascader,
    CascaderOption, CascaderValue, Checkbox, Collapse, CollapsePanel, ColorPicker, Container,
    Content, DatePicker, DateValue, Descriptions, DescriptionsItem, Divider, DividerDirection,
    DividerOrientation, Drawer, DrawerPlacement, Dropdown, Empty, FieldDef, FloatButton,
    FloatButtonBackTop, Footer, Form, FormItem, FormLayout, Grid, Header, Icon, Image, Input,
    InputNumber, Label, Layout, LineChart, LineData, List, Mentions, Menu, MenuItem, MenuMode,
    Message, MessageItem, MessagePlacement, Modal, NavGroup, NavItem, Navigation, NotifPlacement,
    Notification, NotificationItem, OptGroup, Pagination, PieChart, PieData, Popconfirm,
    PopconfirmPlacement, Popover, PopoverPlacement, PopoverTrigger, ProgressBar, ProgressMode,
    ProgressType, QRCode, Radio, RadioDirection, Rate, Result, ResultType, RichText,
    RichTextSegment, RichTextStyle, ScrollDirection, ScrollView, Segmented, Select, SelectableItem,
    SelectableList, SharedActive, Sider, Skeleton, SkeletonShape, Slider, SortDirection, Space,
    SpaceSize, Spin, SpinSize, Splitter, Step, StepStatus, Steps, Switch, Tab, TabPosition, Table,
    TableChange, TableColumn, Tabs, Tag, TagColor, ThemeToggle, TimePicker, TimeValue, Timeline,
    TimelineItem, Tooltip, TooltipPlacement, Transfer, TransferItem, Tree, TreeNode, TreeSelect,
    TriggerMode, Typography, TypographyType, Upload, UploadFile, UploadStatus, ValidateStatus,
    ValidationResult, ValidationRule, Watermark,
};

// app

pub use crate::app::event_loop::run_widget_loop;
pub use crate::app::Container as DiContainer;
pub use crate::app::{map_ui_event, App, AppHandle, AppMode, TimerHandle, WindowConfig};

// view

pub use crate::ui::view::{
    button, column, dynamic_label, grid, input, label, row, scroll, space, ButtonBuilder,
    GridBuilder, InputBuilder, ScrollBuilder, StyleExt, Ui, View, ViewAdapter, ViewNode,
};

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    component! {
        struct PreludeComponentProbe {
            pub label: String,
        }

        render => (
            &self,
            _frame: Rect,
            _ctx: &mut PaintContext,
            _tree: &crate::ui::core::widget::WidgetTree
        ) {}
    }

    #[test]
    fn prelude_exports_event_and_state_capture_types() {
        let _probe = PreludeComponentProbe {
            label: "probe".to_string(),
        };
        let state = State::new(1);
        let _registration = HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {}))
            .with_state_capture(&state);
        let _node = label("captured").on_semantic_capture(SemanticKind::Click, &state, |_| {});

        fn assert_exported<T>() {}
        assert_exported::<AppHandle>();
        assert_exported::<TimerHandle>();
        assert_exported::<ComponentId>();
        assert_exported::<ComponentHandle>();
        assert_exported::<Constraints>();
        assert_exported::<SemanticEvent>();
        assert_exported::<ButtonBuilder>();
        assert_exported::<InputBuilder>();
        assert_exported::<ViewAdapter>();
        assert_exported::<BadgeStatus>();
        assert_exported::<CascaderValue>();
        assert_exported::<DividerDirection>();
        assert_exported::<DividerOrientation>();
        assert_exported::<DrawerPlacement>();
        assert_exported::<FieldDef>();
        assert_exported::<FormItem>();
        assert_exported::<FormLayout>();
        assert_exported::<NavGroup>();
        assert_exported::<NavItem>();
        assert_exported::<OptGroup>();
        assert_exported::<PopconfirmPlacement>();
        assert_exported::<PopoverPlacement>();
        assert_exported::<PopoverTrigger>();
        assert_exported::<ProgressMode>();
        assert_exported::<ProgressType>();
        assert_exported::<RadioDirection>();
        assert_exported::<SpinSize>();
        assert_exported::<Tab>();
        assert_exported::<TriggerMode>();
        assert_exported::<TypographyType>();
        assert_exported::<ValidateStatus>();
        assert_exported::<ValidationResult>();
        assert_exported::<ValidationRule>();
        assert_exported::<FloatButton>();
        assert_exported::<Affix>();
        assert_exported::<BackTop>();
        assert_exported::<Layout>();
        assert_exported::<Header>();
        assert_exported::<Sider>();
        assert_exported::<Content>();
        assert_exported::<Footer>();
        assert_exported::<SelectableList>();
        assert_exported::<SelectableItem>();
        assert_exported::<Table>();
        assert_exported::<TableColumn>();
        assert_exported::<TableChange>();
        assert_exported::<SortDirection>();
        assert_exported::<TreeSelect>();
        assert_exported::<Message>();
        assert_exported::<MessageItem>();
        assert_exported::<MessagePlacement>();
        assert_exported::<Notification>();
        assert_exported::<NotificationItem>();
        assert_exported::<NotifPlacement>();
        assert_exported::<QRCode>();
        assert_exported::<RichText>();
        assert_exported::<RichTextSegment>();
        assert_exported::<RichTextStyle>();
        assert_exported::<SharedActive>();
        assert_exported::<ThemeToggle>();
        assert_exported::<Transfer>();
        assert_exported::<TransferItem>();
        assert_exported::<Upload>();
        assert_exported::<UploadFile>();
        assert_exported::<UploadStatus>();
        assert_exported::<Watermark>();
    }
}
