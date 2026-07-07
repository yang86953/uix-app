use crate::ui::widgets::{
    Button, Cascader, ColorPicker, Container, Grid, Input, Label, ScrollView, Select, Space,
    Tooltip, TreeSelect,
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
    patch_as!(Input);
    patch_as!(Label);
    patch_as!(ScrollView);
    patch_as!(Select);
    patch_as!(Space);
    patch_as!(Tooltip);
    patch_as!(TreeSelect);
    patch_as!(Cascader);
    patch_as!(ColorPicker);

    Err(next)
}
