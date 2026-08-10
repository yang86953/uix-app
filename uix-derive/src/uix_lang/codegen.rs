// 引入过程宏令牌与卫生标识符。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入解析后的核心语法树与诊断类型。
use super::{
    Attribute, AttributeValue, ControlBinding, Diagnostic, Element, ExpressionNode, Node,
    SourceSpan,
};
// 引入受限表达式与事件处理器生成入口。
use super::{
    expression_uses_event, generate_affix, generate_app_layout, generate_autocomplete,
    generate_back_top, generate_button_group, generate_cascader, generate_checkbox,
    generate_color_picker, generate_column, generate_container, generate_date_picker,
    generate_date_range_picker, generate_divider, generate_expression, generate_float_button,
    generate_form, generate_grid, generate_handler_expression, generate_input,
    generate_input_group, generate_input_number, generate_mentions, generate_orphan_col,
    generate_orphan_form_input_item, generate_orphan_form_select_item, generate_radio,
    generate_range_slider, generate_rate, generate_row, generate_scroll_view, generate_segmented,
    generate_select, generate_slider, generate_space, generate_splitter, generate_switch,
    generate_theme_toggle, generate_time_picker, generate_tree_select, generate_typography,
    generate_virtual_scroll, generate_window_control,
};
// 引入属性值与绑定名称的共享生成入口。
use super::{
    align_value, apply_inline_style, boolean_value, deferred_style_diagnostic, justify_value,
    literal_string, numeric_value, planned_builtin_diagnostic, rust_identifier, string_value,
    typography_value,
};

// 生成一个可直接消费的公开 UIX View 表达式。
pub(crate) fn generate_view(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 控制节点只能在父元素的有序子节点列表中展开。
    if matches!(element.name.as_str(), "If" | "For") {
        // 返回根控制节点形状诊断。
        return Err(Diagnostic::new(
            // 指向完整控制元素。
            element.span,
            // 说明单根 View 要求。
            format!("<{}> 不能作为独立 View 根节点生成", element.name),
            // 给出父容器修复建议。
            "把 If 或 For 放入 Container、Row 或 Column 内",
        ));
    }
    // 普通元素委托映射矩阵生成。
    let view = generate_element(element)?;
    // 通过公开 View trait 统一物化为 ViewNode。
    Ok(quote! { ::uix::prelude::View::build(#view) })
}

// 按当前核心映射矩阵生成一个普通元素。
fn generate_element(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 普通元素不应携带控制绑定。
    if element.control.is_some() {
        // 返回内部形状保护诊断。
        return Err(Diagnostic::new(
            // 指向完整元素。
            element.span,
            // 说明控制绑定归属错误。
            format!("<{}> 不能携带 If/For 控制绑定", element.name),
            // 给出合法控制标签。
            "只在 <If> 或 <For> 元素上声明控制绑定",
        ));
    }
    // 只登记本 Gate 可由现有公开 API 确定表达的核心组件。
    match element.name.as_str() {
        // 文本映射到公开 label 构造器。
        "Text" => generate_text(element),
        // 文档通用组件 Label 与 Text 使用同一公开构造器。
        "Label" => generate_text(element),
        // 按钮映射到公开 button 构建器。
        "Button" => generate_button(element),
        // 通用容器按 direction 映射到 Flex row 或 column。
        "Container" => generate_container(element),
        // 文档 Row 映射到 24 单元响应式栅格。
        "Row" => generate_row(element),
        // 显式列容器保留 Flex column 兼容入口。
        "Column" => generate_column(element),
        // 显式 Grid 映射到公开轨道构建器。
        "Grid" => generate_grid(element),
        // 滚动容器映射到公开 ScrollBuilder，并保留状态所有权。
        "ScrollView" => generate_scroll_view(element),
        // 虚拟滚动映射到公开 VirtualScroll 惰性 renderer。
        "VirtualScroll" => generate_virtual_scroll(element),
        // 固钉容器映射到公开 Affix，并从 State<f32> 读取当前滚动位置。
        "Affix" => generate_affix(element),
        // 回到顶部按钮映射到持有 State<f32> 句柄的公开 BackTop。
        "BackTop" => generate_back_top(element),
        // 双面板分隔器映射到公开 Splitter，并保留运行时交互所有权。
        "Splitter" => generate_splitter(element),
        // 应用布局壳五类标签共享公开运行时组合生成入口。
        "Layout" | "Sider" | "Header" | "Content" | "Footer" => generate_app_layout(element),
        // 文本输入映射到公开 Input 与 View Change 事件契约。
        "Input" => generate_input(element),
        // 数值输入映射到公开 InputNumber 绑定、范围、步长与精度契约。
        "InputNumber" => generate_input_number(element),
        // 复合文本输入映射到公开 InputGroup 附加文本与状态绑定契约。
        "InputGroup" => generate_input_group(element),
        // 单值滑块映射到公开 Slider 范围、步长与状态绑定契约。
        "Slider" => generate_slider(element),
        // 区间滑块映射到公开 RangeSlider 范围、步长与双状态绑定契约。
        "RangeSlider" => generate_range_slider(element),
        // 评分组件映射到公开 Rate 星数、半星与状态绑定契约。
        "Rate" => generate_rate(element),
        // 复选框映射到公开 Checkbox 标签、禁用与双向勾选契约。
        "Checkbox" => generate_checkbox(element),
        // 开关映射到公开 Switch 禁用与双向勾选契约。
        "Switch" => generate_switch(element),
        // 单选组映射到公开 Radio 选项与 State<String> 双向值绑定契约。
        "Radio" => generate_radio(element),
        // 分段控制器映射到公开 Segmented 选项与 State<String> 双向值绑定契约。
        "Segmented" => generate_segmented(element),
        // 下拉选择器映射到结构化选项和编译期单选或多选状态契约。
        "Select" => generate_select(element),
        // 级联选择器映射到选项树与 State<CascaderValue> 双向路径契约。
        "Cascader" => generate_cascader(element),
        // 树形选择器映射到 TreeNode 树与 State<String> 稳定 key 双向契约。
        "TreeSelect" => generate_tree_select(element),
        // 自动完成输入映射到字符串候选与 State<String> 双向文本契约。
        "AutoComplete" => generate_autocomplete(element),
        // 提及输入映射到字符串候选与 State<String> 完整文本双向契约。
        "Mentions" => generate_mentions(element),
        // 日期选择映射到 State<Date> 双向值与确定的选择粒度契约。
        "DatePicker" => generate_date_picker(element),
        // 日期范围映射到 start/end 两个 State<Date> 的结构化双向契约。
        "DateRangePicker" => generate_date_range_picker(element),
        // 时间选择映射到 State<Time> 双向值契约。
        "TimePicker" => generate_time_picker(element),
        // 颜色选择映射到 State<Color> 双向值契约。
        "ColorPicker" => generate_color_picker(element),
        // 表单映射到类型化模型、字段投影与提交闭环。
        "Form" => generate_form(element),
        // FormInputItem 只能由 Form 解释类型化字段语义。
        "FormInputItem" => generate_orphan_form_input_item(element),
        // FormSelectItem 只能由 Form 解释类型化选择字段语义。
        "FormSelectItem" => generate_orphan_form_select_item(element),
        // Col 只能由 Row 或 Grid 解释其父级布局语义。
        "Col" => generate_orphan_col(element),
        // 图标映射到公开 Icon 组件。
        "Icon" => generate_icon(element),
        // 分割线映射到现有 Divider Component。
        "Divider" => generate_divider(element),
        // 间距容器映射到现有 Space Component。
        "Space" => generate_space(element),
        // 排版文本映射到现有 Typography Component。
        "Typography" => generate_typography(element),
        // 主题切换映射到现有 ThemeToggle Component。
        "ThemeToggle" => generate_theme_toggle(element),
        // 按钮组映射到静态直接 Button 子项的连体组合。
        "ButtonGroup" => generate_button_group(element),
        // 窗口控制映射到 window_chrome 公开组合函数。
        "WindowControl" => generate_window_control(element),
        // 浮动按钮映射到现有 FloatButton Component。
        "FloatButton" => generate_float_button(element),
        // 文档内置组件按登记类别返回规划中诊断。
        _ if planned_builtin_diagnostic(element).is_some() => {
            // 前置条件保证诊断存在。
            Err(planned_builtin_diagnostic(element).expect("已确认规划中内置组件已登记"))
        }
        // 真正未知元素仍返回普通映射诊断。
        _ => Err(Diagnostic::new(
            // 指向未登记元素。
            element.span,
            // 说明没有静默猜测映射。
            format!("元素 <{}> 尚无已登记的 Rust API 映射", element.name),
            // 指向明确支持路径。
            "使用 Text、Label、Button、ButtonGroup、FloatButton、Icon、Divider、Space、Typography、ThemeToggle、WindowControl、Container、Row、Column、Grid、ScrollView、VirtualScroll、Splitter、Affix、BackTop、Layout、Sider、Header、Content、Footer、Input、InputNumber、InputGroup、Slider、RangeSlider、Rate、Checkbox、Switch、Radio、Segmented、Select、Cascader、TreeSelect、AutoComplete、Mentions、DatePicker、DateRangePicker 或 TimePicker，或先登记组件状态",
        )),
    }
}

// 生成文本元素。
fn generate_text(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 把有序文本与插值组合成单一内容表达式。
    let content = generate_text_content(&element.children, element.span)?;
    // 构造公开 label View。
    let base = quote! { ::uix::prelude::label(#content) };
    // 应用文本支持的公共属性与事件。
    apply_common_attributes(base, &element.attributes, &[])
}

// 生成按钮元素。
fn generate_button(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 普通按钮不声明 ButtonGroup 连体位置。
    generate_button_with_group_position(element, None)
}

// 生成按钮，并在公共样式物化前应用可选 ButtonGroup 连体位置。
pub(super) fn generate_button_with_group_position(
    // 接收 Button 元素。
    element: &Element,
    // 接收可选的公开 ButtonGroupPosition 表达式。
    group_position: Option<TokenStream>,
) -> Result<TokenStream, Diagnostic> {
    // 按钮当前公开 API 只接收文本内容。
    let content = generate_text_content(&element.children, element.span)?;
    // 构造公开按钮构建器。
    let mut view = quote! { ::uix::prelude::button(#content) };
    // 先处理必须在 StyleExt 物化前调用的按钮专有属性。
    for attribute in &element.attributes {
        // 按钮类型选择公开变体方法。
        if attribute.name == "type" {
            // 要求类型为编译期字面量。
            let kind = literal_string(attribute, "Button type")?;
            // 按登记类型应用构建器变体。
            view = match kind.as_str() {
                // 默认类型不增加链式调用。
                "default" => view,
                // 主按钮调用 primary。
                "primary" => quote! { (#view).primary() },
                // 幽灵按钮调用 ghost。
                "ghost" => quote! { (#view).ghost() },
                // 危险按钮调用 danger。
                "danger" => quote! { (#view).danger() },
                // 未登记类型返回编译期诊断。
                _ => {
                    // 返回按钮类型映射诊断。
                    return Err(Diagnostic::new(
                        // 指向完整属性。
                        attribute.span,
                        // 说明未知类型。
                        format!("Button type={kind:?} 尚无公开构建器映射"),
                        // 给出已登记类型集合。
                        "使用 default、primary、ghost 或 danger",
                    ));
                }
            };
        }
        // 禁用状态映射到按钮构建器。
        if attribute.name == "disabled" {
            // 生成布尔属性表达式。
            let value = boolean_value(attribute)?;
            // 应用禁用状态。
            view = quote! { (#view).disabled(#value) };
        }
        // 块级按钮状态映射到按钮构建器。
        if attribute.name == "block" {
            // 生成布尔属性表达式。
            let value = boolean_value(attribute)?;
            // 应用块级状态。
            view = quote! { (#view).block(#value) };
        }
    }
    // ButtonGroup 子按钮在公共样式与事件物化前写入连体位置。
    if let Some(position) = group_position {
        // 调用 ButtonBuilder 的公开连体位置物化入口。
        view = quote! { (#view).group_position(#position) };
    }
    // 在按钮专有属性之后应用公共 View 属性与事件。
    apply_common_attributes(view, &element.attributes, &["type", "disabled", "block"])
}

// 生成图标元素。
fn generate_icon(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 图标不能声明子内容。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶子组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整图标元素。
            element.span,
            // 说明 Icon 是叶子组件。
            "<Icon> 不接受子节点",
            // 给出自闭合写法。
            "使用 <Icon name=\"star\" />",
        ));
    }
    // name 是图标构造器的必需参数。
    let name_attribute = element
        // 借用属性列表。
        .attributes
        // 遍历属性。
        .iter()
        // 查找 name。
        .find(|attribute| attribute.name == "name")
        // 缺失时生成结构化诊断。
        .ok_or_else(|| {
            // 构造必需属性诊断。
            Diagnostic::new(
                // 指向完整图标元素。
                element.span,
                // 说明缺少构造参数。
                "<Icon> 缺少必需的 name 属性",
                // 给出规范示例。
                "使用 <Icon name=\"star\" />",
            )
        })?;
    // 生成字符串或表达式图标名称。
    let name = string_value(name_attribute)?;
    // 构造公开 Icon 组件。
    let mut icon = quote! { ::uix::prelude::Icon::new(#name) };
    // 可选 size 必须在包装为 ViewNode 前应用。
    if let Some(attribute) = element
        // 借用属性列表。
        .attributes
        // 遍历属性。
        .iter()
        // 查找 size。
        .find(|attribute| attribute.name == "size")
    {
        // 生成像素或表达式数值。
        let size = numeric_value(attribute)?;
        // 应用图标尺寸。
        icon = quote! { (#icon).size(#size) };
    }
    // 把 WidgetComponent 包装为公开 ViewNode。
    let base = quote! { ::uix::prelude::ViewNode::leaf(#icon) };
    // 应用其余公共属性与事件。
    apply_common_attributes(base, &element.attributes, &["name", "size"])
}

// 应用所有核心元素共享的公开 View 属性与事件。
pub(super) fn apply_common_attributes(
    // 接收已经生成的基础 View 表达式。
    mut view: TokenStream,
    // 接收源顺序属性。
    attributes: &[Attribute],
    // 接收元素专有阶段已经消费的属性名。
    consumed: &[&str],
) -> Result<TokenStream, Diagnostic> {
    // 先应用非事件属性，避免提前捕获构建器。
    for attribute in attributes {
        // 跳过元素专有阶段已经消费的属性。
        if consumed.contains(&attribute.name.as_str()) {
            // 继续处理下一属性。
            continue;
        }
        // 事件在第二轮统一处理。
        if attribute.name.starts_with('@') {
            // 继续处理下一普通属性。
            continue;
        }
        // 按公开 View API 映射登记属性。
        view = match attribute.name.as_str() {
            // 间距映射到 ViewNode::gap。
            "gap" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用子节点间距。
                quote! { (#view).gap(#value) }
            }
            // 内边距映射到 ViewNode::padding。
            "padding" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用统一内边距。
                quote! { (#view).padding(#value) }
            }
            // 外边距映射到 ViewNode::margin。
            "margin" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用统一外边距。
                quote! { (#view).margin(#value) }
            }
            // 宽度映射到 ViewNode::width。
            "width" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用固定宽度。
                quote! { (#view).width(#value) }
            }
            // 高度映射到 ViewNode::height。
            "height" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用固定高度。
                quote! { (#view).height(#value) }
            }
            // 主轴扩张映射到 ViewNode::flex_grow。
            "flexGrow" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用扩张因子。
                quote! { (#view).flex_grow(#value) }
            }
            // 主轴收缩映射到 ViewNode::flex_shrink。
            "flexShrink" => {
                // 生成数值。
                let value = numeric_value(attribute)?;
                // 应用收缩因子。
                quote! { (#view).flex_shrink(#value) }
            }
            // 文本颜色映射到公开 color 样式。
            "color" => {
                // 生成字符串或 Rust 表达式。
                let value = string_value(attribute)?;
                // 应用前景色。
                quote! { (#view).color(#value) }
            }
            // 背景色映射到公开 bg 样式。
            "backgroundColor" => {
                // 生成字符串或 Rust 表达式。
                let value = string_value(attribute)?;
                // 应用背景色。
                quote! { (#view).bg(#value) }
            }
            // 字号映射到公开 TypographyToken 或数值。
            "fontSize" => {
                // 生成字号表达式。
                let value = typography_value(attribute)?;
                // 应用字号。
                quote! { (#view).font_size(#value) }
            }
            // 自动化标识映射到公开 automation_id。
            "automationId" => {
                // 生成字符串或表达式。
                let value = string_value(attribute)?;
                // 应用稳定自动化标识。
                quote! { (#view).automation_id(#value) }
            }
            // 普通 key 映射到公开 ViewNode::key。
            "key" => {
                // 生成字符串或表达式。
                let value = string_value(attribute)?;
                // 统一格式化为公开 ViewNode 接受的稳定字符串身份。
                quote! { (#view).key(::std::format!("{}", #value)) }
            }
            // 交叉轴对齐映射到公开枚举。
            "align" => {
                // 生成登记的对齐枚举。
                let value = align_value(attribute)?;
                // 应用交叉轴对齐。
                quote! { (#view).align(#value) }
            }
            // 主轴对齐映射到公开枚举。
            "justify" => {
                // 生成登记的对齐枚举。
                let value = justify_value(attribute)?;
                // 应用主轴对齐。
                quote! { (#view).justify(#value) }
            }
            // 样式类由完整样式映射 Gate 处理。
            "class" => {
                // 返回明确阶段边界诊断。
                return Err(deferred_style_diagnostic(attribute, "class"));
            }
            // 内联样式由完整样式映射 Gate 处理。
            "style" => {
                // 把结构化样式属性精确写入当前 ViewNode 的 Style。
                apply_inline_style(view, attribute)?
            }
            // 未登记属性禁止静默丢弃。
            _ => {
                // 返回未知属性映射诊断。
                return Err(Diagnostic::new(
                    // 指向完整属性。
                    attribute.span,
                    // 说明缺少公开 API 映射。
                    format!("属性 {} 尚无已登记的 Rust API 映射", attribute.name),
                    // 指向后续映射矩阵或可用集合。
                    "使用当前核心属性，或在内置组件映射矩阵 Gate 中登记后再使用",
                ));
            }
        };
    }
    // 在全部普通属性物化后应用事件。
    for attribute in attributes {
        // 专用生成器已经消费的事件不能再次进入核心点击映射。
        if consumed.contains(&attribute.name.as_str()) {
            // 继续处理下一事件属性。
            continue;
        }
        // 只处理事件属性。
        if attribute.name.starts_with('@') {
            // 生成事件链式调用。
            view = apply_event(view, attribute)?;
        }
    }
    // 返回完整 View 表达式。
    Ok(view)
}

// 生成当前核心 Gate 支持的点击事件。
fn apply_event(
    // 接收已经生成的 View。
    view: TokenStream,
    // 接收事件属性。
    attribute: &Attribute,
) -> Result<TokenStream, Diagnostic> {
    // 当前公开映射只登记 click。
    if attribute.name != "@click" {
        // 返回未登记事件诊断。
        return Err(Diagnostic::new(
            // 指向完整事件属性。
            attribute.span,
            // 说明缺少事件映射。
            format!("事件 {} 尚无已登记的 Rust API 映射", attribute.name),
            // 给出当前支持集合。
            "当前核心 Gate 使用 @click；其他事件由内置组件映射矩阵登记",
        ));
    }
    // 事件解析器保证事件值是表达式。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回内部形状保护诊断。
        return Err(Diagnostic::new(
            // 指向完整事件属性。
            attribute.span,
            // 说明事件值形状错误。
            "事件处理器必须是受限表达式",
            // 给出规范写法。
            "使用 @click=\"handler()\"",
        ));
    };
    // 需要事件载荷时使用公开 on_click_event。
    if expression_uses_event(&expression.expression) {
        // 创建卫生的语义事件变量。
        let semantic_event = Ident::new("__uix_semantic_event", Span::mixed_site());
        // 创建卫生的点击载荷变量。
        let click_event = Ident::new("__uix_click_event", Span::mixed_site());
        // 生成把 $event 映射到 ClickEvent 的处理器主体。
        let handler = generate_handler_expression(&expression.expression, Some(&click_event))?;
        // 返回带点击载荷筛选的事件链。
        return Ok(quote! {
            // 使用公开点击事件注册入口。
            (#view).on_click_event(move |#semantic_event| {
                // 只在语义事件带点击载荷时执行语言处理器。
                if let ::std::option::Option::Some(#click_event) = #semantic_event.click_payload() {
                    // 丢弃处理器返回值并保留副作用。
                    let _ = { #handler };
                }
            })
        });
    }
    // 不读取事件载荷时使用轻量点击闭包。
    let handler = generate_handler_expression(&expression.expression, None)?;
    // 返回无事件参数点击链。
    Ok(quote! {
        // 使用公开无状态点击入口。
        (#view).on_click_fn(move || {
            // 丢弃处理器返回值并保留副作用。
            let _ = { #handler };
        })
    })
}

// 生成保持 painter 与组合顺序的子节点向量。
pub(super) fn generate_children(children: &[Node]) -> Result<TokenStream, Diagnostic> {
    // 创建卫生的子节点向量名称。
    let output = Ident::new("__uix_children", Span::mixed_site());
    // 生成有序节点追加语句。
    let statements = generate_child_statements(children, &output)?;
    // 返回构建完成的 ViewNode 向量。
    Ok(quote! {{
        // 创建公开 ViewNode 子节点向量。
        let mut #output = ::std::vec::Vec::<::uix::prelude::ViewNode>::new();
        // 按源码顺序执行节点追加与控制流。
        #statements
        // 返回有序子节点。
        #output
    }})
}

// 生成一组节点的顺序追加语句。
fn generate_child_statements(
    // 接收有序语言节点。
    children: &[Node],
    // 接收目标子节点向量。
    output: &Ident,
) -> Result<TokenStream, Diagnostic> {
    // 保存有序语句令牌。
    let mut statements = Vec::new();
    // 按源码顺序处理每个节点。
    for child in children {
        // 忽略布局容器之间仅用于排版源码的空白。
        if matches!(child, Node::Text(text) if text.value.trim().is_empty()) {
            // 继续处理下一节点。
            continue;
        }
        // 控制元素在当前向量作用域内展开。
        if let Node::Element(element) = child {
            // If 与 For 生成控制流语句而非占位节点。
            if matches!(element.name.as_str(), "If" | "For") {
                // 生成控制流语句。
                statements.push(generate_control(element, output)?);
                // 继续处理下一节点。
                continue;
            }
        }
        // 生成普通节点 View。
        let view = generate_node_view(child)?;
        // 按顺序追加到目标向量。
        statements.push(quote! { #output.push(#view); });
    }
    // 拼接全部有序语句。
    Ok(quote! { #(#statements)* })
}

// 生成一个非控制节点的 View 表达式，并向专用布局映射共享单子节点入口。
pub(super) fn generate_node_view(node: &Node) -> Result<TokenStream, Diagnostic> {
    // 按节点类型生成公开 View。
    match node {
        // 普通元素递归生成。
        Node::Element(element) => generate_view(element),
        // 非空直接文本生成 label。
        Node::Text(text) => {
            // 借用文本值用于字面量生成。
            let value = &text.value;
            // 返回文本 View。
            Ok(quote! { ::uix::prelude::label(#value) })
        }
        // 直接插值生成动态字符串 label。
        Node::Interpolation(expression) => {
            // 生成插值表达式。
            let value = generate_expression(&expression.expression, None)?;
            // 返回使用公开 ToString 的文本 View。
            Ok(quote! {
                // 把插值值转换为拥有所有权的文本。
                ::uix::prelude::label(::std::string::ToString::to_string(&(#value)))
            })
        }
    }
}

// 生成 If 或 For 控制流语句。
fn generate_control(
    // 接收控制元素。
    element: &Element,
    // 接收目标子节点向量。
    output: &Ident,
) -> Result<TokenStream, Diagnostic> {
    // 按控制绑定结构生成。
    match (&element.name[..], element.control.as_ref()) {
        // If 按条件决定是否追加子节点。
        ("If", Some(ControlBinding::If(condition))) => {
            // 生成条件表达式。
            let condition = generate_expression(&condition.expression, None)?;
            // 生成分支内有序子节点。
            let children = generate_child_statements(&element.children, output)?;
            // 返回不生成占位节点的条件分支。
            Ok(quote! {
                // 条件为真时才追加分支子节点。
                if #condition {
                    // 保持分支内部源码顺序。
                    #children
                }
            })
        }
        // For 按数据源生成重复子节点。
        (
            "For",
            Some(ControlBinding::For {
                binding,
                binding_span,
                index_binding,
                index_span,
                iterable,
                key,
            }),
        ) => generate_for(
            // 传递循环项绑定。
            binding,
            // 传递循环项跨度。
            *binding_span,
            // 传递可选索引绑定。
            index_binding.as_deref(),
            // 传递可选索引跨度。
            *index_span,
            // 传递数据源表达式。
            iterable,
            // 传递可选稳定 key。
            key.as_ref(),
            // 传递循环子节点。
            &element.children,
            // 传递目标向量。
            output,
            // 传递完整控制跨度。
            element.span,
        ),
        // 名称与控制绑定不一致表示内部结构损坏。
        _ => Err(Diagnostic::new(
            // 指向完整控制元素。
            element.span,
            // 说明控制结构不完整。
            format!("<{}> 缺少匹配的控制绑定", element.name),
            // 给出重新解析建议。
            "使用规范 If 或 For 语法重新声明控制元素",
        )),
    }
}

// 生成 For 循环与可选稳定 key。
#[allow(clippy::too_many_arguments)]
fn generate_for(
    // 接收循环项绑定名。
    binding: &str,
    // 接收循环项绑定跨度。
    binding_span: SourceSpan,
    // 接收可选索引绑定名。
    index_binding: Option<&str>,
    // 接收可选索引绑定跨度。
    index_span: Option<SourceSpan>,
    // 接收数据源表达式。
    iterable: &ExpressionNode,
    // 接收可选稳定 key 表达式。
    key: Option<&ExpressionNode>,
    // 接收循环子节点。
    children: &[Node],
    // 接收目标子节点向量。
    output: &Ident,
    // 接收完整 For 跨度。
    span: SourceSpan,
) -> Result<TokenStream, Diagnostic> {
    // 生成 Rust 循环项标识符。
    let binding = rust_identifier(binding, binding_span)?;
    // 生成可选 Rust 索引标识符。
    let index_binding = index_binding
        // 转换存在的索引名称。
        .map(|name| rust_identifier(name, index_span.unwrap_or(binding_span)))
        // 把 Option<Result> 转置为 Result<Option>。
        .transpose()?;
    // 生成数据源表达式。
    let iterable = generate_expression(&iterable.expression, None)?;
    // 每次生成拥有所有权的克隆项，避免借用逃逸到事件闭包。
    let iterator = quote! { ::std::iter::IntoIterator::into_iter((#iterable).clone()) };
    // 带 key 的 For 必须有一个稳定行根节点。
    let body = if let Some(key) = key {
        // 收集排除排版空白后的直接子节点。
        let renderable = children
            // 遍历子节点。
            .iter()
            // 排除空白文本。
            .filter(|node| is_renderable_node(node))
            // 收集借用。
            .collect::<Vec<_>>();
        // key 只能应用到唯一行根。
        if renderable.len() != 1 {
            // 返回稳定身份形状诊断。
            return Err(Diagnostic::new(
                // 指向完整 For。
                span,
                // 说明 key 需要唯一行根。
                "带 key 的 For 必须恰好生成一个直接子节点",
                // 给出规范结构。
                "用 Container 包裹多个行内节点，再把该 Container 作为 For 的唯一子节点",
            ));
        }
        // 生成唯一行根 View。
        let view = generate_node_view(renderable[0])?;
        // 生成 key 表达式。
        let key = generate_expression(&key.expression, None)?;
        // 返回带稳定身份的追加语句。
        quote! {
            // 生成当前循环行 View。
            let __uix_for_view = #view;
            // 把 key 转成公开 ViewNode 接受的字符串。
            #output.push(__uix_for_view.key(::std::format!("{}", #key)));
        }
    } else {
        // 无 key 时按位置追加全部循环子节点。
        generate_child_statements(children, output)?
    };
    // 有索引绑定时使用 enumerate 保持 usize 下标语义。
    if let Some(index_binding) = index_binding {
        // 返回带索引的循环。
        return Ok(quote! {
            // 克隆数据源并按位置枚举。
            for (#index_binding, #binding) in (#iterator).enumerate() {
                // 按源码顺序生成当前项节点。
                #body
            }
        });
    }
    // 无索引绑定时生成普通循环。
    Ok(quote! {
        // 克隆数据源并逐项迭代。
        for #binding in #iterator {
            // 按源码顺序生成当前项节点。
            #body
        }
    })
}

// 把文本与插值组合成一个拥有所有权的内容表达式。
pub(super) fn generate_text_content(
    // 接收有序文本子节点。
    children: &[Node],
    // 接收所属元素跨度。
    span: SourceSpan,
) -> Result<TokenStream, Diagnostic> {
    // 禁止文本型组件嵌套元素以免静默丢失结构。
    if children
        .iter()
        .any(|child| matches!(child, Node::Element(_)))
    {
        // 返回内容形状诊断。
        return Err(Diagnostic::new(
            // 指向所属文本型元素。
            span,
            // 说明当前公开构造器只接收文本。
            "当前 Text/Button/Typography 核心映射只接受文本与插值子节点",
            // 给出布局修复建议。
            "把图标或其他元素移到相邻 Container/Row 中",
        ));
    }
    // 纯静态文本直接合并为一个字面量。
    if children
        // 遍历全部子节点。
        .iter()
        // 确认没有插值。
        .all(|child| matches!(child, Node::Text(_)))
    {
        // 合并源码顺序中的全部文本片段。
        let value = children
            // 遍历文本片段。
            .iter()
            // 提取文本值。
            .filter_map(|child| match child {
                // 返回文本借用。
                Node::Text(text) => Some(text.value.as_str()),
                // 其他节点不应出现。
                _ => None,
            })
            // 收集为拥有所有权的字符串。
            .collect::<String>();
        // 返回静态字符串字面量。
        return Ok(quote! { #value });
    }
    // 创建卫生的动态文本缓冲区。
    let output = Ident::new("__uix_text", Span::mixed_site());
    // 保存有序文本追加语句。
    let mut statements = Vec::new();
    // 按源码顺序处理文本与插值。
    for child in children {
        // 文本片段直接追加。
        match child {
            // 追加静态文本。
            Node::Text(text) => {
                // 借用文本值。
                let value = &text.value;
                // 生成字符串追加。
                statements.push(quote! { #output.push_str(#value); });
            }
            // 插值先生成 Rust 表达式再转为文本。
            Node::Interpolation(expression) => {
                // 生成插值表达式。
                let value = generate_expression(&expression.expression, None)?;
                // 生成稳定 ToString 追加。
                statements.push(quote! {
                    // 把插值结果转换并追加到动态文本。
                    #output.push_str(&::std::string::ToString::to_string(&(#value)));
                });
            }
            // 元素已在函数开头拒绝。
            Node::Element(_) => {}
        }
    }
    // 返回构建动态文本的 Rust 块。
    Ok(quote! {{
        // 创建动态文本缓冲区。
        let mut #output = ::std::string::String::new();
        // 按源码顺序追加片段。
        #(#statements)*
        // 返回拥有所有权的内容。
        #output
    }})
}

// 判断节点是否会生成可见 View。
pub(super) fn is_renderable_node(node: &Node) -> bool {
    // 空白文本仅用于格式化源文件，不生成节点。
    !matches!(node, Node::Text(text) if text.value.trim().is_empty())
}
