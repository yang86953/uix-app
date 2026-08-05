use crate::ui::form::{Form, FormItem};
use crate::ui::virtualization::virtual_scroll::VirtualScroll;
use crate::ui::widgets::window_chrome::WindowInteractionRegion;
use crate::ui::widgets::{
    Affix, AutoComplete, Avatar, BackTop, Badge, Button, Calendar, Card, Carousel, Cascader,
    Checkbox, Collapse, ColorPicker, Container, Content, DatePicker, DateRangePicker, Descriptions,
    Divider, Empty, FloatButton, FloatButtonGroup, Footer, Grid, Header, Icon, Image, ImageGroup,
    Input, InputNumber, Label, Layout, List, Mentions, Radio, RangeSlider, Rate, ResultView,
    ScrollView, Segmented, Select, SelectableList, Sider, Skeleton, Slider, Space, Splitter,
    Switch, Tag, ThemeToggle, TimePicker, Timeline, Transfer, Typography, Upload, Watermark,
};
// 反馈 capability 启用时才引入对应 patch 目标类型。
#[cfg(feature = "feedback")]
// 该导入覆盖反馈组件族全部拥有运行时状态的组件。
use crate::ui::widgets::{
    Alert, Drawer, Message, Modal, Notification, Popconfirm, Popover, ProgressBar, Spin, Tooltip,
};
// 图表 capability 启用时才引入对应 patch 目标类型。
#[cfg(feature = "charts")]
// 该导入覆盖基础图表与高级图表共用的占位组件。
use crate::ui::widgets::{BarChart, ChartPlaceholder, LineChart, PieChart};
// 表格 capability 启用时才引入对应 patch 目标类型。
#[cfg(feature = "table")]
// 该导入仅供内建组件 patch 分派使用。
use crate::ui::widgets::Table;
// 导航 capability 启用时才引入对应 patch 目标类型。
#[cfg(feature = "navigation")]
// 该导入覆盖全部拥有运行时组件状态的导航目标。
use crate::ui::widgets::{Anchor, Breadcrumb, Dropdown, Menu, NavItem, Pagination, Steps, Tabs};
// 二维码 capability 启用时才引入对应组件类型。
#[cfg(feature = "qrcode")]
// 该导入仅供内建组件 patch 分派使用。
use crate::ui::widgets::QRCode;
// 富文本 capability 启用时才引入对应组件类型。
#[cfg(feature = "rich-text")]
// 该导入仅供内建组件 patch 分派使用。
use crate::ui::widgets::RichText;
// 树组件 capability 启用时才引入展示树与树选择器 patch 目标。
#[cfg(feature = "tree-widgets")]
// 两个类型共享同一源码与公开面门禁。
use crate::ui::widgets::{Tree, TreeSelect};
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
    // 反馈 capability 启用时才生成进度条类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(ProgressBar);
    // 反馈 capability 启用时才生成加载指示器类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Spin);
    patch_as!(FloatButton);
    patch_as!(FloatButtonGroup);
    patch_as!(BackTop);
    // 反馈 capability 启用时才生成警告提示类型化 patch 分支。
    #[cfg(feature = "feedback")]
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
    // 树组件 capability 启用时才生成展示树类型化 patch 分支。
    #[cfg(feature = "tree-widgets")]
    // 启用后保持展示树的原位同步语义。
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
    // 反馈 capability 启用时才生成文字提示类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Tooltip);
    // 反馈 capability 启用时才生成气泡卡片类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Popover);
    // 反馈 capability 启用时才生成气泡确认框类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Popconfirm);
    // 反馈 capability 启用时才生成对话框类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Modal);
    // 反馈 capability 启用时才生成抽屉类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Drawer);
    // 导航 capability 启用时才生成下拉菜单类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Dropdown);
    // 导航 capability 启用时才生成菜单类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Menu);
    // 导航 capability 启用时才生成标签页类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Tabs);
    // 导航 capability 启用时才生成分页类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Pagination);
    // 导航 capability 启用时才生成面包屑类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Breadcrumb);
    // 导航 capability 启用时才生成导航项类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(NavItem);
    // 导航 capability 启用时才生成锚点类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Anchor);
    // 导航 capability 启用时才生成步骤条类型化 patch 分支。
    #[cfg(feature = "navigation")]
    patch_as!(Steps);
    // 树组件 capability 启用时才生成树选择器类型化 patch 分支。
    #[cfg(feature = "tree-widgets")]
    // 启用后保持树选择器的原位同步语义。
    patch_as!(TreeSelect);
    patch_as!(Cascader);
    patch_as!(ColorPicker);
    patch_as!(AutoComplete);
    patch_as!(DatePicker);
    patch_as!(DateRangePicker);
    patch_as!(TimePicker);
    patch_as!(Mentions);
    // 反馈 capability 启用时才生成全局消息容器类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Message);
    // 反馈 capability 启用时才生成通知容器类型化 patch 分支。
    #[cfg(feature = "feedback")]
    patch_as!(Notification);
    // 富文本 capability 启用时才参与内建组件 patch 分派。
    #[cfg(feature = "rich-text")]
    patch_as!(RichText);
    patch_as!(ThemeToggle);
    patch_as!(Transfer);
    patch_as!(Upload);

    Err(next)
}
