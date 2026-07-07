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
pub use crate::native::traits::input::{
    ControlSize, CursorType, KeyCode, KeyMod, MouseButton, ScrollDirection,
};
pub use crate::native::traits::system::StatusLevel;
// draw

pub use crate::draw::traits::GraphicsEngine;
pub use crate::draw::{colors, Color, FontService, ImageService, NullEngine, SoftwareEngine};

// ui

pub use crate::component;
pub use crate::impl_widget_component;
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
pub use crate::ui::{Animation, Easing};
pub use crate::ui::{
    AppState, ClickEvent, ComponentConfigSnapshot, ComponentHandle, EventResult, HandlerId,
    HandlerOptions, HandlerRegistration, IntoWidgetNode, PaintContext, SemanticEvent, SemanticKind,
    SemanticPayload, SnapshotField, SnapshotFields, SnapshotValue, SystemEvent, SystemEventKind,
    WidgetChildren,
};
pub use crate::ui::{
    DragManager, FocusManager, InteractionManager, StateManager, TextManager, WidgetManagers,
};
pub use crate::ui::{OverlayEntry, OverlayId, OverlayKind, OverlayStack};

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
    RichTextSegment, RichTextStyle, ScrollView, Segmented, Select, SelectableItem, SelectableList,
    SharedActive, Sider, Skeleton, SkeletonShape, Slider, SortDirection, Space, SpaceSize, Spin,
    SpinSize, Splitter, Step, StepStatus, Steps, Switch, Tab, TabPosition, Table, TableChange,
    TableColumn, TableRow, Tabs, Tag, TagColor, ThemeToggle, TimePicker, TimeValue, Timeline,
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

        fn assert_exported<T: ?Sized>() {}
        assert_exported::<Animation<f32>>();
        assert_exported::<App>();
        assert_exported::<AppHandle>();
        assert_exported::<AppMode>();
        assert_exported::<WindowConfig>();
        assert_exported::<DiContainer>();
        assert_exported::<TimerHandle>();
        assert_exported::<ComponentId>();
        assert_exported::<ComponentConfigSnapshot>();
        assert_exported::<ComponentHandle>();
        assert_exported::<Constraints>();
        assert_exported::<EdgeInsets>();
        assert_exported::<Errc>();
        assert_exported::<Error>();
        assert_exported::<Point>();
        assert_exported::<Rect>();
        assert_exported::<Size>();
        assert_exported::<WindowId>();
        assert_exported::<ControlSize>();
        assert_exported::<CursorType>();
        assert_exported::<KeyCode>();
        assert_exported::<KeyMod>();
        assert_exported::<MouseButton>();
        assert_exported::<ScrollDirection>();
        assert_exported::<StatusLevel>();
        assert_exported::<Color>();
        assert_exported::<FontService>();
        assert_exported::<ImageService>();
        assert_exported::<NullEngine>();
        assert_exported::<SoftwareEngine>();
        assert_exported::<AlignItems>();
        assert_exported::<FlexDirection>();
        assert_exported::<JustifyContent>();
        assert_exported::<LayoutChild>();
        assert_exported::<SemanticEvent>();
        assert_exported::<SemanticKind>();
        assert_exported::<SemanticPayload>();
        assert_exported::<SystemEvent>();
        assert_exported::<SystemEventKind>();
        assert_exported::<AppState>();
        assert_exported::<ClickEvent>();
        assert_exported::<EventResult>();
        assert_exported::<HandlerId>();
        assert_exported::<HandlerOptions>();
        assert_exported::<HandlerRegistration>();
        assert_exported::<SnapshotField>();
        assert_exported::<SnapshotFields>();
        assert_exported::<SnapshotValue>();
        assert_exported::<Computed<i32>>();
        assert_exported::<State<i32>>();
        assert_exported::<ButtonBuilder>();
        assert_exported::<GridBuilder>();
        assert_exported::<InputBuilder>();
        assert_exported::<ScrollBuilder>();
        assert_exported::<ViewAdapter>();
        assert_exported::<ViewNode>();
        assert_exported::<BoxModel>();
        assert_exported::<BoxShadowDef>();
        assert_exported::<ColorValue>();
        assert_exported::<DisplayMode>();
        assert_exported::<FlexLayout>();
        assert_exported::<GridLayout>();
        assert_exported::<GridTrack>();
        assert_exported::<LayoutOutput>();
        assert_exported::<PaletteColor>();
        assert_exported::<Style>();
        assert_exported::<StyleSet>();
        assert_exported::<AnchorItem>();
        assert_exported::<DesignTokens>();
        assert_exported::<NeutralRole>();
        assert_exported::<Theme>();
        assert_exported::<TypographyToken>();
        assert_exported::<BadgeStatus>();
        assert_exported::<BarData>();
        assert_exported::<BreadcrumbItem>();
        assert_exported::<CascaderOption>();
        assert_exported::<CascaderValue>();
        assert_exported::<CollapsePanel>();
        assert_exported::<DateValue>();
        assert_exported::<DescriptionsItem>();
        assert_exported::<DividerDirection>();
        assert_exported::<DividerOrientation>();
        assert_exported::<DrawerPlacement>();
        assert_exported::<DragManager>();
        assert_exported::<DynTokens>();
        assert_exported::<Easing>();
        assert_exported::<Effect>();
        assert_exported::<FieldDef>();
        assert_exported::<FocusManager>();
        assert_exported::<FormItem>();
        assert_exported::<FormLayout>();
        assert_exported::<InteractionManager>();
        assert_exported::<LineData>();
        assert_exported::<MenuItem>();
        assert_exported::<MenuMode>();
        assert_exported::<NavGroup>();
        assert_exported::<NavItem>();
        assert_exported::<OptGroup>();
        assert_exported::<OverlayEntry>();
        assert_exported::<OverlayId>();
        assert_exported::<OverlayKind>();
        assert_exported::<OverlayStack>();
        assert_exported::<PieData>();
        assert_exported::<PopconfirmPlacement>();
        assert_exported::<PopoverPlacement>();
        assert_exported::<PopoverTrigger>();
        assert_exported::<ProgressMode>();
        assert_exported::<ProgressType>();
        assert_exported::<RadioDirection>();
        assert_exported::<ResultType>();
        assert_exported::<SkeletonShape>();
        assert_exported::<SpaceSize>();
        assert_exported::<StyleState>();
        assert_exported::<ShadowToken>();
        assert_exported::<SpinSize>();
        assert_exported::<StateSlotId>();
        assert_exported::<StateManager>();
        assert_exported::<StatusLevel>();
        assert_exported::<TextManager>();
        assert_exported::<Step>();
        assert_exported::<StepStatus>();
        assert_exported::<Tab>();
        assert_exported::<TabPosition>();
        assert_exported::<TagColor>();
        assert_exported::<ThemePrimitives>();
        assert_exported::<TimeValue>();
        assert_exported::<TimelineItem>();
        assert_exported::<TooltipPlacement>();
        assert_exported::<TreeNode>();
        assert_exported::<TriggerMode>();
        assert_exported::<TypographyType>();
        assert_exported::<ValidateStatus>();
        assert_exported::<ValidationResult>();
        assert_exported::<ValidationRule>();
        assert_exported::<WidgetManagers>();
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
        assert_exported::<TableRow>();
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
        assert_exported::<WidgetChildren>();
    }
}
