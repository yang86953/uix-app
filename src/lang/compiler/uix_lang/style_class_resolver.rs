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
    // 按名称保存直接父类，用于展开元素 class 的完整继承链。
    parents: BTreeMap<String, Option<String>>,
    // 按基础类名与状态保存只含差异字段的伪类注册表。
    variants: BTreeMap<String, BTreeMap<StylePseudoState, Vec<StyleProperty>>>,
    // 按源码顺序保存 @media 条件块及其基础类覆盖。
    media: Vec<MediaBlock>,
}

// 一个 @media 块解析后的类覆盖表；条件已标注到每个属性的 media 字段。
struct MediaBlock {
    // 按基础类名保存已标注条件的覆盖属性。
    overrides: BTreeMap<String, Vec<StyleProperty>>,
}

// 保存一个元素按 class 顺序合并后的状态差异字段。
#[derive(Default)]
pub(crate) struct ResolvedPseudoStyles {
    // 保存悬停状态差异字段。
    pub(crate) hover: Vec<StyleProperty>,
    // 保存焦点状态差异字段。
    pub(crate) focus: Vec<StyleProperty>,
    // 保存键盘焦点可见状态差异字段。
    pub(crate) focus_visible: Vec<StyleProperty>,
    // 保存按压状态差异字段。
    pub(crate) active: Vec<StyleProperty>,
    // 保存禁用状态差异字段。
    pub(crate) disabled: Vec<StyleProperty>,
    // 保存勾选状态差异字段。
    pub(crate) checked: Vec<StyleProperty>,
}

// 实现伪类集合查询。
impl ResolvedPseudoStyles {
    // 判断元素是否没有任何状态差异。
    pub(crate) fn is_empty(&self) -> bool {
        // 六个闭合状态都为空才表示没有伪类。
        self.hover.is_empty()
            && self.focus.is_empty()
            && self.focus_visible.is_empty()
            && self.active.is_empty()
            && self.disabled.is_empty()
            && self.checked.is_empty()
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
        // 记录每个基础类的直接父类，供媒体层沿继承链匹配。
        let parents = declarations
            .iter()
            .map(|(name, style)| (name.clone(), style.extends.clone()))
            .collect();
        // 登记全部 @media 块并验证覆盖目标是已声明的基础类。
        let mut media = Vec::new();
        for block in document
            .declarations
            .iter()
            .filter_map(|declaration| match declaration {
                Declaration::Media(block) => Some(block),
                _ => None,
            })
        {
            let mut overrides = BTreeMap::<String, Vec<StyleProperty>>::new();
            for class in &block.overrides {
                if !classes.contains_key(&class.name) {
                    return Err(Diagnostic::new(
                        class.span,
                        format!("@media 覆盖了未声明的样式类 {}", class.name),
                        format!("先在顶层声明 {} {{ ... }}，再在 @media 内覆盖", class.name),
                    ));
                }
                if overrides.contains_key(&class.name) {
                    return Err(Diagnostic::new(
                        class.span,
                        format!("同一 @media 块内重复覆盖样式类 {}", class.name),
                        "合并同一块内的同名覆盖",
                    ));
                }
                let properties = class
                    .properties
                    .iter()
                    .cloned()
                    .map(|mut property| {
                        property.media = Some(block.query.clone());
                        property
                    })
                    .collect();
                overrides.insert(class.name.clone(), properties);
            }
            media.push(MediaBlock { overrides });
        }
        // 返回完成验证的注册表。
        Ok(Self {
            classes,
            parents,
            variants,
            media,
        })
    }

    // 返回元素 class 列表展开继承后的完整链：每个类先祖先后自身，保持左到右顺序。
    fn class_chain(&self, names: &str) -> Vec<String> {
        let mut chain = Vec::new();
        for name in names.split_whitespace() {
            let mut lineage = Vec::new();
            let mut current = Some(name.to_string());
            while let Some(class) = current {
                if lineage.contains(&class) {
                    break;
                }
                current = self.parents.get(&class).cloned().flatten();
                lineage.push(class);
            }
            lineage.reverse();
            for class in lineage {
                if !chain.contains(&class) {
                    chain.push(class);
                }
            }
        }
        chain
    }

    // 在基础类之后、内联之前叠加匹配元素继承链的 @media 覆盖；
    // 块按源码顺序，块内按元素 class 链顺序。
    fn apply_media_layers(&self, target: &mut Vec<StyleProperty>, class_names: &[String]) {
        if self.media.is_empty() {
            return;
        }
        let chain: Vec<String> = class_names
            .iter()
            .flat_map(|names| self.class_chain(names))
            .fold(Vec::new(), |mut chain, class| {
                if !chain.contains(&class) {
                    chain.push(class);
                }
                chain
            });
        for block in &self.media {
            for class in &chain {
                if let Some(properties) = block.overrides.get(class) {
                    merge_properties(target, properties.clone());
                }
            }
        }
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
                // 合并 focus 差异。
                if let Some(properties) = states.get(&StylePseudoState::Focus) {
                    // 后出现类覆盖先出现类的同名字段。
                    merge_properties(&mut resolved.focus, properties.clone());
                }
                // 合并 focus-visible 差异。
                if let Some(properties) = states.get(&StylePseudoState::FocusVisible) {
                    // 后出现类覆盖先出现类的同名字段。
                    merge_properties(&mut resolved.focus_visible, properties.clone());
                }
                // 合并 active 差异。
                if let Some(properties) = states.get(&StylePseudoState::Active) {
                    // 后出现类覆盖先出现类的同名字段。
                    merge_properties(&mut resolved.active, properties.clone());
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

    // 判断节点树是否使用需要私有自动事实状态的伪类。
    //
    // hover、focus、focus-visible 与 active 的事实由生成的指针/焦点
    // 监听器维护，需要组件私有状态作用域跨 reconcile 持有；
    // checked 与 disabled 读取元素既有属性事实，不需要作用域。
    pub(super) fn nodes_use_auto_state_facts(&self, nodes: &[super::Node]) -> bool {
        // 任一元素自身或后代引用自动事实变体即需要组件状态作用域。
        nodes.iter().any(|node| match node {
            // 元素递归检查自身与后代。
            super::Node::Element(element) => {
                self.element_uses_auto_state_facts(element)
                    || self.nodes_use_auto_state_facts(&element.children)
            }
            // 文本和插值没有 class。
            _ => false,
        })
    }

    // 判断节点树是否使用需要持久化 Animated 状态的 animation 属性。
    pub(super) fn nodes_use_animation(&self, nodes: &[super::Node]) -> bool {
        // 任一元素自身或后代声明 animation 即需要组件状态作用域。
        nodes.iter().any(|node| match node {
            // 元素递归检查自身与后代。
            super::Node::Element(element) => {
                // 合并当前元素与后代结果。
                self.element_uses_animation(element) || self.nodes_use_animation(&element.children)
            }
            // 文本和插值没有样式属性。
            _ => false,
        })
    }

    // 判断节点树是否使用需要持久化目标快照的 transition 属性。
    pub(super) fn nodes_use_transition(&self, nodes: &[super::Node]) -> bool {
        // 任一元素自身或后代声明 transition 即需要组件状态作用域。
        nodes.iter().any(|node| match node {
            // 元素递归检查自身与后代。
            super::Node::Element(element) => {
                // 合并当前元素与后代结果。
                self.element_uses_transition(element)
                    || self.nodes_use_transition(&element.children)
            }
            // 文本和插值没有样式属性。
            _ => false,
        })
    }

    // 判断单个元素的静态 class 或内联 style 是否声明 animation。
    fn element_uses_animation(&self, element: &Element) -> bool {
        // 任一静态样式来源包含 animation 即返回真。
        element.attributes.iter().any(|attribute| {
            // 按属性种类检查类注册表或内联属性。
            match (attribute.name.as_str(), &attribute.value) {
                // 静态 class 按空白分隔名称查找已解析属性。
                ("class", AttributeValue::Literal(source)) => source
                    // 遍历 class 名称。
                    .split_whitespace()
                    // 任一类声明 animation 即命中。
                    .any(|name| {
                        // 查找继承展开后的最终属性。
                        self.classes.get(name).is_some_and(|properties| {
                            // 检查最终级联属性名。
                            properties
                                .iter()
                                .any(|property| property.name == "animation")
                        })
                    }),
                // 内联样式直接检查结构化属性。
                ("style", AttributeValue::InlineStyle(properties)) => properties
                    // 遍历显式内联字段。
                    .iter()
                    // 查找 animation 简写。
                    .any(|property| property.name == "animation"),
                // 其他属性不影响动画作用域。
                _ => false,
            }
        })
    }

    // 判断单个元素的静态 class 或内联 style 是否声明 transition。
    fn element_uses_transition(&self, element: &Element) -> bool {
        // 任一静态样式来源包含 transition 即返回真。
        element.attributes.iter().any(|attribute| {
            // 按属性种类检查类注册表或内联属性。
            match (attribute.name.as_str(), &attribute.value) {
                // 静态 class 按空白分隔名称查找已解析属性。
                ("class", AttributeValue::Literal(source)) => source
                    // 遍历 class 名称。
                    .split_whitespace()
                    // 任一类声明 transition 即命中。
                    .any(|name| {
                        // 查找继承展开后的最终属性。
                        self.classes.get(name).is_some_and(|properties| {
                            // 检查最终级联属性名。
                            properties
                                .iter()
                                .any(|property| property.name == "transition")
                        })
                    }),
                // 内联样式直接检查结构化属性。
                ("style", AttributeValue::InlineStyle(properties)) => properties
                    // 遍历显式内联字段。
                    .iter()
                    // 查找 transition 简写。
                    .any(|property| property.name == "transition"),
                // 其他属性不影响过渡作用域。
                _ => false,
            }
        })
    }

    // 判断单个元素是否引用已登记的自动事实状态变体。
    fn element_uses_auto_state_facts(&self, element: &Element) -> bool {
        // 查找静态 class 中任一自动事实状态变体。
        element.attributes.iter().any(|attribute| {
            // 只匹配静态 class。
            attribute.name == "class"
                && match &attribute.value {
                    // 检查空白分隔的每个类名。
                    AttributeValue::Literal(source) => source.split_whitespace().any(|name| {
                        // 查找类状态表中的自动事实状态项。
                        self.variants.get(name).is_some_and(|states| {
                            states.contains_key(&StylePseudoState::Hover)
                                || states.contains_key(&StylePseudoState::Focus)
                                || states.contains_key(&StylePseudoState::FocusVisible)
                                || states.contains_key(&StylePseudoState::Active)
                        })
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
        // 保存元素引用的 class 文本，供媒体层沿继承链匹配。
        let mut class_names = Vec::new();
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
                if let AttributeValue::Literal(source) = &attribute.value {
                    class_names.push(source.clone());
                }
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
        // 媒体条件层在全部基础类之后、内联之前叠加。
        self.apply_media_layers(&mut properties, &class_names);
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

// 按后者优先合并样式属性；同名但条件不同的声明并存，由运行期条件决定生效者。
fn merge_properties(target: &mut Vec<StyleProperty>, incoming: Vec<StyleProperty>) {
    // 按声明顺序处理覆盖属性。
    for property in incoming {
        // 删除较低优先级的同名同条件属性。
        target.retain(|current| current.name != property.name || current.media != property.media);
        // 把高优先级属性放到末尾。
        target.push(property);
    }
}
