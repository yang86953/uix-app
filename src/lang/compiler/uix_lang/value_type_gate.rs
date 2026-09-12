// 引入文档、record 与值类型 AST。
use super::{Document, WidgetStateInitial, WidgetValueType};
// 引入带位置与修复建议的解析诊断。
use super::Diagnostic;

// 返回一个 lowering 值类型变体在 schema 登记中挂接的 capability。
pub(crate) fn value_type_capability(value_type: &WidgetValueType) -> Option<&'static str> {
    super::super::projection_schema::UI_PROJECTION_SCHEMA
        .value_types()
        .iter()
        // 通过登记名反向映射，保证 capability 事实只存在于 schema 一处。
        .find(|spec| {
            super::widget_parser::registered_value_type(spec.name).as_ref() == Some(value_type)
        })
        .and_then(|spec| spec.capability)
}

// 校验文档内全部 state 注解与 record 字段的组件专属类型 capability。
// 门禁事实由 UiProjectionSchema 登记；enabled 由调用方注入以便统一能力开关。
pub(crate) fn validate_value_type_capabilities(
    document: &Document,
    enabled: impl Fn(&str) -> bool,
) -> Result<(), Diagnostic> {
    // 逐一声明校验。
    for declaration in &document.declarations {
        // 按声明类别定位类型注解。
        match declaration {
            // 组件 state 的显式类型注解是唯一携带变体的位置。
            super::Declaration::Widget(widget) => {
                // 遍历组件私有 state。
                for state in &widget.states {
                    // 只有类型化初始值带登记类型。
                    if let WidgetStateInitial::TypedExpression(value_type, _) = &state.initial {
                        // 校验单个类型的 capability 门禁。
                        ensure_value_type_enabled(
                            value_type,
                            "state",
                            &state.name,
                            state.span,
                            &enabled,
                        )?;
                    }
                }
            }
            // record 字段共享同一登记面。
            super::Declaration::Record(record) => {
                // 遍历 record 字段。
                for field in &record.fields {
                    // 校验字段类型的 capability 门禁。
                    ensure_value_type_enabled(
                        &field.kind,
                        "record 字段",
                        &field.name,
                        field.span,
                        &enabled,
                    )?;
                }
            }
            // 其余声明不含值类型注解。
            _ => {}
        }
    }
    // 全部类型已通过门禁。
    Ok(())
}

// 对单个值类型执行 capability 门禁并在未启用时返回定向诊断。
fn ensure_value_type_enabled(
    value_type: &WidgetValueType,
    subject_kind: &str,
    subject_name: &str,
    span: super::SourceSpan,
    enabled: &impl Fn(&str) -> bool,
) -> Result<(), Diagnostic> {
    // 查询登记的 capability 并核对启用状态。
    if let Some(capability) =
        value_type_capability(value_type).filter(|capability| !enabled(capability))
    {
        // 返回定向能力门禁诊断。
        return Err(Diagnostic::new(
            span,
            format!(
                "{subject_kind} {subject_name} 的类型 {} 需要 capability {capability}",
                schema_name_of(value_type)
            ),
            format!("在 uix 依赖上启用 Cargo feature {capability}，或改用核心通用类型"),
        ));
    }
    // 类型已启用或无需门禁。
    Ok(())
}

// 返回 lowering 变体对应的语言面书写名，用于诊断文本。
fn schema_name_of(value_type: &WidgetValueType) -> &str {
    match value_type {
        WidgetValueType::Library(name) => name,
        WidgetValueType::String => "String",
        WidgetValueType::Number => "number",
        WidgetValueType::Bool => "bool",
        WidgetValueType::U32 => "u32",
        WidgetValueType::USize => "usize",
        WidgetValueType::F32 => "f32",
        WidgetValueType::I32 => "i32",
        WidgetValueType::Color => "Color",
        WidgetValueType::Point => "Point",
        WidgetValueType::HashSetOfString => "HashSet<String>",
        WidgetValueType::VecOfString => "Vec<String>",
        WidgetValueType::VecOfNumber => "Vec<number>",
        WidgetValueType::OptionalString => "Option<String>",
        // record 引用不参与 capability 门禁，诊断不会到达此分支。
        WidgetValueType::Record(_) | WidgetValueType::VecOfRecord(_) => "",
    }
}

// 集中验证组件专属类型的能力门禁与 schema 反向映射契约。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/value_type_gate_tests.rs"]
mod tests;

