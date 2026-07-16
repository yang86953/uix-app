pub use crate::app::Container as DiContainer;
pub use crate::app::{App, AppHandle, AppMode, TimerHandle, WindowConfig};
pub use crate::component;
pub use crate::draw::traits::GraphicsEngine;
use crate::prelude::*;
pub use crate::semantic_handler;
use crate::tests::common::*;
pub use crate::ui::layout::{
    BoxModel, FlexLayout, GridLayout, GridTrack, LayoutChild, LayoutEngine, LayoutOutput,
};
pub use crate::ui::state::{Computed, Effect, State, StateSlotId};
pub use crate::ui::style::{
    BoxShadowDef, ColorValue, DisplayMode, PaletteColor, StyleSet, StyleState, TypographyToken,
};
pub use crate::ui::theme::{NeutralRole, ShadowToken, ThemePrimitives};
pub use crate::ui::view::{
    label, AccessibilityExt, ButtonBuilder, EventExt, GridBuilder, InputBuilder, ScrollBuilder,
    StyleExt, Ui, View, ViewNode,
};
use crate::ui::widgets::Container;
pub use crate::ui::{
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AriaAttribute, ClickEvent,
    ComponentConfigSnapshot, ComponentHandle, HandlerId, HandlerOptions, HandlerRegistration,
    IntoWidgetNode, SemanticPayload, SnapshotCollapsePanel, SnapshotField, SnapshotSource,
    SnapshotTableColumn, SnapshotTableColumnGroup, SnapshotTransferItem, SnapshotTreeNode,
    SnapshotValue, SystemEventKind, WidgetChildren,
};
pub use crate::ui::{
    Affix, Alert, Anchor, AnchorItem, AutoComplete, Avatar, BackTop, Badge, BadgeStatus, BarChart,
    BarData, Breadcrumb, BreadcrumbItem, BreakpointError, Breakpoints, Button, Calendar, Card,
    Carousel, Cascader, CascaderOption, CascaderValue, Checkbox, Col, Collapse, CollapsePanel,
    ColorPicker, Content, Date, DatePicker, DateRangePicker, Descriptions, DescriptionsItem,
    Divider, DividerDirection, DividerOrientation, Drawer, DrawerPlacement, Dropdown, Empty,
    FieldError, FloatButton, FloatButtonBackTop, Footer, Form, FormBuilder, FormCheckboxItem,
    FormColorPickerItem, FormDatePickerItem, FormDateRangePickerItem, FormInputItem,
    FormInputNumberItem, FormItem, FormLayout, FormListBuilder, FormListError, FormListFieldError,
    FormListFields, FormListItemId, FormListModel, FormListValues, FormModel, FormRadioItem,
    FormRateItem, FormSegmentedItem, FormSelectItem, FormSliderItem, FormSwitchItem,
    FormTimePickerItem, Grid, Header, Icon, Image, Input, InputNumber, InputNumberValue,
    IntoFormValue, Label, Layout, LineChart, LineData, List, Mentions, Menu, MenuItem, MenuMode,
    Message, MessageItem, Modal, ModalBuilder, ModalContext, NavGroup, NavItem, Navigation,
    Notification, NotificationItem, OptGroup, Pagination, PickerMode, PieChart, PieData,
    Popconfirm, PopconfirmPlacement, Popover, PopoverPlacement, PopoverTrigger, PresetDate,
    ProgressBar, ProgressMode, ProgressType, QRCode, Radio, RadioDirection, Rate, ResultType,
    ResultView, RichText, RichTextSegment, RichTextStyle, ScrollView, Segmented, Select,
    SelectableItem, SelectableList, SharedActive, Sider, Skeleton, SkeletonShape, Slider,
    SortDirection, Space, SpaceSize, Spin, SpinSize, Splitter, Step, StepStatus, Steps, Switch,
    Tab, TabPosition, Table, TableBuilder, TableChange, TableColumn, TableRow, Tabs, Tag, TagColor,
    ThemeToggle, Time, TimePicker, Timeline, TimelineItem, Tooltip, TooltipPlacement, Transfer,
    TransferItem, Tree, TreeNode, TreeSelect, TriggerMode, Typography, TypographyType, Upload,
    UploadFile, UploadStatus, ValidateStatus, Values, VirtualScroll, VirtualScrollBuilder,
    Watermark, Weekday,
};
pub use crate::ui::{
    Animation, Easing, Keyframe, KeyframeAnimation, KeyframeError, Spring, SpringAnimation,
};

fn assert_input_number_value<T: InputNumberValue>() {}
fn assert_form_value<T: IntoFormValue>() {}

component! {
    struct PreludeComponentProbe {
        pub label: String,
    }

    render => (
        &self,
        _frame: Rect,
        _ctx: &mut PaintContext
    ) {}
}

#[test]
fn prelude_exports_event_and_state_capture_types() {
    let _generated_palette = generate_color_scale(Color::BLUE);
    let _neutral = NEUTRAL_PALETTE;
    let _data_visualization = DATA_VISUALIZATION_PALETTE;
    let _probe = PreludeComponentProbe {
        label: "probe".to_string(),
    };
    let state = State::new(1);
    let _registration =
        HandlerRegistration::new(SemanticKind::Click, Box::new(|_| {})).with_state_capture(&state);
    let _node = label("captured").on_semantic_capture(SemanticKind::Click, &state, |_| {});
    let _window_control = window_control_named(WindowControl::Close, "Close", label("×"));
    let _macro_registration = semantic_handler!(SemanticKind::Click, state[state], |_event| {});

    fn assert_exported<T: ?Sized>() {}
    fn assert_accessibility_ext<T: AccessibilityExt>() {}
    fn assert_event_ext<T: EventExt>() {}
    fn assert_style_ext<T: StyleExt>() {}
    fn assert_view<T: View>() {}
    assert_exported::<Animation<f32>>();
    assert_exported::<App>();
    assert_exported::<AppHandle>();
    assert_exported::<AppMode>();
    assert_exported::<WindowConfig>();
    assert_exported::<AccessibilityRole>();
    assert_exported::<AccessibilitySnapshot>();
    assert_exported::<AccessibilityState>();
    assert_exported::<AriaAttribute>();
    assert_exported::<DiContainer>();
    assert_exported::<TimerHandle>();
    assert_exported::<ComponentId>();
    assert_exported::<ComponentConfigSnapshot>();
    assert_exported::<ComponentConfig>();
    assert_exported::<ComponentOverrides>();
    assert_exported::<Config>();
    assert_exported::<ConfigProvider>();
    assert_exported::<ComponentHandle>();
    assert_exported::<FocusHandle>();
    assert_exported::<FocusHandleError>();
    assert_exported::<ErrorSeverity>();
    assert_exported::<Constraints>();
    assert_exported::<EdgeInsets>();
    assert_exported::<Errc>();
    assert_exported::<Error>();
    assert_exported::<Point>();
    assert_exported::<Rect>();
    assert_exported::<Size>();
    assert_exported::<WindowId>();
    assert_exported::<ControlSize>();
    assert_exported::<Locale>();
    assert_exported::<LocaleProvider>();
    assert_exported::<CursorType>();
    assert_exported::<KeyCode>();
    assert_exported::<KeyMod>();
    assert_exported::<MouseButton>();
    assert_exported::<ScrollDirection>();
    assert_exported::<GraphicsBackend>();
    assert_exported::<StatusLevel>();
    assert_exported::<Color>();
    assert_exported::<FontService>();
    assert_exported::<dyn GraphicsEngine>();
    assert_exported::<ImageService>();
    assert_exported::<NullEngine>();
    assert_exported::<SoftwareEngine>();
    assert_exported::<SettingsService>();
    assert_exported::<AlignItems>();
    assert_exported::<FlexDirection>();
    assert_exported::<JustifyContent>();
    assert_exported::<LayoutChild>();
    assert_exported::<dyn LayoutEngine>();
    assert_exported::<SemanticEvent>();
    assert_exported::<SemanticKind>();
    assert_exported::<SemanticPayload>();
    assert_exported::<SystemEvent>();
    assert_exported::<SystemEventKind>();
    assert_exported::<AppState>();

    let _: fn(&str) = copy_to_clipboard;
    let _: fn() -> Option<String> = read_text_from_clipboard;
    assert_exported::<ClickEvent>();
    assert_exported::<EventResult>();
    assert_exported::<HandlerId>();
    assert_exported::<HandlerOptions>();
    assert_exported::<HandlerRegistration>();
    assert_exported::<SnapshotCollapsePanel>();
    assert_exported::<SnapshotField>();
    assert_exported::<SnapshotFields>();
    assert_exported::<dyn SnapshotSource>();
    assert_exported::<SnapshotTableColumn>();
    assert_exported::<SnapshotTableColumnGroup>();
    assert_exported::<SnapshotTransferItem>();
    assert_exported::<SnapshotTreeNode>();
    assert_exported::<SnapshotValue>();
    assert_exported::<PaintContext>();
    assert_exported::<FocusTrap>();
    assert_exported::<Computed<i32>>();
    assert_exported::<State<i32>>();
    assert_exported::<ButtonBuilder>();
    assert_exported::<GridBuilder>();
    assert_exported::<InputBuilder>();
    assert_exported::<ScrollBuilder>();
    assert_exported::<dyn IntoWidgetNode>();
    assert_exported::<dyn WidgetComponent>();
    assert_accessibility_ext::<ViewNode>();
    assert_event_ext::<ViewNode>();
    assert_style_ext::<ViewNode>();
    assert_view::<ViewNode>();
    assert_exported::<Ui<'static>>();
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
    assert_exported::<ColorScale>();
    assert_exported::<DataVisualizationPalette>();
    assert_exported::<NeutralRole>();
    assert_exported::<Theme>();
    assert_exported::<PrimaryHue>();
    assert_exported::<FunctionalColorRole>();
    assert_exported::<NeutralColorScale>();
    assert_exported::<TypographyToken>();
    assert_exported::<BadgeStatus>();
    assert_exported::<BarData>();
    assert_exported::<BreadcrumbItem>();
    assert_exported::<CascaderOption>();
    assert_exported::<CascaderValue>();
    assert_exported::<CollapsePanel>();
    assert_exported::<Date>();
    assert_exported::<DescriptionsItem>();
    assert_exported::<Fixed>();
    assert_exported::<DividerDirection>();
    assert_exported::<DividerOrientation>();
    assert_exported::<DrawerPlacement>();
    assert_exported::<DynTokens>();
    assert_exported::<Easing>();
    assert_exported::<Effect>();
    assert_exported::<FieldError>();
    assert_exported::<FormBuilder>();
    assert_exported::<FormCheckboxItem>();
    assert_exported::<FormColorPickerItem>();
    assert_exported::<FormDatePickerItem>();
    assert_exported::<FormDateRangePickerItem>();
    assert_exported::<FormInputItem>();
    assert_exported::<FormInputNumberItem<i32>>();
    assert_exported::<FormSelectItem<HashSet<String>>>();
    assert_exported::<FormSwitchItem>();
    assert_exported::<FormItem>();
    assert_exported::<FormLayout>();
    assert_exported::<FormListBuilder>();
    assert_exported::<FormListError>();
    assert_exported::<FormListFieldError>();
    assert_exported::<FormListFields>();
    assert_exported::<FormListItemId>();
    assert_exported::<FormListModel>();
    assert_exported::<FormListValues>();
    assert_exported::<FormModel>();
    assert_exported::<FormRadioItem>();
    assert_exported::<FormRateItem>();
    assert_exported::<FormSegmentedItem>();
    assert_exported::<FormSelectItem>();
    assert_exported::<FormSliderItem>();
    assert_exported::<FormTimePickerItem>();
    assert_exported::<Keyframe<f32>>();
    assert_exported::<KeyframeAnimation<f32>>();
    assert_exported::<KeyframeError>();
    assert_exported::<Spring>();
    assert_exported::<SpringAnimation<f32>>();
    assert_exported::<Trigger>();
    assert_form_value::<Color>();
    assert_form_value::<Date>();
    assert_form_value::<HashSet<String>>();
    assert_form_value::<String>();
    assert_form_value::<Time>();
    assert_exported::<LineData>();
    assert_exported::<MenuItem>();
    assert_exported::<MenuMode>();
    assert_exported::<ModalBuilder>();
    assert_exported::<ModalContext>();
    assert_exported::<NavGroup>();
    assert_exported::<NavItem>();
    assert_exported::<OptGroup>();
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
    assert_exported::<StatusLevel>();
    assert_exported::<Step>();
    assert_exported::<StepStatus>();
    assert_exported::<Tab>();
    assert_exported::<TabPosition>();
    assert_exported::<TagColor>();
    assert_exported::<TableColumnGroup>();
    assert_exported::<ThemePrimitives>();
    assert_exported::<Time>();
    assert_exported::<TimelineItem>();
    assert_exported::<TooltipPlacement>();
    assert_exported::<TreeNode>();
    assert_exported::<TriggerMode>();
    assert_exported::<TypographyType>();
    assert_exported::<ValidateStatus>();
    assert_exported::<Values>();
    assert_exported::<VirtualScroll>();
    assert_exported::<VirtualScrollBuilder>();
    assert_exported::<TableBuilder>();
    assert_exported::<Alert>();
    assert_exported::<Anchor>();
    assert_exported::<AutoComplete>();
    assert_exported::<Avatar>();
    assert_exported::<Badge>();
    assert_exported::<BarChart>();
    assert_exported::<Breadcrumb>();
    assert_exported::<Button>();
    assert_exported::<Calendar>();
    assert_exported::<Card>();
    assert_exported::<Carousel>();
    assert_exported::<Cascader>();
    assert_exported::<Checkbox>();
    assert_exported::<Collapse>();
    assert_exported::<ColorPicker>();
    assert_exported::<Container>();
    assert_exported::<DatePicker>();
    assert_exported::<DateRangePicker>();
    assert_exported::<PresetDate>();
    assert_exported::<PickerMode>();
    assert_exported::<Weekday>();
    assert_exported::<Descriptions>();
    assert_exported::<Divider>();
    assert_exported::<Drawer>();
    assert_exported::<Dropdown>();
    assert_exported::<Empty>();
    assert_exported::<Form>();
    assert_exported::<Grid>();
    assert_exported::<BreakpointError>();
    assert_exported::<Breakpoints>();
    assert_exported::<Col>();
    assert_exported::<Icon>();
    assert_exported::<Image>();
    assert_exported::<Input>();
    assert_exported::<InputNumber>();
    assert_input_number_value::<i32>();
    assert_input_number_value::<f64>();
    assert_exported::<Label>();
    assert_exported::<LineChart>();
    assert_exported::<List>();
    assert_exported::<Mentions>();
    assert_exported::<Menu>();
    assert_exported::<Modal>();
    assert_exported::<Pagination>();
    assert_exported::<PieChart>();
    assert_exported::<Popconfirm>();
    assert_exported::<Popover>();
    assert_exported::<Animated<f32>>();
    assert_exported::<AnimationConfig>();
    assert_exported::<ProgressBar>();
    assert_exported::<Radio>();
    assert_exported::<Rate>();
    assert_exported::<ResultView>();
    assert_exported::<ScrollView>();
    assert_exported::<Segmented>();
    assert_exported::<Select>();
    assert_exported::<Skeleton>();
    assert_exported::<Slider>();
    assert_exported::<Space>();
    assert_exported::<Spin>();
    assert_exported::<Splitter>();
    assert_exported::<Steps>();
    assert_exported::<Switch>();
    assert_exported::<Tabs>();
    assert_exported::<Tag>();
    assert_exported::<Timeline>();
    assert_exported::<TimePicker>();
    assert_exported::<Tooltip>();
    assert_exported::<Tree>();
    assert_exported::<Typography>();
    assert_exported::<FloatButton>();
    assert_exported::<FloatButtonBackTop>();
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
    assert_exported::<Placement>();
    assert_exported::<Navigation>();
    assert_exported::<Notification>();
    assert_exported::<NotificationItem>();
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

#[test]
fn prelude_does_not_shadow_standard_result() {
    fn fallible() -> Result<(), Error> {
        Ok(())
    }

    assert!(fallible().is_ok());
}
