// 引入结构化元素与诊断。
use super::{Diagnostic, Element};

// 保存一个规划中内置组件的文档类别。
struct PlannedBuiltin {
    // 保存 PascalCase 标签名。
    name: &'static str,
    // 保存用户可核对的文档类别。
    category: &'static str,
}

// 登记仍未映射的内置组件文档与 App 入口标签。
const PLANNED_BUILTINS: &[PlannedBuiltin] = &[
    // 标签语法中的应用入口仍需要应用构建上下文。
    PlannedBuiltin {
        name: "App",
        category: "标签语法 / App 应用入口",
    },
];

// 为已登记但规划中的内置组件生成类别化诊断。
pub(crate) fn planned_builtin_diagnostic(element: &Element) -> Option<Diagnostic> {
    // 查找与元素名称相同的矩阵条目。
    let entry = PLANNED_BUILTINS
        .iter()
        .find(|entry| entry.name == element.name)?;
    // 返回不伪装支持的编译期诊断。
    Some(Diagnostic::new(
        // 指向完整元素。
        element.span,
        // 同时说明名称、类别与状态。
        format!(
            "内置组件 <{}> 已登记于{}，但 uix-lang 生成映射仍为规划中",
            element.name, entry.category
        ),
        // 给出当前真实支持集合。
        "当前使用 Text、Label、Button、ButtonGroup、FloatButton、FloatButtonGroup、FloatButtonBackTop、Icon、Divider、Space、Typography、ThemeToggle、WindowControl、WindowDragRegion、Container、Row、Column、Grid、ScrollView、VirtualScroll、Splitter、Affix、BackTop、Layout、Sider、Header、Content、Footer、Input、InputNumber、InputGroup、Slider、RangeSlider、Rate、Checkbox、Switch、Radio、Segmented、Select、Cascader、TreeSelect、AutoComplete、Mentions、DatePicker、DateRangePicker、TimePicker、ColorPicker、Form、FormInputItem、FormSelectItem、FormCheckboxItem、FormRadioItem、FormSwitchItem、FormSliderItem、Upload、Avatar、Image、ImageGroup、List、SelectableList、Collapse、Skeleton、Empty、ResultView、Tag、Card、Descriptions、Timeline、Calendar、Carousel、Tree、Table、Menu、Dropdown、Steps、Pagination、Breadcrumb、Anchor、Tabs、QRCode、Watermark、RichText、Alert、ProgressBar、Popconfirm、Modal、Drawer、Tooltip、Popover、FocusTrap 或 Spin；Col 仅作为 Row/Grid 的直接子项，或先完成该组件的属性映射 Gate",
    ))
}

// 测试内置组件登记表的确定性约束。
#[cfg(test)]
mod tests {
    // 引入登记表与名称集合。
    use super::PLANNED_BUILTINS;
    // 引入有序去重集合。
    use std::collections::BTreeSet;

    // 验证文档登记名唯一且覆盖六个类别。
    #[test]
    fn planned_builtin_names_are_unique_and_cover_all_categories() {
        // 收集所有登记名称。
        let names = PLANNED_BUILTINS
            .iter()
            .map(|entry| entry.name)
            .collect::<BTreeSet<_>>();
        // 名称集合大小必须与登记项一致。
        assert_eq!(names.len(), PLANNED_BUILTINS.len());
        // 收集所有类别。
        let categories = PLANNED_BUILTINS
            .iter()
            .map(|entry| entry.category)
            .collect::<BTreeSet<_>>();
        // App 是当前唯一仍规划中的内置入口类别。
        assert!(categories.contains("标签语法 / App 应用入口"));
    }
}
