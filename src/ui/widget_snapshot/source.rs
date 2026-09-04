use std::any::Any;

use crate::ui::form::{Form, FormItem};
use crate::ui::widgets::*;

use super::SnapshotFields;

/// 允许组件直接导出其类型化配置与运行状态快照。
pub trait SnapshotSource {
    /// 捕获组件当前的类型专属快照字段。
    fn snapshot_fields(&self) -> SnapshotFields;
}

/// 按已登记内置组件类型捕获快照；未知类型返回 [`SnapshotFields::Unknown`]。
pub fn snapshot_fields_from_any(widget: &dyn Any) -> SnapshotFields {
    if let Some(button) = widget.downcast_ref::<Button>() {
        return button.snapshot_fields();
    }
    if let Some(label) = widget.downcast_ref::<Label>() {
        return label.snapshot_fields();
    }
    if let Some(input) = widget.downcast_ref::<Input>() {
        return input.snapshot_fields();
    }
    if let Some(space) = widget.downcast_ref::<Space>() {
        return space.snapshot_fields();
    }
    if let Some(divider) = widget.downcast_ref::<Divider>() {
        return divider.snapshot_fields();
    }
    if let Some(icon) = widget.downcast_ref::<Icon>() {
        return icon.snapshot_fields();
    }
    if let Some(typography) = widget.downcast_ref::<Typography>() {
        return typography.snapshot_fields();
    }
    if let Some(checkbox) = widget.downcast_ref::<Checkbox>() {
        return checkbox.snapshot_fields();
    }
    if let Some(radio) = widget.downcast_ref::<Radio>() {
        return radio.snapshot_fields();
    }
    if let Some(switch) = widget.downcast_ref::<Switch>() {
        return switch.snapshot_fields();
    }
    if let Some(slider) = widget.downcast_ref::<Slider>() {
        return slider.snapshot_fields();
    }
    if let Some(slider) = widget.downcast_ref::<RangeSlider>() {
        return slider.snapshot_fields();
    }
    if let Some(rate) = widget.downcast_ref::<Rate>() {
        return rate.snapshot_fields();
    }
    if let Some(input_number) = widget.downcast_ref::<InputNumber>() {
        return input_number.snapshot_fields();
    }
    if let Some(avatar) = widget.downcast_ref::<Avatar>() {
        return avatar.snapshot_fields();
    }
    if let Some(badge) = widget.downcast_ref::<Badge>() {
        return badge.snapshot_fields();
    }
    if let Some(card) = widget.downcast_ref::<Card>() {
        return card.snapshot_fields();
    }
    if let Some(empty) = widget.downcast_ref::<Empty>() {
        return empty.snapshot_fields();
    }
    if let Some(image) = widget.downcast_ref::<Image>() {
        return image.snapshot_fields();
    }
    if let Some(image_group) = widget.downcast_ref::<ImageGroup>() {
        return image_group.snapshot_fields();
    }
    if let Some(tag) = widget.downcast_ref::<Tag>() {
        return tag.snapshot_fields();
    }
    if let Some(timeline) = widget.downcast_ref::<Timeline>() {
        return timeline.snapshot_fields();
    }
    if let Some(calendar) = widget.downcast_ref::<Calendar>() {
        return calendar.snapshot_fields();
    }
    if let Some(skeleton) = widget.downcast_ref::<Skeleton>() {
        return skeleton.snapshot_fields();
    }
    if let Some(float_button) = widget.downcast_ref::<FloatButton>() {
        return float_button.snapshot_fields();
    }
    if let Some(float_button_group) = widget.downcast_ref::<FloatButtonGroup>() {
        return float_button_group.snapshot_fields();
    }
    // 反馈 capability 启用时才引用警告提示组件类型。
    #[cfg(feature = "feedback")]
    if let Some(alert) = widget.downcast_ref::<Alert>() {
        return alert.snapshot_fields();
    }
    // 反馈 capability 启用时才引用全局消息组件类型。
    #[cfg(feature = "feedback")]
    if let Some(message) = widget.downcast_ref::<Message>() {
        return message.snapshot_fields();
    }
    // 反馈 capability 启用时才引用通知组件类型。
    #[cfg(feature = "feedback")]
    if let Some(notification) = widget.downcast_ref::<Notification>() {
        return notification.snapshot_fields();
    }
    // 反馈 capability 启用时才引用进度条组件类型。
    #[cfg(feature = "feedback")]
    if let Some(progress) = widget.downcast_ref::<ProgressBar>() {
        return progress.snapshot_fields();
    }
    // 反馈 capability 启用时才引用加载指示器组件类型。
    #[cfg(feature = "feedback")]
    if let Some(spin) = widget.downcast_ref::<Spin>() {
        return spin.snapshot_fields();
    }
    // 反馈 capability 启用时才引用文字提示组件类型。
    #[cfg(feature = "feedback")]
    if let Some(tooltip) = widget.downcast_ref::<Tooltip>() {
        return tooltip.snapshot_fields();
    }
    // 反馈 capability 启用时才引用气泡卡片组件类型。
    #[cfg(feature = "feedback")]
    if let Some(popover) = widget.downcast_ref::<Popover>() {
        return popover.snapshot_fields();
    }
    // 反馈 capability 启用时才引用气泡确认框组件类型。
    #[cfg(feature = "feedback")]
    if let Some(popconfirm) = widget.downcast_ref::<Popconfirm>() {
        return popconfirm.snapshot_fields();
    }
    // 反馈 capability 启用时才引用对话框组件类型。
    #[cfg(feature = "feedback")]
    if let Some(modal) = widget.downcast_ref::<Modal>() {
        return modal.snapshot_fields();
    }
    // 反馈 capability 启用时才引用抽屉组件类型。
    #[cfg(feature = "feedback")]
    if let Some(drawer) = widget.downcast_ref::<Drawer>() {
        return drawer.snapshot_fields();
    }
    if let Some(layout) = widget.downcast_ref::<Layout>() {
        return layout.snapshot_fields();
    }
    if let Some(header) = widget.downcast_ref::<Header>() {
        return header.snapshot_fields();
    }
    if let Some(sider) = widget.downcast_ref::<Sider>() {
        return sider.snapshot_fields();
    }
    if let Some(content) = widget.downcast_ref::<Content>() {
        return content.snapshot_fields();
    }
    if let Some(footer) = widget.downcast_ref::<Footer>() {
        return footer.snapshot_fields();
    }
    if let Some(splitter) = widget.downcast_ref::<Splitter>() {
        return splitter.snapshot_fields();
    }
    if let Some(affix) = widget.downcast_ref::<Affix>() {
        return affix.snapshot_fields();
    }
    if let Some(back_top) = widget.downcast_ref::<BackTop>() {
        return back_top.snapshot_fields();
    }
    // 导航 capability 启用时才引用面包屑组件类型。
    #[cfg(feature = "navigation")]
    if let Some(breadcrumb) = widget.downcast_ref::<Breadcrumb>() {
        return breadcrumb.snapshot_fields();
    }
    // 导航 capability 启用时才引用分页组件类型。
    #[cfg(feature = "navigation")]
    if let Some(pagination) = widget.downcast_ref::<Pagination>() {
        return pagination.snapshot_fields();
    }
    // 导航 capability 启用时才引用锚点组件类型。
    #[cfg(feature = "navigation")]
    if let Some(anchor) = widget.downcast_ref::<Anchor>() {
        return anchor.snapshot_fields();
    }
    // 导航 capability 启用时才引用菜单组件类型。
    #[cfg(feature = "navigation")]
    if let Some(menu) = widget.downcast_ref::<Menu>() {
        return menu.snapshot_fields();
    }
    // 导航 capability 启用时才引用下拉菜单组件类型。
    #[cfg(feature = "navigation")]
    if let Some(dropdown) = widget.downcast_ref::<Dropdown>() {
        return dropdown.snapshot_fields();
    }
    // 导航 capability 启用时才引用菜单栏组件类型。
    #[cfg(feature = "navigation")]
    if let Some(menu_bar) = widget.downcast_ref::<MenuBar>() {
        return menu_bar.snapshot_fields();
    }
    // 导航 capability 启用时才引用标签页组件类型。
    #[cfg(feature = "navigation")]
    if let Some(tabs) = widget.downcast_ref::<Tabs>() {
        return tabs.snapshot_fields();
    }
    // 导航 capability 启用时才引用步骤条组件类型。
    #[cfg(feature = "navigation")]
    if let Some(steps) = widget.downcast_ref::<Steps>() {
        return steps.snapshot_fields();
    }
    // 导航 capability 启用时才引用导航项组件类型。
    #[cfg(feature = "navigation")]
    if let Some(nav_item) = widget.downcast_ref::<NavItem>() {
        return nav_item.snapshot_fields();
    }
    // 导航 capability 启用时才引用 Navigation 侧栏外壳。
    #[cfg(feature = "navigation")]
    if let Some(navigation) = widget.downcast_ref::<NavigationShell>() {
        // 返回调用方元数据与整栏折叠快照。
        return navigation.snapshot_fields();
    }
    // 树组件 capability 启用时才引用展示树组件类型。
    #[cfg(feature = "tree-widgets")]
    if let Some(tree) = widget.downcast_ref::<Tree>() {
        return tree.snapshot_fields();
    }
    // 终端 capability 启用时才引用终端组件类型。
    #[cfg(feature = "terminal")]
    if let Some(terminal) = widget.downcast_ref::<Terminal>() {
        return terminal.snapshot_fields();
    }
    if let Some(list) = widget.downcast_ref::<List>() {
        return list.snapshot_fields();
    }
    if let Some(collapse) = widget.downcast_ref::<Collapse>() {
        return collapse.snapshot_fields();
    }
    if let Some(carousel) = widget.downcast_ref::<Carousel>() {
        return carousel.snapshot_fields();
    }
    if let Some(select) = widget.downcast_ref::<Select>() {
        return select.snapshot_fields();
    }
    if let Some(autocomplete) = widget.downcast_ref::<AutoComplete>() {
        return autocomplete.snapshot_fields();
    }
    // 树组件 capability 启用时才引用树选择器组件类型。
    #[cfg(feature = "tree-widgets")]
    if let Some(tree_select) = widget.downcast_ref::<TreeSelect>() {
        return tree_select.snapshot_fields();
    }
    if let Some(cascader) = widget.downcast_ref::<Cascader>() {
        return cascader.snapshot_fields();
    }
    if let Some(color_picker) = widget.downcast_ref::<ColorPicker>() {
        return color_picker.snapshot_fields();
    }
    if let Some(date_picker) = widget.downcast_ref::<DatePicker>() {
        return date_picker.snapshot_fields();
    }
    if let Some(date_range_picker) = widget.downcast_ref::<DateRangePicker>() {
        return date_range_picker.snapshot_fields();
    }
    if let Some(time_picker) = widget.downcast_ref::<TimePicker>() {
        return time_picker.snapshot_fields();
    }
    if let Some(mentions) = widget.downcast_ref::<Mentions>() {
        return mentions.snapshot_fields();
    }
    if let Some(segmented) = widget.downcast_ref::<Segmented>() {
        return segmented.snapshot_fields();
    }
    if let Some(form_item) = widget.downcast_ref::<FormItem>() {
        return form_item.snapshot_fields();
    }
    if let Some(form) = widget.downcast_ref::<Form>() {
        return form.snapshot_fields();
    }
    if let Some(descriptions) = widget.downcast_ref::<Descriptions>() {
        return descriptions.snapshot_fields();
    }
    if let Some(result) = widget.downcast_ref::<ResultView>() {
        return result.snapshot_fields();
    }
    // 表格 capability 启用时才引用表格组件类型。
    #[cfg(feature = "table")]
    // 启用后保留表格结构化快照分派。
    if let Some(table) = widget.downcast_ref::<Table>() {
        return table.snapshot_fields();
    }
    if let Some(selectable_list) = widget.downcast_ref::<SelectableList>() {
        return selectable_list.snapshot_fields();
    }
    if let Some(scroll_view) = widget.downcast_ref::<ScrollView>() {
        return scroll_view.snapshot_fields();
    }
    // 图表 capability 启用时才引用柱状图组件类型。
    #[cfg(feature = "charts")]
    // 启用后保留柱状图结构化快照分派。
    if let Some(bar_chart) = widget.downcast_ref::<BarChart>() {
        return bar_chart.snapshot_fields();
    }
    // 图表 capability 启用时才引用折线图组件类型。
    #[cfg(feature = "charts")]
    // 启用后保留折线图结构化快照分派。
    if let Some(line_chart) = widget.downcast_ref::<LineChart>() {
        return line_chart.snapshot_fields();
    }
    // 图表 capability 启用时才引用饼图组件类型。
    #[cfg(feature = "charts")]
    // 启用后保留饼图结构化快照分派。
    if let Some(pie_chart) = widget.downcast_ref::<PieChart>() {
        return pie_chart.snapshot_fields();
    }
    // 图表 capability 启用时才引用高级图表占位组件类型。
    #[cfg(feature = "charts")]
    // 启用后保留高级图表结构化快照分派。
    if let Some(chart_placeholder) = widget.downcast_ref::<ChartPlaceholder>() {
        return chart_placeholder.snapshot_fields();
    }
    // 关闭二维码 capability 时不引用已裁剪的组件类型。
    #[cfg(feature = "qrcode")]
    // 启用后保留二维码组件的结构化快照分派。
    if let Some(qrcode) = widget.downcast_ref::<QRCode>() {
        return qrcode.snapshot_fields();
    }
    // 关闭富文本 capability 时不引用已裁剪的组件类型。
    #[cfg(feature = "rich-text")]
    // 启用后保留富文本组件的结构化快照分派。
    if let Some(rich_text) = widget.downcast_ref::<RichText>() {
        return rich_text.snapshot_fields();
    }
    if let Some(theme_toggle) = widget.downcast_ref::<ThemeToggle>() {
        return theme_toggle.snapshot_fields();
    }
    if let Some(transfer) = widget.downcast_ref::<Transfer>() {
        return transfer.snapshot_fields();
    }
    if let Some(upload) = widget.downcast_ref::<Upload>() {
        return upload.snapshot_fields();
    }
    if let Some(watermark) = widget.downcast_ref::<Watermark>() {
        return watermark.snapshot_fields();
    }
    if let Some(container) = widget.downcast_ref::<Container>() {
        return container.snapshot_fields();
    }
    if let Some(grid) = widget.downcast_ref::<Grid>() {
        return grid.snapshot_fields();
    }
    SnapshotFields::Unknown
}
