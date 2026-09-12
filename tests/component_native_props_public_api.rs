//! 原生属性的单一声明在不编入 UI/解析器的消费者中仍可检查和解码。
#![cfg(feature = "uix-components")]
use std::{collections::BTreeMap, sync::Arc};
use uix_app::lang::runtime::{
    Effect, ErrorKind, Type as DataType, Value as DataValue, components::*,
};

uix_app::native_props! {pub struct EditorProps {
    title:String => "displayName",
    tags:Vec<String>=vec!["default".into()],
    enabled:OptionalProp<bool>=OptionalProp(None),
    width:OptionalProp<f64>=OptionalProp(None),
    on_save:NativeCallback<(String, i64), bool> => "onSave"=NativeCallback::default(),
}}

fn data(value: DataValue) -> ProjectedValue {
    ProjectedValue::Data(Arc::new(value))
}

#[test]
fn typed_defaults_keep_presence_and_reject_misspelled_or_wrong_properties() {
    let events = EventDispatcher::new(|_, _| panic!("decoding must not invoke a callback"));
    let mut values =
        BTreeMap::from([("displayName".into(), data(DataValue::String("编辑".into())))]);
    let props = EditorProps::read(&values, &events).unwrap();
    assert_eq!(props.title, "编辑");
    assert_eq!(props.tags, ["default"]);
    assert!(props.enabled.0.is_none());
    assert_eq!(
        props.on_save.call(("x".into(), 1)).unwrap_err().kind,
        ErrorKind::UnknownEvent
    );
    values.insert("enabled".into(), data(DataValue::Bool(false)));
    values.insert("width".into(), data(DataValue::Float(0.0)));
    values.insert("tags".into(), data(DataValue::Array(vec![])));
    let props = EditorProps::read(&values, &events).unwrap();
    assert_eq!(props.enabled.0, Some(false));
    assert_eq!(props.width.0, Some(0.0));
    assert!(props.tags.is_empty());
    values.insert(
        "tags".into(),
        data(DataValue::Array(vec![DataValue::Int(1)])),
    );
    assert_eq!(
        EditorProps::read(&values, &events).err().unwrap().kind,
        ErrorKind::Type
    );
    values.remove("tags");
    values.insert(
        "display_name".into(),
        data(DataValue::String("typo".into())),
    );
    assert_eq!(
        EditorProps::read(&values, &events).err().unwrap().kind,
        ErrorKind::Argument
    );
    values.remove("display_name");
    values.remove("displayName");
    assert_eq!(
        EditorProps::read(&values, &events).err().unwrap().kind,
        ErrorKind::Argument
    );
    let signature = EditorProps::signature();
    assert_eq!(signature.required, ["displayName".into()].into());
    let (_, Type::Function(callback)) = signature
        .parameters
        .iter()
        .find(|(name, _)| name == "onSave")
        .unwrap()
    else {
        panic!("callback signature")
    };
    assert_eq!(
        callback.parameters,
        [Type::Data(DataType::String), Type::Data(DataType::Int)]
    );
    assert_eq!(*callback.returns, Type::Data(DataType::Bool));
    assert_eq!(callback.effect, Effect::Command);
}
