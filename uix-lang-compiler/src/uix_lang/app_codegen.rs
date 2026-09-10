// 引入确定性名称集合。
use std::collections::BTreeSet;

// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入 Compiler System 唯一主题 token 登记。
use crate::projection_schema::UI_PROJECTION_SCHEMA;

// 引入应用入口生成所需的完整 UIX 语法树。
use super::{
    Attribute, AttributeValue, ControlBinding, Declaration, Diagnostic, Document, Element,
    Expression, ExpressionKind, Node, SourceSpan, WidgetStateInitial,
};
// 引入共享 View、颜色与完整主题 token 生成事实。
use super::{generate_document_view, parse_color};
// 引入实际 DesignTokens 字段更新入口。
use super::theme_token_codegen::theme_token_update;

// 保存完成验证的 App 字符串属性。
#[derive(Default)]
struct AppAttributes {
    // 保存可选窗口标题。
    title: Option<String>,
    // 保存可选窗口尺寸。
    size: Option<(i32, i32)>,
    // 保存初始主题名称。
    theme: Option<String>,
    // 保存初始主题属性的精确诊断位置。
    theme_span: Option<SourceSpan>,
    // 保存可选设置文件路径。
    settings: Option<String>,
}

// 把根为 <App> 的文档生成为现有公开 App builder。
pub(crate) fn generate_document_app(document: &Document) -> Result<TokenStream, Diagnostic> {
    // 独立应用入口必须显式使用 App 根。
    if document.root.name != "App" {
        // 返回入口形状诊断。
        return Err(Diagnostic::new(
            // 指向实际文档根。
            document.root.span,
            // 说明根标签要求。
            format!("uix_app! 需要 <App> 根，当前根为 <{}>", document.root.name),
            // 给出 View 与 App 两条正确入口。
            "使用 <App> 包裹唯一 View 根，或改用 uix!(...) 生成 ViewNode",
        ));
    }
    // 解析并拒绝 App 范围外的属性。
    let attributes = parse_app_attributes(&document.root.attributes)?;
    // 提取 App 的唯一可渲染直接子根。
    let child = app_child(&document.root)?;
    // 收集同文档具名主题并保持声明顺序。
    let theme_names = document
        // 遍历顶层声明。
        .declarations
        // 借用声明序列。
        .iter()
        // 只保留主题名称。
        .filter_map(|declaration| match declaration {
            // 返回主题名借用。
            Declaration::Theme(theme) => Some(theme.name.as_str()),
            // 忽略其他声明。
            _ => None,
        })
        // 收集到确定性集合。
        .collect::<BTreeSet<_>>();
    // 验证初始主题精确命中同文档声明或内建预设。
    let initial_theme = attributes.theme.as_deref().unwrap_or("light");
    // 未知初始主题必须在宏展开期失败。
    validate_theme_name(
        // 验证最终初始主题名称。
        initial_theme,
        // 显式 theme 使用属性跨度，默认主题使用 App 根跨度。
        attributes.theme_span.unwrap_or(document.root.span),
        // 使用同一 App 名称集合。
        &theme_names,
    )?;
    // 文档中的 setTheme 字面量使用同一名称表验证。
    validate_theme_requests(document, &theme_names)?;
    // 把 App 子根作为独立 View 文档交给既有组件展开器。
    let view_document = Document {
        // 复用顶层组件、样式、record 与主题声明。
        declarations: document.declarations.clone(),
        // 使用唯一 App 子元素作为 View 根。
        root: child,
    };
    // 生成不持有窗口生命周期的 ViewNode 表达式。
    let view = generate_document_view(&view_document)?;
    // 为全部同文档主题生成拥有所有权的 Theme 值。
    let themes = document
        // 遍历顶层声明。
        .declarations
        // 借用声明序列。
        .iter()
        // 生成主题映射条目并传播诊断。
        .filter_map(|declaration| match declaration {
            // 主题声明进入生成器。
            Declaration::Theme(theme) => Some(generate_named_theme(theme)),
            // 其他声明不进入 App 主题表。
            _ => None,
        })
        // 收集全部成功条目。
        .collect::<Result<Vec<_>, _>>()?;
    // 从唯一现有 App builder 开始组装。
    let mut app = quote! { ::uix_app::prelude::App::new() };
    // 可选标题只调用现有公开 builder。
    if let Some(title) = attributes.title {
        // 追加标题配置。
        app = quote! { (#app).title(#title) };
    }
    // 可选尺寸只调用现有公开 builder。
    if let Some((width, height)) = attributes.size {
        // 追加严格解析后的尺寸配置。
        app = quote! { (#app).size(#width, #height) };
    }
    // 可选设置路径只调用现有公开 builder。
    if let Some(settings) = attributes.settings {
        // 追加既有设置加载入口。
        app = quote! { (#app).settings(#settings) };
    }
    // 安装 App 作用域主题表；同名文档主题覆盖内建预设。
    app = quote! {
        (#app).__uix_named_themes(::std::vec![#(#themes),*], #initial_theme)
    };
    // 返回可继续链式配置且尚未运行的 App builder。
    Ok(quote! {
        (#app).root(move || {
            // 每次协调都由现有 App 根工厂构建 ViewNode。
            #view
        })
    })
}

// 解析 App 第一版四项字符串属性。
fn parse_app_attributes(attributes: &[Attribute]) -> Result<AppAttributes, Diagnostic> {
    // 创建空的验证结果。
    let mut parsed = AppAttributes::default();
    // 记录已经出现的名称以拒绝重复声明。
    let mut seen = BTreeSet::new();
    // 按源码顺序解析属性。
    for attribute in attributes {
        // 重复属性会导致配置归属不清。
        if !seen.insert(attribute.name.as_str()) {
            // 返回重复属性诊断。
            return Err(Diagnostic::new(
                // 指向后出现的属性。
                attribute.span,
                // 说明重复名称。
                format!("App 属性 {} 重复声明", attribute.name),
                // 给出合并动作。
                "合并重复属性并只保留一次",
            ));
        }
        // 四项属性均只接受字符串字面量。
        let value = match &attribute.value {
            // 克隆已经解码的字面量。
            AttributeValue::Literal(value) => value.clone(),
            // 表达式与内联样式均不属于 App 配置契约。
            _ => {
                // 返回字面量要求诊断。
                return Err(Diagnostic::new(
                    // 指向完整属性。
                    attribute.span,
                    // 说明具体属性要求。
                    format!("App 属性 {} 只接受字符串字面量", attribute.name),
                    // 给出正确示例形状。
                    "使用 title=\"应用\"、size=\"1200x800\"、theme=\"light\" 或 settings=\"settings.toml\"",
                ));
            }
        };
        // 按批准的第一版白名单写入配置。
        match attribute.name.as_str() {
            // 保存窗口标题。
            "title" => parsed.title = Some(value),
            // 严格解析窗口尺寸。
            "size" => parsed.size = Some(parse_size(&value, attribute)?),
            // 保存初始主题名。
            "theme" => {
                // 保存主题名称。
                parsed.theme = Some(value);
                // 保存属性精确跨度供未知名称诊断。
                parsed.theme_span = Some(attribute.span);
            }
            // 保存设置文件路径。
            "settings" => parsed.settings = Some(value),
            // 其他 App builder 能力留在 Rust 链式配置。
            name => {
                // 返回属性白名单诊断。
                return Err(Diagnostic::new(
                    // 指向未知属性。
                    attribute.span,
                    // 说明第一版边界。
                    format!("App 属性 {name} 尚未登记"),
                    // 指向语言面与 Rust builder 的分工。
                    "只使用 title、size、theme、settings；其他能力在 uix_app!(...) 后链式配置",
                ));
            }
        }
    }
    // 返回完成验证的配置。
    Ok(parsed)
}

// 严格解析正 i32 的 W x H 字符串。
fn parse_size(value: &str, attribute: &Attribute) -> Result<(i32, i32), Diagnostic> {
    // 按单个大小写 x 分隔并保留精确分量数量。
    let parts = value.split(['x', 'X']).map(str::trim).collect::<Vec<_>>();
    // 必须恰好具有宽高两项且都不为空。
    if parts.len() != 2 || parts.iter().any(|part| part.is_empty()) {
        // 返回形状诊断。
        return Err(size_diagnostic(attribute, value));
    }
    // 宽度必须可表示为正 i32。
    let width = parts[0]
        // 解析十进制有符号整数。
        .parse::<i32>()
        // 把格式或溢出统一映射到尺寸诊断。
        .map_err(|_| size_diagnostic(attribute, value))?;
    // 高度必须可表示为正 i32。
    let height = parts[1]
        // 解析十进制有符号整数。
        .parse::<i32>()
        // 把格式或溢出统一映射到尺寸诊断。
        .map_err(|_| size_diagnostic(attribute, value))?;
    // 零或负数不能创建有效初始窗口。
    if width <= 0 || height <= 0 {
        // 返回范围诊断。
        return Err(size_diagnostic(attribute, value));
    }
    // 返回完成验证的窗口尺寸。
    Ok((width, height))
}

// 构造统一尺寸诊断。
fn size_diagnostic(attribute: &Attribute, value: &str) -> Diagnostic {
    // 返回包含原值与严格格式建议的诊断。
    Diagnostic::new(
        // 指向 size 属性。
        attribute.span,
        // 说明无效值。
        format!("App size 值 {value:?} 不是正 i32 的 W x H"),
        // 给出合法示例。
        "使用 size=\"1200x800\"，宽高都必须大于 0 且不超过 i32 上限",
    )
}

// 提取 App 恰好一个可渲染直接子元素。
fn app_child(app: &Element) -> Result<Element, Diagnostic> {
    // 忽略只承担排版的空白文本。
    let renderable = app
        // 遍历直接子节点。
        .children
        // 借用子节点序列。
        .iter()
        // 过滤纯空白文本。
        .filter(|node| !matches!(node, Node::Text(text) if text.value.trim().is_empty()))
        // 收集直接可渲染节点。
        .collect::<Vec<_>>();
    // App 必须直接拥有唯一元素根。
    if renderable.len() != 1 {
        // 返回子根数量诊断。
        return Err(Diagnostic::new(
            // 指向完整 App。
            app.span,
            // 说明实际数量。
            format!(
                "<App> 必须恰有一个可渲染直接子根，当前为 {} 个",
                renderable.len()
            ),
            // 给出显式容器修复动作。
            "用 Column 或 Container 包裹多个子节点，并保留一个直接元素根",
        ));
    }
    // 唯一直接根必须是元素而非裸文本或插值。
    match renderable[0] {
        // 克隆元素供独立 View 文档生成。
        Node::Element(element) => Ok(element.clone()),
        // 裸文本或插值不形成 View 根。
        _ => Err(Diagnostic::new(
            // 指向完整 App。
            app.span,
            // 说明元素根要求。
            "<App> 的唯一直接子根必须是 View 元素",
            // 给出最小包装建议。
            "使用 <Text>...</Text> 或 Container 包裹内容",
        )),
    }
}

// 生成一个文档主题映射条目。
fn generate_named_theme(
    // 接收已经通过名称去重的主题声明。
    theme: &super::ThemeDeclaration,
) -> Result<TokenStream, Diagnostic> {
    // dark 名称默认继承暗色基元，其余名称继承亮色基元。
    let dark_base = theme.name == "dark";
    // 保存主题基元更新语句。
    let mut primitive_updates = Vec::new();
    // 保存派生后精确 token 更新语句。
    let mut token_updates = Vec::new();
    // 按声明顺序验证完整运行时 token 白名单。
    for property in &theme.properties {
        // 名称支持面由 schema 裁决，生成器只负责已登记字段映射。
        if UI_PROJECTION_SCHEMA.theme_token(&property.name).is_none() {
            // 未登记名称在生成前失败并保留原始 SourceSpan。
            return Err(Diagnostic::new(
                property.span,
                format!("主题 token {} 尚未登记", property.name),
                "使用 UiProjectionSchema 已公开的 camelCase token 名",
            ));
        }
        // 两个历史别名继续驱动基元色板推导。
        match property.name.as_str() {
            // 主色同步更新 primary 与 info 基元。
            "primaryColor" => {
                // 解析兼容别名颜色。
                let (red, green, blue, alpha) = parse_color(&property.value.source, property)?;
                // 生成公开颜色值。
                let color =
                    quote! { ::uix_app::prelude::Color::from_rgba(#red, #green, #blue, #alpha) };
                // 同步更新品牌与信息基元。
                primitive_updates.push(quote! {
                    __uix_primitives.primary = #color;
                    __uix_primitives.info = #color;
                });
            }
            // 背景别名同时驱动推导并保留作者精确布局背景。
            "backgroundColor" => {
                // 解析兼容别名颜色。
                let (red, green, blue, alpha) = parse_color(&property.value.source, property)?;
                // 生成公开颜色值。
                let color =
                    quote! { ::uix_app::prelude::Color::from_rgba(#red, #green, #blue, #alpha) };
                // 更新主题明暗推导使用的背景基元。
                primitive_updates.push(quote! { __uix_primitives.bg = #color; });
                // 派生后保持兼容别名的精确值。
                token_updates.push(quote! { __uix_tokens.color_bg_layout = #color; });
            }
            // 其余名称逐项对齐运行时 DesignTokens 字段。
            _ => token_updates.push(theme_token_update(property)?),
        }
    }
    // 保存主题名称。
    let name = &theme.name;
    // 返回拥有所有权的名称与主题值。
    Ok(quote! {
        (#name, {
            // 选择可预测的内建基元作为未声明字段的基线。
            let mut __uix_primitives = if #dark_base {
                // dark 同名主题继承暗色基元。
                ::uix_app::prelude::ThemePrimitives::antd_dark()
            } else {
                // 其他主题继承亮色基元。
                ::uix_app::prelude::ThemePrimitives::antd_light()
            };
            // 应用主题声明的基元覆盖。
            #(#primitive_updates)*
            // 背景亮度决定完整 token 的明暗推导。
            let __uix_is_dark = !__uix_primitives.bg.is_light();
            // 从基元生成完整公开设计令牌。
            let mut __uix_tokens = __uix_primitives.into_design_tokens(__uix_is_dark);
            // 保持作者声明背景色的精确语义。
            #(#token_updates)*
            // 包装为 App 接受的 Theme 值。
            ::uix_app::prelude::Theme::new(__uix_tokens)
        })
    })
}

// 验证一个主题名称可由当前 App 解析。
fn validate_theme_name(
    // 接收待验证名称。
    name: &str,
    // 接收诊断跨度。
    span: super::SourceSpan,
    // 接收同文档主题名称。
    themes: &BTreeSet<&str>,
) -> Result<(), Diagnostic> {
    // 同文档名称优先，内建 light 与 dark 次之。
    if themes.contains(name) || matches!(name, "light" | "dark") {
        // 返回验证成功。
        return Ok(());
    }
    // 未知名称必须在编译期失败。
    Err(Diagnostic::new(
        // 指向引用位置。
        span,
        // 说明未知名称。
        format!("App 作用域内没有主题 {name:?}"),
        // 给出声明或内建名称修复方式。
        "声明同名 @theme，或使用内建 light / dark",
    ))
}

// 验证完整文档中的 setTheme 字面量。
fn validate_theme_requests(
    // 接收完整文档。
    document: &Document,
    // 接收可解析名称集合。
    themes: &BTreeSet<&str>,
) -> Result<(), Diagnostic> {
    // 验证文档根子树。
    validate_element_theme_requests(&document.root, themes)?;
    // 验证全部组件声明，即使组件稍后才被展开。
    for declaration in &document.declarations {
        // 只处理组件体与私有状态表达式。
        if let Declaration::Widget(widget) = declaration {
            // 验证组件有序子节点。
            for node in &widget.children {
                // 验证单个节点。
                validate_node_theme_requests(node, themes)?;
            }
            // 验证组件私有状态初始表达式。
            for state in &widget.states {
                // 提取可包含调用的表达式。
                let expression = match &state.initial {
                    // 普通表达式直接借用。
                    WidgetStateInitial::Expression(expression) => Some(expression),
                    // 类型化表达式直接借用。
                    WidgetStateInitial::TypedExpression(_, expression) => Some(expression),
                    // 空数组没有调用。
                    WidgetStateInitial::EmptyArray => None,
                };
                // 存在表达式时递归验证。
                if let Some(expression) = expression {
                    // 验证表达式树。
                    validate_expression_theme_requests(expression, themes)?;
                }
            }
        }
    }
    // 返回整份文档验证成功。
    Ok(())
}

// 递归验证元素中的属性、控制绑定与子节点表达式。
fn validate_element_theme_requests(
    // 接收待验证元素。
    element: &Element,
    // 接收可解析主题名称。
    themes: &BTreeSet<&str>,
) -> Result<(), Diagnostic> {
    // 验证表达式属性。
    for attribute in &element.attributes {
        // 只处理表达式属性。
        if let AttributeValue::Expression(expression) = &attribute.value {
            // 验证属性表达式。
            validate_expression_theme_requests(&expression.expression, themes)?;
        }
    }
    // 验证 If 与 For 专用控制表达式。
    if let Some(control) = &element.control {
        // 按控制绑定形状递归。
        match control {
            // 验证 If 或 ElseIf 条件表达式。
            ControlBinding::If(condition) => {
                // 递归条件表达式。
                validate_expression_theme_requests(&condition.expression, themes)?;
            }
            // 验证 For 数据源与可选 key 表达式。
            ControlBinding::For { iterable, key, .. } => {
                // 递归数据源表达式。
                validate_expression_theme_requests(&iterable.expression, themes)?;
                // 存在 key 时递归稳定身份表达式。
                if let Some(key) = key {
                    // 验证 key 表达式。
                    validate_expression_theme_requests(&key.expression, themes)?;
                }
            }
        }
    }
    // 验证有序子节点。
    for node in &element.children {
        // 验证单个节点。
        validate_node_theme_requests(node, themes)?;
    }
    // 返回元素验证成功。
    Ok(())
}

// 递归验证一个 UIX 节点中的主题请求。
fn validate_node_theme_requests(
    // 接收节点。
    node: &Node,
    // 接收可解析主题名称。
    themes: &BTreeSet<&str>,
) -> Result<(), Diagnostic> {
    // 按节点形状递归。
    match node {
        // 元素递归验证属性与子节点。
        Node::Element(element) => validate_element_theme_requests(element, themes),
        // 普通文本没有表达式。
        Node::Text(_) => Ok(()),
        // 插值表达式递归验证。
        Node::Interpolation(expression) => {
            // 验证插值表达式。
            validate_expression_theme_requests(&expression.expression, themes)
        }
        // 成员块已在声明解析阶段剥离，不会出现在文档树中。
        Node::WidgetMember(_) => Ok(()),
    }
}

// 递归查找并验证 setTheme 调用。
fn validate_expression_theme_requests(
    // 接收表达式 AST。
    expression: &Expression,
    // 接收可解析主题名称。
    themes: &BTreeSet<&str>,
) -> Result<(), Diagnostic> {
    // 按表达式形状递归。
    match &expression.kind {
        // action 内联只存在于 Widget 事件，App 主题预检不会接收该形状。
        ExpressionKind::LoweredAction(_) => Ok(()),
        // 调用先检查 setTheme，再检查全部子表达式。
        ExpressionKind::Call { callee, arguments } => {
            // 识别已经由 parser 限制为单字符串参数的内建调用。
            if matches!(&callee.kind, ExpressionKind::Identifier(name) if name == "setTheme") {
                // parser 已保证首参数存在且是字符串。
                if let Some(argument) = arguments.first() {
                    // 提取静态主题名称。
                    if let ExpressionKind::String(name) = &argument.value.kind {
                        // 使用调用参数跨度报告未知名称。
                        validate_theme_name(name, argument.span, themes)?;
                    }
                }
            }
            // 验证调用目标。
            validate_expression_theme_requests(callee, themes)?;
            // 验证全部参数值。
            for argument in arguments {
                // 递归参数表达式。
                validate_expression_theme_requests(&argument.value, themes)?;
            }
            // 返回调用验证成功。
            Ok(())
        }
        // 对象字段值逐项递归。
        ExpressionKind::Object(fields) => fields.iter().try_for_each(|field| {
            // 验证字段值。
            validate_expression_theme_requests(&field.value, themes)
        }),
        // 数组项逐项递归。
        ExpressionKind::Array(items) => items.iter().try_for_each(|item| {
            // 验证数组项。
            validate_expression_theme_requests(item, themes)
        }),
        // 受限闭包递归验证唯一表达式体中的主题请求。
        ExpressionKind::Closure { body, .. } => validate_expression_theme_requests(body, themes),
        // 一元表达式递归操作数。
        ExpressionKind::Unary { operand, .. } => {
            // 验证一元操作数。
            validate_expression_theme_requests(operand, themes)
        }
        // 二元表达式递归两侧。
        ExpressionKind::Binary { left, right, .. } => {
            // 先验证左侧。
            validate_expression_theme_requests(left, themes)?;
            // 再验证右侧。
            validate_expression_theme_requests(right, themes)
        }
        // 三元表达式递归全部分支。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            // 验证条件。
            validate_expression_theme_requests(condition, themes)?;
            // 验证真分支。
            validate_expression_theme_requests(then_branch, themes)?;
            // 验证假分支。
            validate_expression_theme_requests(else_branch, themes)
        }
        // 成员访问递归对象。
        ExpressionKind::Member { object, .. } => {
            // 验证成员对象。
            validate_expression_theme_requests(object, themes)
        }
        // 下标访问递归对象与索引。
        ExpressionKind::Index { object, index } => {
            // 验证被索引对象。
            validate_expression_theme_requests(object, themes)?;
            // 验证索引表达式。
            validate_expression_theme_requests(index, themes)
        }
        // 叶表达式不包含主题调用。
        ExpressionKind::Identifier(_)
        | ExpressionKind::Number(_)
        | ExpressionKind::String(_)
        | ExpressionKind::Boolean(_) => Ok(()),
    }
}
