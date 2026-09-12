// 引入有序集合以保持 `all` 字段生成确定。
use std::collections::BTreeSet;

// 引入组件展开器。
use super::widget_codegen::WidgetExpander;
// 引入动画值与样式颜色验证入口。
use super::style_value_codegen::{animation_f32_value, color_value};
// 引入 transition 降低所需 AST 与诊断。
use super::{
    AnimationEasing, AnimationPropertyKind, AttributeValue, Diagnostic, DynamicStyleBinding,
    Element, StyleProperty, TransitionBinding, WidgetScopeMarker,
};

// 实现 transition 简写消费、状态分支矩阵验证与最终装饰附加。
impl WidgetExpander {
    // 从最终基础样式或动态完整分支提取 transition。
    pub(super) fn prepare_transition(
        // 可变借用展开器以读取组件作用域和 For 实例路径。
        &mut self,
        // 接收已经完成基础、动态与伪类降低的元素。
        element: &mut Element,
    ) -> Result<(), Diagnostic> {
        // 保存基础 transition 简写。
        let mut transition = None;
        // 保存普通静态路径的最终基础字段。
        let mut static_base = Vec::new();
        // 静态路径从唯一结构化 style 中移除 transition。
        for attribute in &mut element.attributes {
            // 只处理合并后的 style 属性。
            if attribute.name != "style" {
                // 继续下一属性。
                continue;
            }
            // 样式合并器保证结构化字段列表。
            let AttributeValue::InlineStyle(properties) = &mut attribute.value else {
                // 返回内部形状保护诊断。
                return Err(Diagnostic::new(
                    // 指向完整 style 属性。
                    attribute.span,
                    // 陈述失败原因。
                    "transition 降低要求结构化 style 属性",
                    // 给出规范入口。
                    "使用 UIX 样式类或 style=\"transition: ...;\"",
                ));
            };
            // 定位基础简写。
            if let Some(index) = properties
                // 遍历最终级联字段。
                .iter()
                // 查找 transition。
                .position(|property| property.name == "transition")
            {
                // 从普通 Style 生成前消费简写。
                transition = Some(properties.remove(index));
            }
            // 复制剩余基础字段供矩阵与初值验证。
            static_base = properties.clone();
            // style 属性唯一，完成后退出。
            break;
        }
        // 保存动态样式原始分支与全部完整目标分支。
        let mut dynamic_branches = Vec::new();
        // 检查并消费动态分支里的 transition。
        for marker in &mut element.widget_scopes {
            // 只处理 setStyle 动态元数据。
            let WidgetScopeMarker::DynamicStyle(binding) = marker else {
                // 继续下一装饰。
                continue;
            };
            // 从原始和目标分支移除 transition，避免普通 Style 生成误报。
            let branch_transitions = take_dynamic_transitions(binding);
            // 原始分支是唯一允许定义播放配置的动态分支。
            let original = branch_transitions.first().cloned().flatten();
            // 静态路径与动态路径不得同时形成两个基础简写。
            if transition.is_none() {
                // 使用动态原始分支简写。
                transition = original.clone();
            }
            // 目标类只允许携带与基础完全相同的内联简写副本。
            for target in branch_transitions.into_iter().skip(1).flatten() {
                // 缺少基础简写或值不同都会改变播放器配置所有权。
                if original
                    // 比较规范值源码。
                    .as_ref()
                    // 相同值表示由内联样式复制到全部动态分支。
                    .is_none_or(|base| base.value.source != target.value.source)
                {
                    // 返回配置所有权诊断。
                    return Err(transition_diagnostic(
                        // 指向目标类中的 transition。
                        &target,
                        // 陈述失败原因。
                        "setStyle 目标类不能改变 transition 播放配置",
                        // 给出稳定配置写法。
                        "把 transition 放到原始基础类或共享内联 style",
                    ));
                }
            }
            // 复制完成消费的动态完整分支。
            dynamic_branches = binding
                // 遍历所有闭合分支。
                .variants
                // 借用迭代器。
                .iter()
                // 复制每个完整字段集合。
                .map(|variant| variant.properties.clone())
                // 收集供后续验证。
                .collect();
        }
        // 伪类差异不得改变 transition 配置本身。
        for marker in &element.widget_scopes {
            // 只检查伪类叠加元数据。
            let WidgetScopeMarker::PseudoStyle(binding) = marker else {
                // 继续下一装饰。
                continue;
            };
            // 串接三个状态差异列表。
            let property = binding
                // 借用 hover 差异。
                .hover
                // 建立迭代器。
                .iter()
                // 串接 disabled 差异。
                .chain(
                    binding
                        // 借用可选 disabled 元组。
                        .disabled
                        // 只取字段列表。
                        .as_ref()
                        // 转为切片。
                        .map_or(&[][..], |(_, properties)| properties.as_slice()),
                )
                // 串接 checked 差异。
                .chain(
                    binding
                        // 借用可选 checked 元组。
                        .checked
                        // 只取字段列表。
                        .as_ref()
                        // 转为切片。
                        .map_or(&[][..], |(_, properties)| properties.as_slice()),
                )
                // 定位 transition 简写。
                .find(|property| property.name == "transition");
            // 状态分支中出现配置时返回定向诊断。
            if let Some(property) = property {
                // 状态进入/离开必须共享基础配置。
                return Err(transition_diagnostic(
                    // 指向状态分支配置。
                    property,
                    // 陈述失败原因。
                    "状态伪类不能改变 transition 播放配置",
                    // 给出基础声明写法。
                    "把 transition 放到同名基础样式类",
                ));
            }
        }
        // 没有 transition 时保持元素不变。
        let Some(transition) = transition else {
            // 报告无需附加装饰。
            return Ok(());
        };
        // animation 与 transition 同时拥有同一节点 Animated 值会形成竞争播放器。
        if element
            // 遍历既有装饰。
            .widget_scopes
            // 借用迭代器。
            .iter()
            // 查找 animation 绑定。
            .any(|marker| matches!(marker, WidgetScopeMarker::Animation(_)))
        {
            // 返回明确组合诊断。
            return Err(transition_diagnostic(
                // 指向 transition 简写。
                &transition,
                // 陈述失败原因。
                "同一节点不能同时声明 animation 与 transition",
                // 给出拆分建议。
                "把关键帧动画与状态过渡放到不同节点",
            ));
        }
        // 解析固定顺序简写。
        let (selection, duration_micros, easing, delay_micros) =
            parse_transition_shorthand(&transition)?;
        // 动态路径使用原始完整分支作为首次基础，静态路径使用合并 style。
        let base = dynamic_branches
            // 读取原始动态分支。
            .first()
            // 动态分支存在时优先使用。
            .cloned()
            // 否则使用静态基础字段。
            .unwrap_or(static_base);
        // 收集基础、动态和伪类中出现的全部目标字段值。
        let mut candidate_sets = vec![base.clone()];
        // 追加动态完整分支。
        candidate_sets.extend(dynamic_branches.iter().skip(1).cloned());
        // 追加伪类差异字段。
        append_pseudo_candidates(&element.widget_scopes, &mut candidate_sets);
        // 解析 `all` 或单一具体属性的实际字段集合。
        let properties = resolve_transition_properties(selection, &candidate_sets, &transition)?;
        // 验证基础值、动态完整分支和全部显式目标值。
        validate_transition_values(
            // 传递实际字段集合。
            &properties,
            // 传递初始基础字段。
            &base,
            // 传递动态完整分支。
            &dynamic_branches,
            // 传递全部候选值。
            &candidate_sets,
            // 传递简写供缺失值诊断。
            &transition,
        )?;
        // transition 必须归属最近组件或文档根状态作用域。
        let Some(widget_scope) = self.widget_scope_stack.last().cloned() else {
            // 返回生命周期所有者诊断。
            return Err(transition_diagnostic(
                // 指向 transition 简写。
                &transition,
                // 陈述失败原因。
                "transition 缺少声明式生命周期所有者",
                // 给出合法使用边界。
                "从 uix! 或 uix_app! 文档根展开该 View，或把它放入 Widget",
            ));
        };
        // 使用节点类型与源码位置形成跨构建稳定声明身份。
        let declaration_id = Self::stable_widget_id(&format!(
            // 固定 transition 身份前缀。
            "transition:{}:{}:{}",
            // 纳入节点类型。
            element.name,
            // 纳入源码开始位置。
            element.span.start,
            // 纳入源码结束位置。
            element.span.end
        ));
        // transition 最后包裹全部动态与伪类目标样式。
        element
            // 借用最终装饰列表。
            .widget_scopes
            // 追加目标比较绑定。
            .push(WidgetScopeMarker::Transition(TransitionBinding {
                // 保存最近生命周期作用域。
                widget_scope_name: widget_scope.to_string(),
                // 保存稳定声明身份。
                declaration_id,
                // 继承最近 For 实际路径。
                instance_path_name: self.for_path_stack.last().map(ToString::to_string),
                // 保存时长。
                duration_micros,
                // 保存延迟。
                delay_micros,
                // 保存缓动。
                easing,
                // 保存闭合字段集合。
                properties,
            }));
        // 报告降低成功。
        Ok(())
    }
}

// 从动态完整分支移除 transition 并按分支顺序返回简写。
fn take_dynamic_transitions(
    // 可变借用动态绑定。
    binding: &mut DynamicStyleBinding,
) -> Vec<Option<StyleProperty>> {
    // 每个分支最多保留一个最终级联 transition。
    binding
        // 可变遍历闭合分支。
        .variants
        // 借用迭代器。
        .iter_mut()
        // 从字段列表取出简写。
        .map(|variant| {
            // 定位 transition 字段。
            variant
                // 可变借用完整字段。
                .properties
                // 查找字段位置。
                .iter()
                // 定位名称。
                .position(|property| property.name == "transition")
                // 存在时从普通 Style 生成前移除。
                .map(|index| variant.properties.remove(index))
        })
        // 收集与分支一一对应的结果。
        .collect()
}

// 把伪类差异字段复制到值验证候选集合。
fn append_pseudo_candidates(
    // 借用全部元素装饰。
    markers: &[WidgetScopeMarker],
    // 可变借用候选字段集合。
    candidates: &mut Vec<Vec<StyleProperty>>,
) {
    // 遍历装饰查找伪类元数据。
    for marker in markers {
        // 只处理伪类装饰。
        let WidgetScopeMarker::PseudoStyle(binding) = marker else {
            // 继续下一装饰。
            continue;
        };
        // 追加 hover 差异。
        candidates.push(binding.hover.clone());
        // 追加可选 disabled 差异。
        if let Some((_, properties)) = &binding.disabled {
            // 复制字段供独立验证。
            candidates.push(properties.clone());
        }
        // 追加可选 checked 差异。
        if let Some((_, properties)) = &binding.checked {
            // 复制字段供独立验证。
            candidates.push(properties.clone());
        }
    }
}

// 解析 all 或单一具体属性的实际闭合集合。
fn resolve_transition_properties(
    // 接收 None 表示 all，Some 表示具体字段。
    selection: Option<AnimationPropertyKind>,
    // 借用所有可能目标字段。
    candidates: &[Vec<StyleProperty>],
    // 借用简写供空集合诊断。
    transition: &StyleProperty,
) -> Result<Vec<AnimationPropertyKind>, Diagnostic> {
    // 具体字段直接形成单元素集合。
    if let Some(property) = selection {
        // 返回闭合字段。
        return Ok(vec![property]);
    }
    // all 只收集首批可插值矩阵内实际出现的字段。
    let properties = candidates
        // 遍历全部基础与状态字段集合。
        .iter()
        // 展平字段。
        .flat_map(|properties| properties.iter())
        // 把已支持名称映射为闭合枚举。
        .filter_map(|property| transition_property_kind(&property.name))
        // 收集到有序集合并去重。
        .collect::<BTreeSet<_>>()
        // 转回稳定向量。
        .into_iter()
        // 收集所有字段。
        .collect::<Vec<_>>();
    // all 没有任何首批字段时不得伪装成有效过渡。
    if properties.is_empty() {
        // 返回支持矩阵诊断。
        return Err(transition_diagnostic(
            // 指向完整简写。
            transition,
            // 陈述失败原因。
            "transition all 没有可过渡的已支持字段",
            // 给出首批矩阵。
            "声明 width、height、borderRadius、opacity、color 或 backgroundColor 的状态变化",
        ));
    }
    // 返回实际字段集合。
    Ok(properties)
}

// 验证首次基础、动态完整分支与全部显式目标值。
fn validate_transition_values(
    // 借用实际字段集合。
    selected: &[AnimationPropertyKind],
    // 借用首次基础字段。
    base: &[StyleProperty],
    // 借用动态完整分支。
    dynamic_branches: &[Vec<StyleProperty>],
    // 借用所有候选字段。
    candidates: &[Vec<StyleProperty>],
    // 借用 transition 简写供缺失值诊断。
    transition: &StyleProperty,
) -> Result<(), Diagnostic> {
    // 逐个验证选中字段。
    for kind in selected {
        // 初始目标必须能在首次 View 上形成具体 typed 值。
        validate_required_target(*kind, base, transition)?;
        // setStyle 每个完整分支都必须继续形成具体 typed 值。
        for branch in dynamic_branches {
            // 验证当前完整分支。
            validate_required_target(*kind, branch, transition)?;
        }
        // 所有显式值复用 animation 的范围与具体颜色验证。
        for property in candidates
            // 遍历候选集合。
            .iter()
            // 展平字段。
            .flat_map(|properties| properties.iter())
            // 只保留当前字段。
            .filter(|property| transition_property_kind(&property.name) == Some(*kind))
        {
            // 数值与颜色沿用同一 typed Animated 边界。
            match kind {
                // 颜色字段接受具体颜色与已登记主题 token 引用，运行期按
                // 当前有效主题解析后再插值。
                AnimationPropertyKind::Color | AnimationPropertyKind::BackgroundColor => {
                    // 解析结果只用于验证。
                    let _ = color_value(property)?;
                }
                // 数值字段验证范围与尺寸单位。
                _ => {
                    // 解析结果只用于验证。
                    let _ = animation_f32_value(*kind, property)?;
                }
            }
        }
    }
    // 报告全部目标值合法。
    Ok(())
}

// 验证一个完整目标分支能为字段提供运行时初值。
fn validate_required_target(
    // 接收闭合字段。
    kind: AnimationPropertyKind,
    // 借用完整分支字段。
    properties: &[StyleProperty],
    // 借用 transition 简写供诊断。
    transition: &StyleProperty,
) -> Result<(), Diagnostic> {
    // 显式值存在时由候选值验证负责具体语法。
    if properties
        // 遍历当前完整分支。
        .iter()
        // 查找同名字段。
        .any(|property| property.name == kind.style_name())
    {
        // 报告分支具备目标。
        return Ok(());
    }
    // borderRadius 与 opacity 的 Style 默认值本身可插值。
    if matches!(
        // 检查允许默认值的两类字段。
        kind,
        // 默认圆角为零。
        AnimationPropertyKind::BorderRadius
            // 默认透明度为一。
            | AnimationPropertyKind::Opacity
    ) {
        // 报告默认目标合法。
        return Ok(());
    }
    // 其他字段缺失时运行时无法得到固定或具体目标。
    Err(transition_diagnostic(
        // 指向完整简写。
        transition,
        // 陈述失败原因。
        format!("transition {} 缺少具体基础值", kind.style_name()),
        // 给出字段声明建议。
        format!(
            "在基础样式和每个 setStyle 目标类中声明 {}",
            kind.style_name()
        ),
    ))
}

// 解析固定顺序 transition 简写。
fn parse_transition_shorthand(
    // 借用完整简写属性。
    property: &StyleProperty,
) -> Result<(Option<AnimationPropertyKind>, u64, AnimationEasing, u64), Diagnostic> {
    // 按空白拆分最多四个固定位置字段。
    let parts = property
        // 借用原始源码。
        .value
        // 读取简写文本。
        .source
        // 按空白分词。
        .split_whitespace()
        // 收集供位置解析。
        .collect::<Vec<_>>();
    // 简写至少包含 property 且最多四项。
    if parts.is_empty() || parts.len() > 4 {
        // 返回固定顺序诊断。
        return Err(transition_diagnostic(
            // 指向完整简写。
            property,
            // 陈述失败原因。
            "transition 简写需要一到四个固定位置字段",
            // 给出完整顺序。
            "使用 property duration timing-function delay",
        ));
    }
    // 第一项选择 all 或单一首批字段。
    let selection = if parts[0] == "all" {
        // None 表示稍后从状态差异解析全部字段。
        None
    } else {
        // 具体字段必须属于首批矩阵。
        Some(transition_property_kind(parts[0]).ok_or_else(|| {
            // 返回未知字段诊断。
            transition_diagnostic(
                // 指向完整简写。
                property,
                // 陈述失败原因。
                format!("transition 属性 {:?} 不受支持", parts[0]),
                // 给出闭合集合。
                "使用 all、width、height、borderRadius、opacity、color 或 backgroundColor",
            )
        })?)
    };
    // 第二项缺失时使用零时长。
    let duration_micros = match parts.get(1) {
        // 解析显式时长。
        Some(value) => parse_transition_time(value, property, "duration")?,
        // 使用默认零时长。
        None => 0,
    };
    // 第三项缺失时使用默认 ease。
    let easing = match parts.get(2).copied().unwrap_or("ease") {
        // 线性插值。
        "linear" => AnimationEasing::Linear,
        // CSS 默认 ease。
        "ease" => AnimationEasing::Ease,
        // 缓入。
        "ease-in" => AnimationEasing::EaseIn,
        // 缓出。
        "ease-out" => AnimationEasing::EaseOut,
        // 缓入缓出。
        "ease-in-out" => AnimationEasing::EaseInOut,
        // 未登记曲线返回诊断。
        value => {
            // 返回闭合集合诊断。
            return Err(transition_diagnostic(
                // 指向完整简写。
                property,
                // 陈述失败原因。
                format!("transition timing-function {value:?} 不受支持"),
                // 给出支持集合。
                "使用 linear、ease、ease-in、ease-out 或 ease-in-out",
            ));
        }
    };
    // 第四项缺失时使用零延迟。
    let delay_micros = match parts.get(3) {
        // 解析显式延迟。
        Some(value) => parse_transition_time(value, property, "delay")?,
        // 使用默认零延迟。
        None => 0,
    };
    // 返回完整播放配置。
    Ok((selection, duration_micros, easing, delay_micros))
}

// 解析 transition 的秒或毫秒时间字段。
fn parse_transition_time(
    // 接收单个时间文本。
    source: &str,
    // 借用完整简写供诊断。
    property: &StyleProperty,
    // 接收字段名称。
    field: &str,
) -> Result<u64, Diagnostic> {
    // 按单位选择秒倍率。
    let (number, seconds_scale) = if let Some(number) = source.strip_suffix("ms") {
        // 毫秒转换为千分之一秒。
        (number, 0.001)
    } else if let Some(number) = source.strip_suffix('s') {
        // 秒保持原倍率。
        (number, 1.0)
    } else {
        // 无单位时间返回定向诊断。
        return Err(transition_diagnostic(
            // 指向完整简写。
            property,
            // 陈述失败原因。
            format!("transition {field} {source:?} 缺少 s 或 ms 单位"),
            // 给出合法示例。
            "使用 250ms、1s 或 1.5s",
        ));
    };
    // 解析有限非负十进制数。
    let value = number
        // 转换为 f64。
        .parse::<f64>()
        // 丢弃语法错误。
        .ok()
        // 只保留有限非负值。
        .filter(|value| value.is_finite() && *value >= 0.0)
        // 否则返回字段诊断。
        .ok_or_else(|| {
            // 构造稳定诊断。
            transition_diagnostic(
                // 指向完整简写。
                property,
                // 陈述失败原因。
                format!("transition {field} {source:?} 不是有限非负时间"),
                // 给出合法示例。
                "使用 0s、250ms 或 1.5s",
            )
        })?;
    // 换算为微秒。
    let micros = value * seconds_scale * 1_000_000.0;
    // 拒绝超过整数表示范围的时间。
    if micros > u64::MAX as f64 {
        // 返回范围诊断。
        return Err(transition_diagnostic(
            // 指向完整简写。
            property,
            // 陈述失败原因。
            format!("transition {field} 超出可表示范围"),
            // 给出修复建议。
            "缩短过渡时间",
        ));
    }
    // 四舍五入为确定性微秒整数。
    Ok(micros.round() as u64)
}

// 把样式名称映射到 transition 首批闭合字段。
fn transition_property_kind(name: &str) -> Option<AnimationPropertyKind> {
    // 按已支持矩阵映射。
    match name {
        // 显式宽度。
        "width" => Some(AnimationPropertyKind::Width),
        // 显式高度。
        "height" => Some(AnimationPropertyKind::Height),
        // 统一圆角。
        "borderRadius" => Some(AnimationPropertyKind::BorderRadius),
        // 透明度。
        "opacity" => Some(AnimationPropertyKind::Opacity),
        // 前景颜色。
        "color" => Some(AnimationPropertyKind::Color),
        // 背景颜色。
        "backgroundColor" => Some(AnimationPropertyKind::BackgroundColor),
        // 其他字段不属于首批矩阵。
        _ => None,
    }
}

// 构造 transition 统一诊断。
fn transition_diagnostic(
    // 借用完整简写或目标属性。
    property: &StyleProperty,
    // 接收失败原因。
    message: impl Into<String>,
    // 接收修复建议。
    suggestion: impl Into<String>,
) -> Diagnostic {
    // 返回带精确字段跨度的诊断。
    Diagnostic::new(
        // 使用属性跨度。
        property.span,
        // 保存失败原因。
        message,
        // 保存修复建议。
        suggestion,
    )
}
