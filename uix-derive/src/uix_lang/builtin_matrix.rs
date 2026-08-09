// 引入结构化元素与诊断。
use super::{Diagnostic, Element};

// 保存一个规划中内置组件的文档类别。
struct PlannedBuiltin {
    // 保存 PascalCase 标签名。
    name: &'static str,
    // 保存用户可核对的文档类别。
    category: &'static str,
}

// 登记六类内置组件文档与 App 入口中的规划中标签。
const PLANNED_BUILTINS: &[PlannedBuiltin] = &[
    // 标签语法中的应用入口仍需要应用构建上下文。
    PlannedBuiltin {
        name: "App",
        category: "标签语法 / App 应用入口",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "Checkbox",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "Switch",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "Radio",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "Segmented",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "Select",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "Cascader",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "TreeSelect",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "AutoComplete",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "Mentions",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "DatePicker",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "DateRangePicker",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "TimePicker",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "ColorPicker",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "Form",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "FormInputItem",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "FormSelectItem",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "FormCheckboxItem",
        category: "内置组件 / 输入组件",
    },
    // 输入组件规划项。
    PlannedBuiltin {
        name: "Upload",
        category: "内置组件 / 输入组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Avatar",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Badge",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Tag",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Card",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Descriptions",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Skeleton",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Empty",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "ResultView",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "List",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "SelectableList",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Tree",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Table",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "DataTable",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Timeline",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Collapse",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Calendar",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Carousel",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Image",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "ImageGroup",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "QRCode",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "Watermark",
        category: "内置组件 / 展示组件",
    },
    // 展示组件规划项。
    PlannedBuiltin {
        name: "RichText",
        category: "内置组件 / 展示组件",
    },
    // 导航组件规划项。
    PlannedBuiltin {
        name: "Menu",
        category: "内置组件 / 导航组件",
    },
    // 导航组件规划项。
    PlannedBuiltin {
        name: "Navigation",
        category: "内置组件 / 导航组件",
    },
    // 导航组件规划项。
    PlannedBuiltin {
        name: "Tabs",
        category: "内置组件 / 导航组件",
    },
    // 导航组件规划项。
    PlannedBuiltin {
        name: "Breadcrumb",
        category: "内置组件 / 导航组件",
    },
    // 导航组件规划项。
    PlannedBuiltin {
        name: "Steps",
        category: "内置组件 / 导航组件",
    },
    // 导航组件规划项。
    PlannedBuiltin {
        name: "Anchor",
        category: "内置组件 / 导航组件",
    },
    // 导航组件规划项。
    PlannedBuiltin {
        name: "Dropdown",
        category: "内置组件 / 导航组件",
    },
    // 导航组件规划项。
    PlannedBuiltin {
        name: "Pagination",
        category: "内置组件 / 导航组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "Message",
        category: "内置组件 / 反馈组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "Notification",
        category: "内置组件 / 反馈组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "Alert",
        category: "内置组件 / 反馈组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "Modal",
        category: "内置组件 / 反馈组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "Drawer",
        category: "内置组件 / 反馈组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "Popover",
        category: "内置组件 / 反馈组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "Popconfirm",
        category: "内置组件 / 反馈组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "Tooltip",
        category: "内置组件 / 反馈组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "FocusTrap",
        category: "内置组件 / 反馈组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "ProgressBar",
        category: "内置组件 / 反馈组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "Spin",
        category: "内置组件 / 反馈组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "FloatButtonGroup",
        category: "内置组件 / 反馈组件",
    },
    // 反馈组件规划项。
    PlannedBuiltin {
        name: "FloatButtonBackTop",
        category: "内置组件 / 反馈组件",
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
        "当前使用 Text、Label、Button、ButtonGroup、FloatButton、Icon、Divider、Space、Typography、ThemeToggle、WindowControl、Container、Row、Column、Grid、ScrollView、VirtualScroll、Input、InputNumber、InputGroup、Slider、RangeSlider 或 Rate；Col 仅作为 Row/Grid 的直接子项，或先完成该组件的属性映射 Gate",
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
        // 仍有规划中标签的四类内置组件必须全部出现。
        for category in [
            "内置组件 / 输入组件",
            "内置组件 / 展示组件",
            "内置组件 / 导航组件",
            "内置组件 / 反馈组件",
        ] {
            // 当前类别必须存在至少一个登记项。
            assert!(categories.contains(category));
        }
    }
}
