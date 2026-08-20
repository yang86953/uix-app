// 引入有序属性映射以保持生成结果确定。
use std::collections::BTreeMap;

// 引入组件展开器。
use super::widget_codegen::WidgetExpander;
// 引入动画 AST、样式属性、元素与诊断。
use super::{
    AnimationBinding, AnimationDirection, AnimationEasing, AnimationFillMode, AnimationPlayback,
    AnimationPropertyBinding, AnimationPropertyFrame, AnimationPropertyKind, AttributeValue,
    Diagnostic, Element, StyleProperty, WidgetScopeMarker,
};

// 实现静态 animation 属性到持久化 Animated 绑定的降低。
impl WidgetExpander {
    // 消费合并后的 animation 简写并附加最终 View 装饰。
    pub(super) fn prepare_animation(
        // 可变借用展开器以读取关键帧注册表与作用域栈。
        &mut self,
        // 接收已经完成 class 与内联样式级联的核心元素。
        element: &mut Element,
    ) -> Result<(), Diagnostic> {
        // 保存消费出的 animation 属性。
        let mut animation = None;
        // 保存 animation 之外的最终基础样式属性。
        let mut baseline_properties = Vec::new();
        // 查找唯一结构化 style 属性。
        for attribute in &mut element.attributes {
            // 非 style 属性无需处理。
            if attribute.name != "style" {
                // 继续下一属性。
                continue;
            }
            // 合并器只会生成结构化内联样式。
            let AttributeValue::InlineStyle(properties) = &mut attribute.value else {
                // 返回内部形状保护诊断。
                return Err(Diagnostic::new(
                    // 指向完整 style 属性。
                    attribute.span,
                    // 陈述失败原因。
                    "animation 降低要求结构化 style 属性",
                    // 给出规范入口。
                    "使用 UIX 样式类或 style=\"animation: ...;\"",
                ));
            };
            // 从普通 Style 映射前移除 animation 简写。
            if let Some(index) = properties
                // 遍历最终级联属性。
                .iter()
                // 定位 animation 属性。
                .position(|property| property.name == "animation")
            {
                // 保存被消费的完整属性与跨度。
                animation = Some(properties.remove(index));
            }
            // 复制其余基础属性供 fill-mode 恢复值推导。
            baseline_properties = properties.clone();
            // style 属性唯一，完成后退出。
            break;
        }
        // 没有 animation 时保持元素不变。
        let Some(animation) = animation else {
            // 报告无需附加动画装饰。
            return Ok(());
        };
        // `none` 明确关闭动画且不建立运行时状态。
        if animation.value.source == "none" {
            // animation 已被消费，普通 Style 不再报告规划中诊断。
            return Ok(());
        }
        // 解析固定位置简写并取得关键帧名称。
        let (name, playback) = parse_animation_shorthand(&animation)?;
        // animation 必须引用同文档或已导入关键帧。
        let keyframes = self.keyframes.get(&name).cloned().ok_or_else(|| {
            // 返回未知关键帧诊断。
            Diagnostic::new(
                // 指向完整 animation 属性。
                animation.span,
                // 陈述失败原因。
                format!("animation 引用了未声明关键帧 {name}"),
                // 给出修复建议。
                format!("先声明 @keyframes {name} {{ ... }}"),
            )
        })?;
        // animation 必须位于组件或文档根状态作用域内。
        let Some(widget_scope) = self.widget_scope_stack.last().cloned() else {
            // 返回生命周期所有者诊断。
            return Err(Diagnostic::new(
                // 指向完整 animation 属性。
                animation.span,
                // 陈述失败原因。
                "animation 缺少声明式生命周期所有者",
                // 给出合法使用边界。
                "从 uix! 或 uix_app! 文档根展开该 View，或把它放入 Widget",
            ));
        };
        // 按闭合字段顺序收集各自关键帧。
        let mut property_frames =
            BTreeMap::<AnimationPropertyKind, Vec<AnimationPropertyFrame>>::new();
        // 遍历关键帧声明中的所有样式字段。
        for frame in &keyframes.frames {
            // 遍历当前偏移的字段。
            for property in &frame.properties {
                // 把字段名映射到首批可动画矩阵。
                let kind = animation_property_kind(property)?;
                // 追加当前字段在该偏移处的值。
                property_frames
                    // 定位或创建字段帧列表。
                    .entry(kind)
                    // 追加有序帧。
                    .or_default()
                    // 保存偏移与原始属性值。
                    .push(AnimationPropertyFrame {
                        // 复制确定性百万分比偏移。
                        offset_millionths: frame.offset_millionths,
                        // 复制待专用生成器解析的值。
                        property: property.clone(),
                    });
            }
        }
        // 保存完成基础值推导的属性绑定。
        let mut properties = Vec::with_capacity(property_frames.len());
        // 按闭合字段枚举顺序生成稳定绑定。
        for (kind, frames) in property_frames {
            // 关键帧收集保证字段至少有一个值。
            let first = frames
                // 读取首个字段帧。
                .first()
                // 内部不变量失败时使用动画属性返回受控诊断。
                .ok_or_else(|| {
                    // 返回缺少字段帧诊断。
                    Diagnostic::new(
                        // 指向完整 animation 属性。
                        animation.span,
                        // 陈述失败原因。
                        "关键帧没有可生成的动画字段",
                        // 给出支持矩阵。
                        "使用 width、height、borderRadius、opacity、color 或 backgroundColor",
                    )
                })?;
            // 优先使用最终基础样式同名值，否则用首个关键帧作为稳定基础值。
            let baseline = baseline_properties
                // 遍历 animation 之外的最终级联属性。
                .iter()
                // 查找与字段枚举对应的样式名。
                .find(|property| property.name == kind.style_name())
                // 复制显式基础值。
                .cloned()
                // 未声明基础值时回退到首帧。
                .unwrap_or_else(|| first.property.clone());
            // 保存完整字段绑定。
            properties.push(AnimationPropertyBinding {
                // 保存闭合字段类型。
                kind,
                // 保存 fill-mode 恢复基础值。
                baseline,
                // 保存有序关键帧。
                frames,
            });
        }
        // 使用节点类型与静态跨度形成跨构建稳定声明身份。
        let declaration_id = Self::stable_widget_id(&format!(
            // 固定身份前缀并纳入节点源码位置。
            "animation:{}:{}:{}",
            // 使用节点类型。
            element.name,
            // 使用节点起始偏移。
            element.span.start,
            // 使用节点结束偏移。
            element.span.end
        ));
        // 把动画元数据作为最终 View 装饰附加。
        element
            // 借用装饰列表。
            .widget_scopes
            // 追加 animation 装饰。
            .push(WidgetScopeMarker::Animation(AnimationBinding {
                // 保存最近生命周期作用域。
                widget_scope_name: widget_scope.to_string(),
                // 保存稳定节点声明身份。
                declaration_id,
                // 继承最近 For 的实际实例路径。
                instance_path_name: self.for_path_stack.last().map(ToString::to_string),
                // 保存完整播放配置。
                playback,
                // 保存字段关键帧。
                properties,
            }));
        // 报告降低成功。
        Ok(())
    }
}

// 实现闭合动画字段到 UIX 样式名的规范映射。
impl AnimationPropertyKind {
    // 返回关键帧与基础样式共享的字段名。
    pub(super) const fn style_name(self) -> &'static str {
        // 按闭合字段集合映射。
        match self {
            // 返回宽度字段名。
            Self::Width => "width",
            // 返回高度字段名。
            Self::Height => "height",
            // 返回圆角字段名。
            Self::BorderRadius => "borderRadius",
            // 返回透明度字段名。
            Self::Opacity => "opacity",
            // 返回前景颜色字段名。
            Self::Color => "color",
            // 返回背景颜色字段名。
            Self::BackgroundColor => "backgroundColor",
        }
    }
}

// 把关键帧样式名映射到闭合可动画字段。
fn animation_property_kind(
    // 接收关键帧中的单个样式属性。
    property: &StyleProperty,
) -> Result<AnimationPropertyKind, Diagnostic> {
    // 按首批支持矩阵选择字段。
    match property.name.as_str() {
        // 映射显式宽度。
        "width" => Ok(AnimationPropertyKind::Width),
        // 映射显式高度。
        "height" => Ok(AnimationPropertyKind::Height),
        // 映射统一圆角。
        "borderRadius" => Ok(AnimationPropertyKind::BorderRadius),
        // 映射节点透明度。
        "opacity" => Ok(AnimationPropertyKind::Opacity),
        // 映射前景颜色。
        "color" => Ok(AnimationPropertyKind::Color),
        // 映射普通背景颜色。
        "backgroundColor" => Ok(AnimationPropertyKind::BackgroundColor),
        // 未登记字段不能静默跳过。
        name => Err(Diagnostic::new(
            // 指向完整关键帧属性。
            property.span,
            // 陈述失败原因。
            format!("关键帧样式属性 {name} 尚无 Animated 绑定"),
            // 给出当前支持矩阵。
            "使用 width、height、borderRadius、opacity、color 或 backgroundColor",
        )),
    }
}

// 解析文档定义的固定位置 animation 简写。
fn parse_animation_shorthand(
    // 接收完整 animation 样式属性。
    property: &StyleProperty,
) -> Result<(String, AnimationPlayback), Diagnostic> {
    // 按空白拆分最多七个固定位置字段。
    let parts = property
        // 借用原始值文本。
        .value
        // 读取源码。
        .source
        // 按 Unicode 空白分词。
        .split_whitespace()
        // 收集供位置解析。
        .collect::<Vec<_>>();
    // 简写至少需要关键帧名称且最多七项。
    if parts.is_empty() || parts.len() > 7 {
        // 返回形状诊断。
        return Err(animation_diagnostic(
            // 传递属性跨度。
            property,
            // 陈述失败原因。
            "animation 简写需要一到七个固定位置字段",
            // 给出完整顺序。
            "使用 name duration timing-function delay iteration-count direction fill-mode",
        ));
    }
    // 第一项必须是规范标识符名称。
    let name = parts[0];
    // 拒绝不合法关键帧标识符。
    if !is_identifier(name) {
        // 返回名称诊断。
        return Err(animation_diagnostic(
            // 传递属性跨度。
            property,
            // 陈述失败原因。
            format!("animation 名称 {name:?} 不是合法标识符"),
            // 给出合法示例。
            "使用 fade、slideIn 或其他字母开头的名称",
        ));
    }
    // 第二项缺失时使用文档默认零时长。
    let duration_micros = match parts.get(1) {
        // 解析显式时长。
        Some(value) => parse_time(value, property, "duration")?,
        // 使用默认零时长。
        None => 0,
    };
    // 第三项缺失时使用 CSS 默认 ease。
    let easing = match parts.get(2).copied().unwrap_or("ease") {
        // 映射线性曲线。
        "linear" => AnimationEasing::Linear,
        // 映射默认 ease。
        "ease" => AnimationEasing::Ease,
        // 映射缓入。
        "ease-in" => AnimationEasing::EaseIn,
        // 映射缓出。
        "ease-out" => AnimationEasing::EaseOut,
        // 映射缓入缓出。
        "ease-in-out" => AnimationEasing::EaseInOut,
        // 拒绝未登记曲线。
        value => {
            // 返回缓动诊断。
            return Err(animation_diagnostic(
                // 传递属性跨度。
                property,
                // 陈述失败原因。
                format!("animation timing-function {value:?} 不受支持"),
                // 给出闭合集合。
                "使用 linear、ease、ease-in、ease-out 或 ease-in-out",
            ));
        }
    };
    // 第四项缺失时使用零延迟。
    let delay_micros = match parts.get(3) {
        // 解析显式延迟。
        Some(value) => parse_time(value, property, "delay")?,
        // 使用默认零延迟。
        None => 0,
    };
    // 第五项缺失时默认播放一轮。
    let iterations = match parts.get(4).copied().unwrap_or("1") {
        // infinite 映射为无界轮次。
        "infinite" => None,
        // 其他值必须是非负整数。
        value => Some(value.parse::<u64>().map_err(|_| {
            // 返回迭代次数诊断。
            animation_diagnostic(
                // 传递属性跨度。
                property,
                // 陈述失败原因。
                format!("animation iteration-count {value:?} 不是非负整数"),
                // 给出合法示例。
                "使用 0、1、2 或 infinite",
            )
        })?),
    };
    // 第六项缺失时默认正向播放。
    let direction = match parts.get(5).copied().unwrap_or("normal") {
        // 映射正向。
        "normal" => AnimationDirection::Normal,
        // 映射倒向。
        "reverse" => AnimationDirection::Reverse,
        // 映射正向交替。
        "alternate" => AnimationDirection::Alternate,
        // 映射倒向交替。
        "alternate-reverse" => AnimationDirection::AlternateReverse,
        // 拒绝未知方向。
        value => {
            // 返回方向诊断。
            return Err(animation_diagnostic(
                // 传递属性跨度。
                property,
                // 陈述失败原因。
                format!("animation direction {value:?} 不受支持"),
                // 给出闭合集合。
                "使用 normal、reverse、alternate 或 alternate-reverse",
            ));
        }
    };
    // 第七项缺失时默认不填充。
    let fill_mode = match parts.get(6).copied().unwrap_or("none") {
        // 映射无填充。
        "none" => AnimationFillMode::None,
        // 映射完成后填充。
        "forwards" => AnimationFillMode::Forwards,
        // 映射延迟期填充。
        "backwards" => AnimationFillMode::Backwards,
        // 映射双向填充。
        "both" => AnimationFillMode::Both,
        // 拒绝未知填充模式。
        value => {
            // 返回填充诊断。
            return Err(animation_diagnostic(
                // 传递属性跨度。
                property,
                // 陈述失败原因。
                format!("animation fill-mode {value:?} 不受支持"),
                // 给出闭合集合。
                "使用 none、forwards、backwards 或 both",
            ));
        }
    };
    // 返回名称与确定性播放配置。
    Ok((
        // 复制关键帧名称。
        name.to_string(),
        // 保存完整播放配置。
        AnimationPlayback {
            // 保存单轮时长。
            duration_micros,
            // 保存启动延迟。
            delay_micros,
            // 保存有限或无限轮次。
            iterations,
            // 保存缓动函数。
            easing,
            // 保存方向。
            direction,
            // 保存填充模式。
            fill_mode,
        },
    ))
}

// 把秒或毫秒时间文本转换为确定性微秒整数。
fn parse_time(
    // 接收单个时间字段文本。
    source: &str,
    // 接收完整 animation 属性供诊断定位。
    property: &StyleProperty,
    // 接收字段名称供诊断。
    field: &str,
) -> Result<u64, Diagnostic> {
    // 按后缀选择到秒的倍率。
    let (number, seconds_scale) = if let Some(number) = source.strip_suffix("ms") {
        // 毫秒换算为千分之一秒。
        (number, 0.001)
    } else if let Some(number) = source.strip_suffix('s') {
        // 秒保持原倍率。
        (number, 1.0)
    } else {
        // 无单位时间不属于文档契约。
        return Err(animation_diagnostic(
            // 传递属性跨度。
            property,
            // 陈述失败原因。
            format!("animation {field} {source:?} 缺少 s 或 ms 单位"),
            // 给出合法示例。
            "使用 250ms、1s 或 1.5s",
        ));
    };
    // 解析有限非负十进制数。
    let value = number
        // 转换为 f64 以保留时间精度。
        .parse::<f64>()
        // 只接受有限非负值。
        .ok()
        // 过滤非有限值。
        .filter(|value| value.is_finite() && *value >= 0.0)
        // 否则返回字段诊断。
        .ok_or_else(|| {
            // 构造有限非负时间诊断。
            animation_diagnostic(
                // 传递属性跨度。
                property,
                // 陈述失败原因。
                format!("animation {field} {source:?} 不是有限非负时间"),
                // 给出合法示例。
                "使用 0s、250ms 或 1.5s",
            )
        })?;
    // 换算为微秒并执行确定性四舍五入。
    let micros = value * seconds_scale * 1_000_000.0;
    // 拒绝超过 u64 表示范围的时间。
    if micros > u64::MAX as f64 {
        // 返回范围诊断。
        return Err(animation_diagnostic(
            // 传递属性跨度。
            property,
            // 陈述失败原因。
            format!("animation {field} 超出可表示范围"),
            // 给出修复建议。
            "缩短动画时间",
        ));
    }
    // 返回确定性微秒整数。
    Ok(micros.round() as u64)
}

// 判断字符串是否为 UIX 关键帧标识符。
fn is_identifier(value: &str) -> bool {
    // 读取首字符。
    let mut chars = value.chars();
    // 首字符必须是字母或下划线。
    let Some(first) = chars.next() else {
        // 空名称非法。
        return false;
    };
    // 拒绝非法首字符。
    if !first.is_ascii_alphabetic() && first != '_' {
        // 报告非法。
        return false;
    }
    // 后续只允许字母、数字、下划线或连字符。
    chars.all(|value| value.is_ascii_alphanumeric() || matches!(value, '_' | '-'))
}

// 构造 animation 简写的统一诊断。
fn animation_diagnostic(
    // 接收完整属性。
    property: &StyleProperty,
    // 接收失败原因。
    message: impl Into<String>,
    // 接收修复建议。
    help: impl Into<String>,
) -> Diagnostic {
    // 返回指向完整 animation 属性的诊断。
    Diagnostic::new(
        // 使用属性完整跨度。
        property.span,
        // 保存失败原因。
        message,
        // 保存修复建议。
        help,
    )
}
