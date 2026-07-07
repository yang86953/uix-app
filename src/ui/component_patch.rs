use crate::ui::widgets::{
    Anchor, AutoComplete, Avatar, Badge, Button, Cascader, Checkbox, ColorPicker, Container,
    DatePicker, Divider, Drawer, Dropdown, Grid, Icon, Input, InputNumber, Label, Mentions, Menu,
    Modal, Pagination, Popconfirm, Popover, Radio, ScrollView, Select, Slider, Space, Steps,
    Switch, Tabs, TimePicker, Tooltip, TreeSelect, Typography,
};
use crate::ui::WidgetComponent;

pub(crate) fn patch_builtin_widget(
    current: &mut dyn WidgetComponent,
    next: Box<dyn WidgetComponent>,
) -> Result<bool, Box<dyn WidgetComponent>> {
    macro_rules! patch_as {
        ($ty:ty) => {
            if current.as_any().is::<$ty>() && next.as_any().is::<$ty>() {
                let next = next
                    .into_any()
                    .downcast::<$ty>()
                    .expect("type checked before downcast");
                current
                    .as_any_mut()
                    .downcast_mut::<$ty>()
                    .expect("type checked before downcast")
                    .sync_from(*next);
                return Ok(true);
            }
        };
    }

    patch_as!(Button);
    patch_as!(Container);
    patch_as!(Grid);
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
    patch_as!(Anchor);
    patch_as!(Steps);
    patch_as!(TreeSelect);
    patch_as!(Cascader);
    patch_as!(ColorPicker);
    patch_as!(AutoComplete);
    patch_as!(DatePicker);
    patch_as!(TimePicker);
    patch_as!(Mentions);

    Err(next)
}
