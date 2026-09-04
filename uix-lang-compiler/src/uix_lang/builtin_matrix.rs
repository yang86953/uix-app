// 引入 Compiler System 唯一拥有的 UI 投影 schema。
use crate::projection_schema::{RegistrationStatus, UI_PROJECTION_SCHEMA};

// 引入结构化元素与诊断。
use super::{Diagnostic, Element};

// 返回当前已经进入 AOT 生成矩阵的完整标签清单。
pub(crate) fn supported_builtin_hint() -> String {
    UI_PROJECTION_SCHEMA.supported_component_hint()
}

// 为已登记但规划中的内置组件生成类别化诊断。
pub(crate) fn planned_builtin_diagnostic(element: &Element) -> Option<Diagnostic> {
    let entry = UI_PROJECTION_SCHEMA.component(&element.name)?;
    if entry.status != RegistrationStatus::Planned {
        return None;
    }
    Some(Diagnostic::new(
        element.span,
        format!(
            "内置组件 <{}> 已登记于{}，但 uix-lang 生成映射仍为规划中",
            element.name,
            entry.category.label()
        ),
        format!(
            "当前使用 {}；Col 仅作为 Row/Grid 的直接子项，或先完成该组件的属性映射 Gate",
            supported_builtin_hint()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use crate::projection_schema::{RegistrationStatus, UI_PROJECTION_SCHEMA};

    use super::supported_builtin_hint;

    #[test]
    fn app_and_supported_hint_share_projection_schema() {
        let app = UI_PROJECTION_SCHEMA.component("App").expect("App 应已登记");
        assert_eq!(app.status, RegistrationStatus::Available);
        let names = supported_builtin_hint().split('、').count();
        assert_eq!(names, 112);
    }
}
