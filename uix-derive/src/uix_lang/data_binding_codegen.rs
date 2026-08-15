// 引入过程宏标识符与令牌流。
use proc_macro2::{Ident, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入表达式语法树节点。
use super::{Expression, ExpressionKind};

// 保存一个数据类型构造的完整生成规格。
pub(crate) struct DataConstructorSpec {
    // 保存公开 API 类型路径。
    pub(crate) path: TokenStream,
    // 保存公开构造函数名（None 表示 new）。
    pub(crate) method: Option<Ident>,
    // 标记整数参数是否规范化为小数形状。
    pub(crate) normalize_numbers: bool,
}

// 登记当前公开契约允许在语言面直接构造的结构化数据类型名。
const DATA_TYPE_NAMES: &[&str] = &[
    // 下拉选择器选项。
    "SelectOption",
    // 级联选择器选项。
    "CascaderOption",
    // 树与树选择器节点。
    "TreeNode",
    // 时间轴事件项。
    "TimelineItem",
    // 可选中列表条目。
    "SelectableItem",
    // 折叠面板条目。
    "CollapsePanel",
    // 步骤条步骤。
    "Step",
    // 描述列表条目。
    "DescriptionsItem",
    // 面包屑条目。
    "BreadcrumbItem",
    // 锚点导航目标。
    "AnchorItem",
    // 标签页元数据。
    "Tab",
    // 菜单树条目。
    "MenuItem",
    // keyed 下拉菜单选项。
    "DropdownItem",
    // 柱状图分类数据。
    "BarData",
    // 折线图分类数据。
    "LineData",
    // 饼图分类数据。
    "PieData",
    // 散点图坐标数据。
    "ScatterData",
    // 漏斗图阶段数据。
    "FunnelData",
    // 矩形树图层级节点。
    "TreemapNode",
    // 仪表盘色带区间。
    "GaugeRange",
    // 热力图矩阵单元。
    "HeatmapCell",
    // 瀑布图变化项。
    "WaterfallData",
    // 日期语义类型。
    "Date",
    // 时间语义类型。
    "Time",
    // 颜色语义类型。
    "Color",
    // 坐标语义类型。
    "Point",
];

// 查找语言面类型名对应的公开构造规格。
pub(crate) fn data_constructor_spec(name: &str) -> Option<DataConstructorSpec> {
    // 按语言面类型名生成公开构造规格。
    let (path, method, normalize_numbers) = match name {
        // 映射下拉选项。
        "SelectOption" => (quote! { ::uix::prelude::SelectOption }, None, false),
        // 映射级联选项。
        "CascaderOption" => (quote! { ::uix::prelude::CascaderOption }, None, false),
        // 映射树节点。
        "TreeNode" => (quote! { ::uix::prelude::TreeNode }, None, false),
        // 映射时间轴事件。
        "TimelineItem" => (quote! { ::uix::prelude::TimelineItem }, None, false),
        // 映射可选中列表条目。
        "SelectableItem" => (quote! { ::uix::prelude::SelectableItem }, None, false),
        // 映射折叠面板条目。
        "CollapsePanel" => (quote! { ::uix::prelude::CollapsePanel }, None, false),
        // 映射步骤。
        "Step" => (quote! { ::uix::prelude::Step }, None, false),
        // 映射描述条目。
        "DescriptionsItem" => (quote! { ::uix::prelude::DescriptionsItem }, None, false),
        // 映射面包屑条目。
        "BreadcrumbItem" => (quote! { ::uix::prelude::BreadcrumbItem }, None, false),
        // 映射锚点目标。
        "AnchorItem" => (quote! { ::uix::prelude::AnchorItem }, None, false),
        // 映射标签页元数据。
        "Tab" => (quote! { ::uix::prelude::Tab }, None, false),
        // 菜单项使用显式 label/key 构造入口。
        "MenuItem" => (
            quote! { ::uix::prelude::MenuItem },
            Some(Ident::new("from_text", proc_macro2::Span::mixed_site())),
            false,
        ),
        // 下拉选项使用显式 label/key 构造入口。
        "DropdownItem" => (
            quote! { ::uix::prelude::DropdownItem },
            Some(Ident::new("from_text", proc_macro2::Span::mixed_site())),
            false,
        ),
        // 柱状图数据构造器接收 f32 数值。
        "BarData" => (quote! { ::uix::prelude::BarData }, None, true),
        // 折线图数据构造器接收 f32 数值。
        "LineData" => (quote! { ::uix::prelude::LineData }, None, true),
        // 饼图数据构造器接收 f32 数值与颜色。
        "PieData" => (quote! { ::uix::prelude::PieData }, None, true),
        // 散点图数据构造器接收两个 f32 坐标。
        "ScatterData" => (quote! { ::uix::prelude::ScatterData }, None, true),
        // 漏斗图数据构造器接收 f32 阶段值。
        "FunnelData" => (quote! { ::uix::prelude::FunnelData }, None, true),
        // 矩形树图节点构造器接收 f32 权重并允许 children 链。
        "TreemapNode" => (quote! { ::uix::prelude::TreemapNode }, None, true),
        // 仪表盘色带构造器接收两个 f32 边界与颜色。
        "GaugeRange" => (quote! { ::uix::prelude::GaugeRange }, None, true),
        // 热力图单元由表达式生成层单独规范化第三个 f32 参数。
        "HeatmapCell" => (quote! { ::uix::prelude::HeatmapCell }, None, false),
        // 瀑布图数据由表达式生成层单独映射数值与类型参数。
        "WaterfallData" => (quote! { ::uix::prelude::WaterfallData }, None, false),
        // 日期构造 year/month/day。
        "Date" => (quote! { ::uix::prelude::Date }, None, false),
        // 时间构造 hour/minute。
        "Time" => (quote! { ::uix::prelude::Time }, None, false),
        // 颜色使用 hex 构造函数。
        "Color" => (
            quote! { ::uix::prelude::Color },
            Some(Ident::new("hex", proc_macro2::Span::mixed_site())),
            false,
        ),
        // 坐标参数规范化为 f32 形状。
        "Point" => (quote! { ::uix::prelude::Point }, None, true),
        // 其他类型名不在登记表。
        _ => return None,
    };
    // 返回完整构造规格。
    Some(DataConstructorSpec {
        // 保存公开路径。
        path,
        // 保存构造函数名。
        method,
        // 保存数字规范化策略。
        normalize_numbers,
    })
}

// 判断语言面类型名是否属于已登记数据类型。
pub(crate) fn is_registered_data_type(name: &str) -> bool {
    // 按精确名称查找登记表。
    DATA_TYPE_NAMES.contains(&name)
}

// 把表达式树中的整数数字字面量规范化为小数形状。
pub(crate) fn normalize_number_literals(expression: &mut Expression) {
    // 递归访问当前节点。
    match &mut expression.kind {
        // 整数形状补充小数点。
        ExpressionKind::Number(source) => {
            // 只改写没有小数点或指数的整数形态。
            if !source.contains('.') && !source.contains('e') && !source.contains('E') {
                // 追加零小数以匹配 f32 参数。
                source.push_str(".0");
            }
        }
        // 一元表达式递归操作数。
        ExpressionKind::Unary { operand, .. } => normalize_number_literals(operand),
        // 二元表达式递归两侧。
        ExpressionKind::Binary { left, right, .. } => {
            // 规范化左侧。
            normalize_number_literals(left);
            // 规范化右侧。
            normalize_number_literals(right);
        }
        // 三元表达式递归条件与分支。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            // 规范化条件。
            normalize_number_literals(condition);
            // 规范化真分支。
            normalize_number_literals(then_branch);
            // 规范化假分支。
            normalize_number_literals(else_branch);
        }
        // 成员访问递归对象。
        ExpressionKind::Member { object, .. } => normalize_number_literals(object),
        // 索引访问递归对象与下标。
        ExpressionKind::Index { object, index } => {
            // 规范化被索引对象。
            normalize_number_literals(object);
            // 规范化下标。
            normalize_number_literals(index);
        }
        // 调用递归目标与参数。
        ExpressionKind::Call { callee, arguments } => {
            // 规范化调用目标。
            normalize_number_literals(callee);
            // 规范化参数值。
            for argument in arguments {
                // 递归当前参数。
                normalize_number_literals(&mut argument.value);
            }
        }
        // 对象递归字段值。
        ExpressionKind::Object(fields) => {
            // 规范化字段值。
            for field in fields {
                // 递归当前字段。
                normalize_number_literals(&mut field.value);
            }
        }
        // 数组递归元素。
        ExpressionKind::Array(items) => {
            // 规范化元素。
            for item in items {
                // 递归当前元素。
                normalize_number_literals(item);
            }
        }
        // 受限闭包递归规范化唯一表达式体。
        ExpressionKind::Closure { body, .. } => normalize_number_literals(body),
        // 其余叶节点不需要规范化。
        ExpressionKind::Identifier(_) | ExpressionKind::String(_) | ExpressionKind::Boolean(_) => {}
    }
}

// 递归穿透成员与调用链，返回最内层标识符。
pub(crate) fn data_chain_root(expression: &Expression) -> Option<&str> {
    // 保存当前游标。
    let mut cursor = expression;
    // 循环穿透构造链结构。
    loop {
        // 按当前节点形状继续。
        match &cursor.kind {
            // 标识符是链的根。
            ExpressionKind::Identifier(name) => return Some(name.as_str()),
            // 成员访问继续穿透对象。
            ExpressionKind::Member { object, .. } => cursor = object,
            // 调用继续穿透目标。
            ExpressionKind::Call { callee, .. } => cursor = callee,
            // 其余结构不是数据构造链。
            _ => return None,
        }
    }
}

// 判断表达式是否属于已登记数据类型的构造链。
pub(crate) fn is_data_constructor_chain(expression: &Expression) -> bool {
    // 穿透到链根并核对构造白名单。
    data_chain_root(expression).is_some_and(is_registered_data_type)
}

// Step.status 的字符串语义值到公开枚举路径令牌映射。
pub(crate) fn step_status_path(value: &str) -> Option<TokenStream> {
    // 按语言面语义值映射枚举变体。
    match value {
        // 映射已完成步骤。
        "finish" => Some(quote! { ::uix::prelude::StepStatus::Finish }),
        // 映射进行中步骤。
        "process" => Some(quote! { ::uix::prelude::StepStatus::Process }),
        // 映射等待中步骤。
        "wait" => Some(quote! { ::uix::prelude::StepStatus::Wait }),
        // 其他值不在登记表。
        _ => None,
    }
}
