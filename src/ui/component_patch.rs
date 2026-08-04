use crate::ui::form::{Form, FormItem};
use crate::ui::virtualization::virtual_scroll::VirtualScroll;
use crate::ui::widgets::window_chrome::WindowInteractionRegion;
use crate::ui::widgets::{
    Affix, Alert, Anchor, AutoComplete, Avatar, BackTop, Badge, Breadcrumb, Button, Calendar, Card,
    Carousel, Cascader, Checkbox, Collapse, ColorPicker, Container, Content, DatePicker,
    DateRangePicker, Descriptions, Divider, Drawer, Dropdown, Empty, FloatButton, FloatButtonGroup,
    Footer, Grid, Header, Icon, Image, ImageGroup, Input, InputNumber, Label, Layout, List,
    Mentions, Menu, Message, Modal, NavItem, Notification, Pagination, Popconfirm, Popover,
    ProgressBar, Radio, RangeSlider, Rate, ResultView, ScrollView, Segmented, Select,
    SelectableList, Sider, Skeleton, Slider, Space, Spin, Splitter, Steps, Switch, Tabs, Tag,
    ThemeToggle, TimePicker, Timeline, Tooltip, Transfer, Tree, TreeSelect, Typography, Upload,
    Watermark,
};
// 图表 capability 启用时才引入对应 patch 目标类型。
#[cfg(feature = "charts")]
// 该导入覆盖基础图表与高级图表共用的占位组件。
use crate::ui::widgets::{BarChart, ChartPlaceholder, LineChart, PieChart};
// 表格 capability 启用时才引入对应 patch 目标类型。
#[cfg(feature = "table")]
// 该导入仅供内建组件 patch 分派使用。
use crate::ui::widgets::Table;
// 二维码 capability 启用时才引入对应组件类型。
#[cfg(feature = "qrcode")]
// 该导入仅供内建组件 patch 分派使用。
use crate::ui::widgets::QRCode;
// 富文本 capability 启用时才引入对应组件类型。
#[cfg(feature = "rich-text")]
// 该导入仅供内建组件 patch 分派使用。
use crate::ui::widgets::RichText;
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
            SnapshotFields::Collapse {
                panels: current_panels,
                accordion: current_accordion,
                ..
            },
            SnapshotFields::Collapse {
                panels: next_panels,
                accordion: next_accordion,
                ..
            },
        ) => Some(
            current_accordion != next_accordion
                || current_panels.len() != next_panels.len()
                || current_panels
                    .iter()
                    .zip(&next_panels)
                    .any(|(current, next)| {
                        current.header != next.header || current.content != next.content
                    }),
        ),
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
        (
            SnapshotFields::ImageGroup {
                images: current_images,
                start_index: current_start,
                ..
            },
            SnapshotFields::ImageGroup {
                images: next_images,
                start_index: next_start,
                ..
            },
        ) => Some(current_images != next_images || current_start != next_start),
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
    patch_as!(RangeSlider);
    patch_as!(InputNumber);
    patch_as!(Rate);
    patch_as!(Segmented);
    patch_as!(FormItem);
    patch_as!(Form);
    // 表格 capability 启用时才生成类型化 patch 分支。
    #[cfg(feature = "table")]
    // 启用后保持表格组件的原位同步语义。
    patch_as!(Table);
    patch_as!(ProgressBar);
    patch_as!(Spin);
    patch_as!(FloatButton);
    patch_as!(FloatButtonGroup);
    patch_as!(BackTop);
    patch_as!(Alert);
    patch_as!(Tag);
    patch_as!(Empty);
    patch_as!(Skeleton);
    patch_as!(Card);
    patch_as!(Image);
    patch_as!(ImageGroup);
    patch_as!(Calendar);
    patch_as!(Collapse);
    patch_as!(Carousel);
    patch_as!(Descriptions);
    patch_as!(ResultView);
    patch_as!(List);
    patch_as!(Timeline);
    patch_as!(Tree);
    patch_as!(SelectableList);
    // 图表 capability 启用时才生成柱状图类型化 patch 分支。
    #[cfg(feature = "charts")]
    // 启用后保持柱状图的原位同步语义。
    patch_as!(BarChart);
    // 图表 capability 启用时才生成折线图类型化 patch 分支。
    #[cfg(feature = "charts")]
    // 启用后保持折线图的原位同步语义。
    patch_as!(LineChart);
    // 图表 capability 启用时才生成饼图类型化 patch 分支。
    #[cfg(feature = "charts")]
    // 启用后保持饼图的原位同步语义。
    patch_as!(PieChart);
    // 图表 capability 启用时才生成高级图表占位组件 patch 分支。
    #[cfg(feature = "charts")]
    // 启用后保持高级图表的原位同步语义。
    patch_as!(ChartPlaceholder);
    // 关闭二维码 capability 时不生成类型化 patch 分支。
    #[cfg(feature = "qrcode")]
    // 启用后保持二维码组件的原位同步语义。
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
    // 富文本 capability 启用时才参与内建组件 patch 分派。
    #[cfg(feature = "rich-text")]
    patch_as!(RichText);
    patch_as!(ThemeToggle);
    patch_as!(Transfer);
    patch_as!(Upload);

    Err(next)
}
