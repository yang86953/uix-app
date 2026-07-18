use crate::ui::foundation::virtual_scroll::VirtualScroll;
use crate::ui::widgets::{
    Affix, Alert, Anchor, AutoComplete, Avatar, BackTop, Badge, BarChart, Breadcrumb, Button,
    Calendar, Card, Carousel, Cascader, Checkbox, Collapse, ColorPicker, Container, Content,
    DatePicker, DateRangePicker, Descriptions, Divider, Drawer, Dropdown, Empty, FloatButton,
    Footer, Form, FormItem, Grid, Header, Icon, Image, Input, InputNumber, Label, Layout,
    LineChart, List, Mentions, Menu, Message, Modal, NavItem, Notification, Pagination, PieChart,
    Popconfirm, Popover, ProgressBar, QRCode, Radio, Rate, ResultView, RichText, ScrollView,
    Segmented, Select, SelectableList, Sider, Skeleton, Slider, Space, Spin, Splitter, Steps,
    Switch, Table, Tabs, Tag, ThemeToggle, TimePicker, Timeline, Tooltip, Transfer, Tree,
    TreeSelect, Typography, Upload, Watermark,
};
use crate::ui::window_chrome::WindowInteractionRegion;
use crate::ui::SnapshotFields;
use crate::ui::WidgetComponent;

/// Compare authored configuration for widgets whose public snapshot also
/// contains runtime state. `None` falls back to ordinary snapshot equality.
pub(crate) fn builtin_widget_config_changed(
    current: &dyn WidgetComponent,
    next: &dyn WidgetComponent,
) -> Option<bool> {
    match (current.snapshot_fields(), next.snapshot_fields()) {
        (
            SnapshotFields::Carousel {
                show_dots: current_dots,
                show_arrows: current_arrows,
                fixed_width: current_width,
                fixed_height: current_height,
                ..
            },
            SnapshotFields::Carousel {
                show_dots: next_dots,
                show_arrows: next_arrows,
                fixed_width: next_width,
                fixed_height: next_height,
                ..
            },
        ) => Some(
            current_dots != next_dots
                || current_arrows != next_arrows
                || current_width != next_width
                || current_height != next_height,
        ),
        _ => None,
    }
}

pub(crate) fn builtin_widget_runtime_changed(
    current: &dyn WidgetComponent,
    next: &dyn WidgetComponent,
) -> bool {
    if let (Some(current), Some(next)) = (
        current.as_any().downcast_ref::<Input>(),
        next.as_any().downcast_ref::<Input>(),
    ) {
        return current.controlled_value_changed(next);
    }
    false
}

pub(crate) fn patch_builtin_widget(
    current: &mut dyn WidgetComponent,
    next: Box<dyn WidgetComponent>,
) -> std::result::Result<bool, Box<dyn WidgetComponent>> {
    macro_rules! patch_as {
        ($ty:ty) => {
            if current.as_any().is::<$ty>() && next.as_any().is::<$ty>() {
                let Some(current) = current.as_any_mut().downcast_mut::<$ty>() else {
                    return Err(next);
                };
                let Ok(next) = next.into_any().downcast::<$ty>() else {
                    return Ok(false);
                };
                current.sync_from(*next);
                return Ok(true);
            }
        };
    }

    patch_as!(Button);
    patch_as!(VirtualScroll);
    patch_as!(Container);
    patch_as!(WindowInteractionRegion);
    patch_as!(Grid);
    patch_as!(Layout);
    patch_as!(Header);
    patch_as!(Sider);
    patch_as!(Content);
    patch_as!(Footer);
    patch_as!(Affix);
    patch_as!(Splitter);
    patch_as!(Divider);
    patch_as!(Icon);
    patch_as!(Typography);
    patch_as!(Avatar);
    patch_as!(Badge);
    patch_as!(Input);
    patch_as!(Checkbox);
    patch_as!(Radio);
    patch_as!(Switch);
    patch_as!(Slider);
    patch_as!(InputNumber);
    patch_as!(Rate);
    patch_as!(Segmented);
    patch_as!(FormItem);
    patch_as!(Form);
    patch_as!(Table);
    patch_as!(ProgressBar);
    patch_as!(Spin);
    patch_as!(FloatButton);
    patch_as!(BackTop);
    patch_as!(Alert);
    patch_as!(Tag);
    patch_as!(Empty);
    patch_as!(Skeleton);
    patch_as!(Card);
    patch_as!(Image);
    patch_as!(Calendar);
    patch_as!(Collapse);
    patch_as!(Carousel);
    patch_as!(Descriptions);
    patch_as!(ResultView);
    patch_as!(List);
    patch_as!(Timeline);
    patch_as!(Tree);
    patch_as!(SelectableList);
    patch_as!(BarChart);
    patch_as!(LineChart);
    patch_as!(PieChart);
    patch_as!(QRCode);
    patch_as!(Watermark);
    patch_as!(Label);
    patch_as!(ScrollView);
    patch_as!(Select);
    patch_as!(Space);
    patch_as!(Tooltip);
    patch_as!(Popover);
    patch_as!(Popconfirm);
    patch_as!(Modal);
    patch_as!(Drawer);
    patch_as!(Dropdown);
    patch_as!(Menu);
    patch_as!(Tabs);
    patch_as!(Pagination);
    patch_as!(Breadcrumb);
    patch_as!(NavItem);
    patch_as!(Anchor);
    patch_as!(Steps);
    patch_as!(TreeSelect);
    patch_as!(Cascader);
    patch_as!(ColorPicker);
    patch_as!(AutoComplete);
    patch_as!(DatePicker);
    patch_as!(DateRangePicker);
    patch_as!(TimePicker);
    patch_as!(Mentions);
    patch_as!(Message);
    patch_as!(Notification);
    patch_as!(RichText);
    patch_as!(ThemeToggle);
    patch_as!(Transfer);
    patch_as!(Upload);

    Err(next)
}
