use std::any::Any;

use crate::ui::form::{Form, FormItem};
use crate::ui::widgets::*;

use super::SnapshotFields;

pub trait SnapshotSource {
    fn snapshot_fields(&self) -> SnapshotFields;
}

pub fn snapshot_fields_from_any(component: &dyn Any) -> SnapshotFields {
    if let Some(button) = component.downcast_ref::<Button>() {
        return button.snapshot_fields();
    }
    if let Some(label) = component.downcast_ref::<Label>() {
        return label.snapshot_fields();
    }
    if let Some(input) = component.downcast_ref::<Input>() {
        return input.snapshot_fields();
    }
    if let Some(space) = component.downcast_ref::<Space>() {
        return space.snapshot_fields();
    }
    if let Some(divider) = component.downcast_ref::<Divider>() {
        return divider.snapshot_fields();
    }
    if let Some(icon) = component.downcast_ref::<Icon>() {
        return icon.snapshot_fields();
    }
    if let Some(typography) = component.downcast_ref::<Typography>() {
        return typography.snapshot_fields();
    }
    if let Some(checkbox) = component.downcast_ref::<Checkbox>() {
        return checkbox.snapshot_fields();
    }
    if let Some(radio) = component.downcast_ref::<Radio>() {
        return radio.snapshot_fields();
    }
    if let Some(switch) = component.downcast_ref::<Switch>() {
        return switch.snapshot_fields();
    }
    if let Some(slider) = component.downcast_ref::<Slider>() {
        return slider.snapshot_fields();
    }
    if let Some(slider) = component.downcast_ref::<RangeSlider>() {
        return slider.snapshot_fields();
    }
    if let Some(rate) = component.downcast_ref::<Rate>() {
        return rate.snapshot_fields();
    }
    if let Some(input_number) = component.downcast_ref::<InputNumber>() {
        return input_number.snapshot_fields();
    }
    if let Some(avatar) = component.downcast_ref::<Avatar>() {
        return avatar.snapshot_fields();
    }
    if let Some(badge) = component.downcast_ref::<Badge>() {
        return badge.snapshot_fields();
    }
    if let Some(card) = component.downcast_ref::<Card>() {
        return card.snapshot_fields();
    }
    if let Some(empty) = component.downcast_ref::<Empty>() {
        return empty.snapshot_fields();
    }
    if let Some(image) = component.downcast_ref::<Image>() {
        return image.snapshot_fields();
    }
    if let Some(image_group) = component.downcast_ref::<ImageGroup>() {
        return image_group.snapshot_fields();
    }
    if let Some(tag) = component.downcast_ref::<Tag>() {
        return tag.snapshot_fields();
    }
    if let Some(timeline) = component.downcast_ref::<Timeline>() {
        return timeline.snapshot_fields();
    }
    if let Some(calendar) = component.downcast_ref::<Calendar>() {
        return calendar.snapshot_fields();
    }
    if let Some(skeleton) = component.downcast_ref::<Skeleton>() {
        return skeleton.snapshot_fields();
    }
    if let Some(float_button) = component.downcast_ref::<FloatButton>() {
        return float_button.snapshot_fields();
    }
    if let Some(float_button_group) = component.downcast_ref::<FloatButtonGroup>() {
        return float_button_group.snapshot_fields();
    }
    if let Some(alert) = component.downcast_ref::<Alert>() {
        return alert.snapshot_fields();
    }
    if let Some(message) = component.downcast_ref::<Message>() {
        return message.snapshot_fields();
    }
    if let Some(notification) = component.downcast_ref::<Notification>() {
        return notification.snapshot_fields();
    }
    if let Some(progress) = component.downcast_ref::<ProgressBar>() {
        return progress.snapshot_fields();
    }
    if let Some(spin) = component.downcast_ref::<Spin>() {
        return spin.snapshot_fields();
    }
    if let Some(tooltip) = component.downcast_ref::<Tooltip>() {
        return tooltip.snapshot_fields();
    }
    if let Some(popover) = component.downcast_ref::<Popover>() {
        return popover.snapshot_fields();
    }
    if let Some(popconfirm) = component.downcast_ref::<Popconfirm>() {
        return popconfirm.snapshot_fields();
    }
    if let Some(modal) = component.downcast_ref::<Modal>() {
        return modal.snapshot_fields();
    }
    if let Some(drawer) = component.downcast_ref::<Drawer>() {
        return drawer.snapshot_fields();
    }
    if let Some(layout) = component.downcast_ref::<Layout>() {
        return layout.snapshot_fields();
    }
    if let Some(header) = component.downcast_ref::<Header>() {
        return header.snapshot_fields();
    }
    if let Some(sider) = component.downcast_ref::<Sider>() {
        return sider.snapshot_fields();
    }
    if let Some(content) = component.downcast_ref::<Content>() {
        return content.snapshot_fields();
    }
    if let Some(footer) = component.downcast_ref::<Footer>() {
        return footer.snapshot_fields();
    }
    if let Some(splitter) = component.downcast_ref::<Splitter>() {
        return splitter.snapshot_fields();
    }
    if let Some(affix) = component.downcast_ref::<Affix>() {
        return affix.snapshot_fields();
    }
    if let Some(back_top) = component.downcast_ref::<BackTop>() {
        return back_top.snapshot_fields();
    }
    if let Some(breadcrumb) = component.downcast_ref::<Breadcrumb>() {
        return breadcrumb.snapshot_fields();
    }
    if let Some(pagination) = component.downcast_ref::<Pagination>() {
        return pagination.snapshot_fields();
    }
    if let Some(anchor) = component.downcast_ref::<Anchor>() {
        return anchor.snapshot_fields();
    }
    if let Some(menu) = component.downcast_ref::<Menu>() {
        return menu.snapshot_fields();
    }
    if let Some(dropdown) = component.downcast_ref::<Dropdown>() {
        return dropdown.snapshot_fields();
    }
    if let Some(tabs) = component.downcast_ref::<Tabs>() {
        return tabs.snapshot_fields();
    }
    if let Some(steps) = component.downcast_ref::<Steps>() {
        return steps.snapshot_fields();
    }
    if let Some(nav_item) = component.downcast_ref::<NavItem>() {
        return nav_item.snapshot_fields();
    }
    if let Some(tree) = component.downcast_ref::<Tree>() {
        return tree.snapshot_fields();
    }
    if let Some(list) = component.downcast_ref::<List>() {
        return list.snapshot_fields();
    }
    if let Some(collapse) = component.downcast_ref::<Collapse>() {
        return collapse.snapshot_fields();
    }
    if let Some(carousel) = component.downcast_ref::<Carousel>() {
        return carousel.snapshot_fields();
    }
    if let Some(select) = component.downcast_ref::<Select>() {
        return select.snapshot_fields();
    }
    if let Some(autocomplete) = component.downcast_ref::<AutoComplete>() {
        return autocomplete.snapshot_fields();
    }
    if let Some(tree_select) = component.downcast_ref::<TreeSelect>() {
        return tree_select.snapshot_fields();
    }
    if let Some(cascader) = component.downcast_ref::<Cascader>() {
        return cascader.snapshot_fields();
    }
    if let Some(color_picker) = component.downcast_ref::<ColorPicker>() {
        return color_picker.snapshot_fields();
    }
    if let Some(date_picker) = component.downcast_ref::<DatePicker>() {
        return date_picker.snapshot_fields();
    }
    if let Some(date_range_picker) = component.downcast_ref::<DateRangePicker>() {
        return date_range_picker.snapshot_fields();
    }
    if let Some(time_picker) = component.downcast_ref::<TimePicker>() {
        return time_picker.snapshot_fields();
    }
    if let Some(mentions) = component.downcast_ref::<Mentions>() {
        return mentions.snapshot_fields();
    }
    if let Some(segmented) = component.downcast_ref::<Segmented>() {
        return segmented.snapshot_fields();
    }
    if let Some(form_item) = component.downcast_ref::<FormItem>() {
        return form_item.snapshot_fields();
    }
    if let Some(form) = component.downcast_ref::<Form>() {
        return form.snapshot_fields();
    }
    if let Some(descriptions) = component.downcast_ref::<Descriptions>() {
        return descriptions.snapshot_fields();
    }
    if let Some(result) = component.downcast_ref::<ResultView>() {
        return result.snapshot_fields();
    }
    if let Some(table) = component.downcast_ref::<Table>() {
        return table.snapshot_fields();
    }
    if let Some(selectable_list) = component.downcast_ref::<SelectableList>() {
        return selectable_list.snapshot_fields();
    }
    if let Some(scroll_view) = component.downcast_ref::<ScrollView>() {
        return scroll_view.snapshot_fields();
    }
    if let Some(bar_chart) = component.downcast_ref::<BarChart>() {
        return bar_chart.snapshot_fields();
    }
    if let Some(line_chart) = component.downcast_ref::<LineChart>() {
        return line_chart.snapshot_fields();
    }
    if let Some(pie_chart) = component.downcast_ref::<PieChart>() {
        return pie_chart.snapshot_fields();
    }
    if let Some(qrcode) = component.downcast_ref::<QRCode>() {
        return qrcode.snapshot_fields();
    }
    if let Some(rich_text) = component.downcast_ref::<RichText>() {
        return rich_text.snapshot_fields();
    }
    if let Some(theme_toggle) = component.downcast_ref::<ThemeToggle>() {
        return theme_toggle.snapshot_fields();
    }
    if let Some(transfer) = component.downcast_ref::<Transfer>() {
        return transfer.snapshot_fields();
    }
    if let Some(upload) = component.downcast_ref::<Upload>() {
        return upload.snapshot_fields();
    }
    if let Some(watermark) = component.downcast_ref::<Watermark>() {
        return watermark.snapshot_fields();
    }
    if let Some(container) = component.downcast_ref::<Container>() {
        return container.snapshot_fields();
    }
    if let Some(grid) = component.downcast_ref::<Grid>() {
        return grid.snapshot_fields();
    }
    SnapshotFields::Unknown
}
