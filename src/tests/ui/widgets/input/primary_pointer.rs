use crate::tests::common::*;
use crate::ui::widgets::{
    AutoComplete, Cascader, CascaderOption, Checkbox, ColorPicker, DatePicker, DateRangePicker,
    Input, InputNumber, Mentions, Radio, Rate, Segmented, Select, Slider, Switch, TimePicker,
    TreeSelect,
};

#[test]
fn input_controls_ignore_secondary_pointer_activation() {
    let mut controls: Vec<(&str, Box<dyn EventHandler>)> = vec![
        ("autocomplete", Box::new(AutoComplete::new())),
        (
            "cascader",
            Box::new(Cascader::new(Vec::<CascaderOption>::new(), "area")),
        ),
        ("checkbox", Box::new(Checkbox::new("agree"))),
        ("color picker", Box::new(ColorPicker::new())),
        ("date picker", Box::new(DatePicker::new())),
        ("date range picker", Box::new(DateRangePicker::new())),
        ("input", Box::new(Input::new("name"))),
        ("input number", Box::new(InputNumber::new())),
        ("mentions", Box::new(Mentions::new("person"))),
        ("radio", Box::new(Radio::new().options(vec!["A", "B"]))),
        ("rate", Box::new(Rate::new())),
        ("segmented", Box::new(Segmented::new(["A", "B"]))),
        ("select", Box::new(Select::new().options(vec!["A", "B"]))),
        ("slider", Box::new(Slider::new(0.0..=100.0))),
        ("switch", Box::new(Switch::new())),
        ("time picker", Box::new(TimePicker::new())),
        ("tree select", Box::new(TreeSelect::new())),
    ];
    let event = SystemEvent::PointerDown {
        pos: Point::new(8.0, 8.0),
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    };

    for (name, control) in &mut controls {
        assert_eq!(
            control.on_event(&event),
            EventResult::NotHandled,
            "{name} must reserve secondary pointer for context menus"
        );
    }
}

#[test]
fn drag_capable_inputs_ignore_secondary_pointer_release() {
    let event = SystemEvent::PointerUp {
        pos: Point::new(8.0, 8.0),
        button: MouseButton::Right,
        mods: KeyMod::NONE,
    };

    assert_eq!(
        Slider::new(0.0..=100.0).on_event(&event),
        EventResult::NotHandled
    );
    assert_eq!(Input::new("name").on_event(&event), EventResult::NotHandled);
}
