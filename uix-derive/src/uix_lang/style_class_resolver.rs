// 引入确定性名称映射。
use std::collections::BTreeMap;

// 引入样式类、元素与诊断 AST。
use super::{
    Attribute, AttributeValue, Declaration, Diagnostic, Document, Element, StyleClassDeclaration,
    StyleProperty, StylePseudoState,
};

// 保存已经展开继承的样式类注册表。
pub(crate) struct StyleClassResolver {
    // 按名称保存最终属性列表。
    classes: BTreeMap<String, Vec<StyleProperty>>,
    // 按基础类名与状态保存只含差异字段的伪类注册表。
    variants: BTreeMap<String, BTreeMap<StylePseudoState, Vec<StyleProperty>>>,
}

// 保存一个元素按 class 顺序合并后的状态差异字段。
#[derive(Default)]
pub(crate) struct ResolvedPseudoStyles {
    // 保存悬停状态差异字段。
    pub(crate) hover: Vec<StyleProperty>,
    // 保存禁用状态差异字段。
    pub(crate) disabled: Vec<StyleProperty>,
    // 保存勾选状态差异字段。
    pub(crate) checked: Vec<StyleProperty>,
}

// 实现伪类集合查询。
impl ResolvedPseudoStyles {
    // 判断元素是否没有任何状态差异。
    pub(crate) fn is_empty(&self) -> bool {
        // 三个闭合状态都为空才表示没有伪类。
        self.hover.is_empty() && self.disabled.is_empty() && self.checked.is_empty()
    }
}

// 实现文档级样式类校验与元素改写。
impl StyleClassResolver {
    // 从顶层声明构造完整样式类注册表。
    pub(super) fn new(document: &Document) -> Result<Self, Diagnostic> {
        // 收集全部已通过名称去重的样式类声明。
        let declarations = document
            // 遍历顶层声明。
            .declarations
            // 借用声明迭代器。
            .iter()
            // 只保留样式类。
            .filter_map(|declaration| match declaration {
                // 基础类进入继承解析表。
                Declaration::StyleClass(style) if style.state.is_none() => {
                    Some((style.name.clone(), style.clone()))
                }
                // 忽略其他声明类别。
                _ => None,
            })
            // 收集到确定性映射。
            .collect::<BTreeMap<_, _>>();
        // 保存已经完成继承展开的缓存。
        let mut classes = BTreeMap::new();
        // 按名称顺序验证并展开每一个样式类。
        for name in declarations.keys() {
            // 为当前解析创建空递归栈。
            let mut stack = Vec::new();
            // 展开并缓存当前样式类。
            resolve_class(name, &declarations, &mut classes, &mut stack)?;
        }
        // 保存按基础类和状态组织的差异字段。
        let mut variants =
            BTreeMap::<String, BTreeMap<StylePseudoState, Vec<StyleProperty>>>::new();
        // 登记全部状态变体并验证同前缀基础类存在。
        for style in document
            .declarations
            .iter()
            .filter_map(|declaration| match declaration {
                // 只选择状态样式声明。
                Declaration::StyleClass(style) if style.state.is_some() => Some(style),
                // 忽略基础类与其他声明。
                _ => None,
            })
        {
            // 状态变体必须有同名基础类作为隐含父级。
            if !classes.contains_key(&style.name) {
                // 返回未声明基础类诊断。
                return Err(Diagnostic::new(
                    style.span,
                    format!("状态伪类 {} 缺少同前缀基础类", style.name),
                    format!("先声明 {} {{ ... }}", style.name),
                ));
            }
            // 解析器已保证状态存在且同状态不重复。
            variants.entry(style.name.clone()).or_default().insert(
                style.state.expect("状态声明必须携带状态"),
                style.properties.clone(),
            );
        }
        // 返回完成验证的注册表。
        Ok(Self { classes, variants })
    }

    // 解析元素 class 引用对应的全部状态差异。
    pub(super) fn pseudo_styles(
        &self,
        element: &Element,
    ) -> Result<ResolvedPseudoStyles, Diagnostic> {
        // 创建空的状态合并结果。
        let mut resolved = ResolvedPseudoStyles::default();
        // 按属性源码顺序查找 class。
        for attribute in &element.attributes {
            // 只处理 class 属性。
            if attribute.name != "class" {
                // 继续下一属性。
                continue;
            }
            // 状态类与普通类共享静态 class 契约。
            let AttributeValue::Literal(source) = &attribute.value else {
                // 返回动态类名诊断。
                return Err(Diagnostic::new(
                    attribute.span,
                    "状态伪类只支持静态 class 名称",
                    "使用 class=\"baseButton\"，自定义动态切换继续使用 setStyle",
                ));
            };
            // 多个类按从左到右顺序覆盖同状态字段。
            for name in source.split_whitespace() {
                // 没有状态变体的普通类无需处理。
                let Some(states) = self.variants.get(name) else {
                    // 继续下一类名。
                    continue;
                };
                // 合并 hover 差异。
                if let Some(properties) = states.get(&StylePseudoState::Hover) {
                    // 后出现类覆盖先出现类的同名字段。
                    merge_properties(&mut resolved.hover, properties.clone());
                }
                // 合并 disabled 差异。
                if let Some(properties) = states.get(&StylePseudoState::Disabled) {
                    // 后出现类覆盖先出现类的同名字段。
                    merge_properties(&mut resolved.disabled, properties.clone());
                }
                // 合并 checked 差异。
                if let Some(properties) = states.get(&StylePseudoState::Checked) {
                    // 后出现类覆盖先出现类的同名字段。
                    merge_properties(&mut resolved.checked, properties.clone());
                }
            }
        }
        // 返回闭合状态集合。
        Ok(resolved)
    }

    // 判断节点树是否使用需要私有 hover 状态的伪类。
    pub(super) fn nodes_use_hover(&self, nodes: &[super::Node]) -> bool {
        // 任一元素自身或后代引用 hover 变体即需要组件状态作用域。
        nodes.iter().any(|node| match node {
            // 元素递归检查自身与后代。
            super::Node::Element(element) => {
                self.element_uses_hover(element) || self.nodes_use_hover(&element.children)
            }
            // 文本和插值没有 class。
            _ => false,
        })
    }

    // 判断单个元素是否引用已登记 hover 变体。
    fn element_uses_hover(&self, element: &Element) -> bool {
        // 查找静态 class 中任一具名 hover 变体。
        element.attributes.iter().any(|attribute| {
            // 只匹配静态 class。
            attribute.name == "class"
                && match &attribute.value {
                    // 检查空白分隔的每个类名。
                    AttributeValue::Literal(source) => source.split_whitespace().any(|name| {
                        // 查找类状态表中的 hover 项。
                        self.variants
                            .get(name)
                            .is_some_and(|states| states.contains_key(&StylePseudoState::Hover))
                    }),
                    // 动态 class 由后续诊断负责。
                    _ => false,
                }
        })
    }

    // 把元素的 class 与内联 style 合并为单一结构化 style。
    pub(super) fn apply(&self, element: &mut Element) -> Result<(), Diagnostic> {
        // 保存按优先级合并的属性。
        let mut properties = Vec::new();
        // 保存最终合成属性使用的源码跨度。
        let mut style_span = element.span;
        // 保存内联样式以便最后覆盖样式类。
        let mut inline = None;
        // 保存不属于 class/style 的原始属性。
        let mut retained = Vec::new();
        // 依次消费元素属性。
        for attribute in element.attributes.drain(..) {
            // 样式类先按声明顺序合并。
            if attribute.name == "class" {
                // 更新合成属性跨度。
                style_span = attribute.span;
                // 展开 class 引用。
                self.apply_class_attribute(&mut properties, &attribute)?;
                // class 已转换，不保留原属性。
                continue;
            }
            // 内联样式留到样式类之后应用。
            if attribute.name == "style" {
                // 更新合成属性跨度。
                style_span = attribute.span;
                // 只接受解析器生成的结构化内联样式。
                let AttributeValue::InlineStyle(values) = attribute.value else {
                    // 返回内部形状保护诊断。
                    return Err(Diagnostic::new(
                        // 指向完整 style 属性。
                        attribute.span,
                        // 说明值形状错误。
                        "style 必须是结构化内联样式",
                        // 给出规范写法。
                        "使用 style=\"color: #fff;\"",
                    ));
                };
                // 保存内联属性与跨度。
                inline = Some(values);
                // 原 style 稍后替换为合并结果。
                continue;
            }
            // 其他属性保持原顺序。
            retained.push(attribute);
        }
        // 内联样式最后覆盖同名样式类属性。
        if let Some(inline) = inline {
            // 合并全部内联属性。
            merge_properties(&mut properties, inline);
        }
        // 没有 class/style 时不合成空属性。
        if !properties.is_empty() {
            // 把合并结果放在普通属性之后，确保样式优先级稳定。
            retained.push(Attribute {
                // 使用公开代码生成器识别的属性名。
                name: "style".to_string(),
                // 保存结构化属性列表。
                value: AttributeValue::InlineStyle(properties),
                // 沿用最近的用户声明跨度。
                span: style_span,
            });
        }
        // 写回已经移除 class 的属性列表。
        element.attributes = retained;
        // 报告改写成功。
        Ok(())
    }

    // 把单个 class 属性展开到目标属性列表。
    fn apply_class_attribute(
        // 借用解析器状态。
        &self,
        // 接收目标属性列表。
        target: &mut Vec<StyleProperty>,
        // 接收 class 属性。
        attribute: &Attribute,
    ) -> Result<(), Diagnostic> {
        // class 必须是编译期可确定的字面量。
        let AttributeValue::Literal(source) = &attribute.value else {
            // 返回动态 class 诊断。
            return Err(Diagnostic::new(
                // 指向完整 class 属性。
                attribute.span,
                // 说明无法在编译期选择样式类。
                "class 必须使用编译期字符串字面量",
                // 给出规范写法。
                "使用 class=\"baseButton\"",
            ));
        };
        // 至少需要一个类名。
        if source.split_whitespace().next().is_none() {
            // 返回空 class 诊断。
            return Err(Diagnostic::new(
                // 指向完整 class 属性。
                attribute.span,
                // 说明类名为空。
                "class 不能为空",
                // 给出删除或填写建议。
                "删除 class 属性，或填写已声明的样式类名",
            ));
        }
        // 多个类按从左到右顺序覆盖。
        for name in source.split_whitespace() {
            // 查找已经展开继承的类。
            let properties = self.classes.get(name).ok_or_else(|| {
                // 构造未知类诊断。
                Diagnostic::new(
                    // 指向 class 使用位置。
                    attribute.span,
                    // 说明缺失名称。
                    format!("class 引用了未声明的样式类 {name}"),
                    // 给出声明修复动作。
                    format!("在根元素前声明 {name} {{ ... }}，或改用现有类名"),
                )
            })?;
            // 当前类覆盖之前类的同名属性。
            merge_properties(target, properties.clone());
        }
        // 报告 class 展开成功。
        Ok(())
    }
}

// 递归展开一个样式类的 extends 链。
fn resolve_class(
    // 接收待展开类名。
    name: &str,
    // 接收原始声明表。
    declarations: &BTreeMap<String, StyleClassDeclaration>,
    // 接收已展开缓存。
    resolved: &mut BTreeMap<String, Vec<StyleProperty>>,
    // 接收当前递归栈。
    stack: &mut Vec<String>,
) -> Result<Vec<StyleProperty>, Diagnostic> {
    // 已缓存类直接返回副本。
    if let Some(properties) = resolved.get(name) {
        // 返回缓存结果。
        return Ok(properties.clone());
    }
    // 获取原始声明。
    let declaration = declarations
        // 查找类名。
        .get(name)
        // 顶层调用已保证名称存在。
        .expect("样式类名称来自声明表")
        // 克隆以避免递归期间借用冲突。
        .clone();
    // 重复进入栈表示继承环。
    if let Some(start) = stack.iter().position(|entry| entry == name) {
        // 构造完整环路径。
        let mut cycle = stack[start..].to_vec();
        // 闭合环路径。
        cycle.push(name.to_string());
        // 返回循环继承诊断。
        return Err(Diagnostic::new(
            // 指向当前类声明。
            declaration.span,
            // 说明环路径。
            format!("样式类继承形成循环：{}", cycle.join(" -> ")),
            // 给出打断继承建议。
            "删除循环中的一个 extends，保持继承关系为无环图",
        ));
    }
    // 把当前类压入递归栈。
    stack.push(name.to_string());
    // 先展开可选父类。
    let mut properties = if let Some(parent) = &declaration.extends {
        // 父类必须存在。
        if !declarations.contains_key(parent) {
            // 返回未知父类诊断。
            return Err(Diagnostic::new(
                // 指向当前类声明。
                declaration.span,
                // 说明缺失父类。
                format!("样式类 {name} 继承了未声明的样式类 {parent}"),
                // 给出声明或改名建议。
                format!("先声明 {parent} {{ ... }}，或修正 extends 名称"),
            ));
        }
        // 递归取得父类最终属性。
        resolve_class(parent, declarations, resolved, stack)?
    } else {
        // 无父类从空属性开始。
        Vec::new()
    };
    // 子类属性覆盖父类同名属性。
    merge_properties(&mut properties, declaration.properties);
    // 当前类展开完成，弹出递归栈。
    let popped = stack.pop();
    // 调试保护栈平衡。
    debug_assert_eq!(popped.as_deref(), Some(name));
    // 缓存最终属性。
    resolved.insert(name.to_string(), properties.clone());
    // 返回最终属性。
    Ok(properties)
}

// 按后者优先合并样式属性。
fn merge_properties(target: &mut Vec<StyleProperty>, incoming: Vec<StyleProperty>) {
    // 按声明顺序处理覆盖属性。
    for property in incoming {
        // 删除较低优先级的同名属性。
        target.retain(|current| current.name != property.name);
        // 把高优先级属性放到末尾。
        target.push(property);
    }
}
