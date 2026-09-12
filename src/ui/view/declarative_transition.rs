// 引入跨线程共享状态所需的同步原语。
use std::sync::{Arc, Mutex};

// 引入可插值具体颜色。
use crate::draw::Color;
// 引入既有声明式动画源与缓动曲线。
use crate::ui::animation::{Animated, Easing};
// 引入样式颜色值与主题令牌解析，支持语义主题目标。
use crate::ui::theme::style::ColorValue;

// 引入最终目标声明节点。
use super::ViewNode;

/// UIX transition 首批支持的闭合属性集合。
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UixTransitionProperty {
    /// 显式宽度。
    Width,
    /// 显式高度。
    Height,
    /// 统一圆角半径。
    BorderRadius,
    /// 节点透明度。
    Opacity,
    /// 前景或文本颜色。
    Color,
    /// 普通背景颜色。
    BackgroundColor,
}

/// UIX transition 的已验证播放参数。
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UixTransitionSpec {
    /// 单次过渡时长，单位秒。
    pub duration: f64,
    /// 启动延迟，单位秒。
    pub delay: f64,
    /// 当前过渡缓动曲线。
    pub easing: Easing,
}

/// UIX transition 生成器与运行时契约错配诊断。
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UixTransitionError {
    /// 目标节点没有可插值的固定宽度。
    MissingWidth,
    /// 目标节点没有可插值的固定高度。
    MissingHeight,
    /// 目标节点没有可插值的背景颜色。
    MissingBackgroundColor,
}

// 为生成代码的受控 panic 提供稳定错误文本。
impl std::fmt::Display for UixTransitionError {
    // 把闭合错误映射到用户可读文本。
    fn fmt(
        // 借用当前错误。
        &self,
        // 借用格式化器。
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        // 按失败字段返回稳定说明。
        formatter.write_str(match self {
            // 宽度缺少固定目标。
            Self::MissingWidth => "transition width 缺少固定目标值",
            // 高度缺少固定目标。
            Self::MissingHeight => "transition height 缺少固定目标值",
            // 背景字段缺少可解析值。
            Self::MissingBackgroundColor => "transition backgroundColor 缺少背景颜色值",
        })
    }
}

// 把样式颜色值解析为当前构建期有效主题下的具体颜色。
//
// 具体颜色原样返回；语义主题色经局部 Provider 与窗口主题作用域解析，
// 让主题切换在下一轮声明构建中形成新目标并从当前呈现值续播。
fn resolved_color(value: ColorValue) -> Color {
    let tokens = crate::ui::__private::uix_effective_build_tokens();
    value.resolve(tokens.as_ref())
}

// 保存单个 typed 动画源与最近声明目标。
#[derive(Debug, Clone)]
struct TransitionSlot<T>
where
    // 目标必须可被 Animated 插值并跨线程读取。
    T: crate::ui::animation::traits::Animatable + Sync,
{
    // 保存窗口帧调度器推进的共享源。
    source: Animated<T>,
    // 保存最近声明目标以避免每次 reconcile 重启动画。
    target: T,
}

// 为可比较目标提供原位重定向。
impl<T> TransitionSlot<T>
where
    // 目标必须可插值、可比较并跨线程读取。
    T: crate::ui::animation::traits::Animatable + Sync + PartialEq,
{
    // 使用首次目标创建静止源。
    fn new(target: T) -> Self {
        // 返回共享源与目标快照。
        Self {
            // 首次挂载从最终目标静止开始，不伪装状态变化。
            source: Animated::new(target),
            // 保存首次目标供后续差异比较。
            target,
        }
    }

    // 目标变化时从当前值重定向；同目标通常不重启动画。
    fn value_for(&mut self, target: T, spec: UixTransitionSpec) -> T {
        // 立即完成策略：零时长零延迟或减少动效开启。进行中的同目标过渡
        // 也必须收束到目标并释放定时帧，不能因目标相等被跳过。
        let immediate_finish =
            crate::ui::animation::reduced_motion() || (spec.duration <= 0.0 && spec.delay <= 0.0);
        if self.target != target || (immediate_finish && !self.source.is_finished()) {
            // 从 source 当前值过渡到新目标并复用同一活动源身份；
            // 立即完成策略由播放层落到目标值。
            self.source
                .animate_to_after(spec.delay, target, spec.duration, spec.easing);
            // 保存新目标供下一次 reconcile 比较。
            self.target = target;
        }
        // 读取当前帧值并登记到既有窗口帧捕获。
        self.source.value()
    }
}

// 保存一个节点全部已选 transition 字段。
#[derive(Debug, Clone, Default)]
struct TransitionValues {
    // 可选宽度源。
    width: Option<TransitionSlot<f32>>,
    // 可选高度源。
    height: Option<TransitionSlot<f32>>,
    // 可选圆角源。
    border_radius: Option<TransitionSlot<f32>>,
    // 可选透明度源。
    opacity: Option<TransitionSlot<f32>>,
    // 可选前景颜色源。
    color: Option<TransitionSlot<Color>>,
    // 可选背景颜色源。
    background_color: Option<TransitionSlot<Color>>,
}

/// 持久化一个 UIX 节点 transition 字段和最近目标。
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct UixDeclarativeTransition {
    // 内部互斥只保护短期目标比较，不拥有帧调度生命周期。
    inner: Arc<Mutex<TransitionValues>>,
}

/// 组合声明位置或 For 路径与最终 View key，形成 transition 状态身份。
#[doc(hidden)]
pub fn uix_transition_identity(
    // 借用已经完成 key 应用的最终目标节点。
    view: &ViewNode,
    // 接收静态位置或实际 For 实例路径。
    instance_path: &str,
) -> String {
    // 显式 key 存在时参与实际节点身份。
    match &view.key {
        // 组合路径与 key，避免同位置换 key 复用旧动画。
        Some(key) => format!("{instance_path}|{key}"),
        // 无 key 时直接沿用静态或循环实例路径。
        None => instance_path.to_string(),
    }
}

/// 从首次挂载的最终目标 View 创建静止 transition 状态。
#[doc(hidden)]
pub fn uix_transition_state(
    // 借用已经完成全部基础与状态样式叠加的节点。
    view: &ViewNode,
    // 接收编译器验证后的闭合字段集合。
    properties: &[UixTransitionProperty],
) -> Result<UixDeclarativeTransition, UixTransitionError> {
    // 创建空字段集合并按声明选择初始化。
    let mut values = TransitionValues::default();
    // 遍历去重后的编译期字段集合。
    for property in properties {
        // 按字段读取首次最终目标。
        match property {
            // 宽度必须已有固定值。
            UixTransitionProperty::Width => {
                // 缺少固定宽度时报告生成器契约错配。
                let target = view.style.width.ok_or(UixTransitionError::MissingWidth)?;
                // 从首次目标创建静止宽度源。
                values.width = Some(TransitionSlot::new(target));
            }
            // 高度必须已有固定值。
            UixTransitionProperty::Height => {
                // 缺少固定高度时报告生成器契约错配。
                let target = view.style.height.ok_or(UixTransitionError::MissingHeight)?;
                // 从首次目标创建静止高度源。
                values.height = Some(TransitionSlot::new(target));
            }
            // 圆角始终拥有有限基础值，编译器负责数值范围。
            UixTransitionProperty::BorderRadius => {
                // 从首次圆角创建静止源。
                values.border_radius = Some(TransitionSlot::new(view.style.border_radius));
            }
            // 透明度始终拥有基础值，编译器负责零到一范围。
            UixTransitionProperty::Opacity => {
                // 从首次透明度创建静止源。
                values.opacity = Some(TransitionSlot::new(view.style.opacity));
            }
            // 前景颜色按当前有效主题解析为具体颜色。
            UixTransitionProperty::Color => {
                // 语义主题色与具体颜色都经有效令牌快照解析。
                let target = resolved_color(view.style.color);
                // 从首次前景色创建静止源。
                values.color = Some(TransitionSlot::new(target));
            }
            // 背景颜色必须存在并按当前有效主题解析。
            UixTransitionProperty::BackgroundColor => {
                // 缺失背景返回契约错配；主题语义值正常解析。
                let target = view
                    .style
                    .background
                    .map(resolved_color)
                    .ok_or(UixTransitionError::MissingBackgroundColor)?;
                // 从首次背景色创建静止源。
                values.background_color = Some(TransitionSlot::new(target));
            }
        }
    }
    // 返回可由组件私有 State 跨 reconcile 复用的共享容器。
    Ok(UixDeclarativeTransition {
        // 只创建一个短期互斥内部值容器。
        inner: Arc::new(Mutex::new(values)),
    })
}

impl UixDeclarativeTransition {
}

/// 比较最终声明目标、原位重定向变化字段并写回当前帧值。
#[doc(hidden)]
pub fn uix_apply_transition(
    // 接收已经完成全部状态样式叠加的目标节点。
    mut view: ViewNode,
    // 借用组件私有持久化 transition 状态。
    state: &UixDeclarativeTransition,
    // 接收编译期验证的播放配置。
    spec: UixTransitionSpec,
) -> Result<ViewNode, UixTransitionError> {
    // 短期锁定目标快照与 typed 动画源。
    let mut values = state
        // 借用共享内部容器。
        .inner
        // 取得短期互斥锁。
        .lock()
        // poison 时恢复内部值，避免用户 panic 永久破坏窗口状态。
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // 选中宽度时读取目标并写回当前帧值。
    if let Some(slot) = values.width.as_mut() {
        // 目标节点必须继续提供固定宽度。
        let target = view.style.width.ok_or(UixTransitionError::MissingWidth)?;
        // 原位重定向后覆盖声明目标宽度。
        let frame = slot.value_for(target, spec);
        // 帧值是最终有效值：值样式与显式声明快照同步，避免 S1 的
        // 声明最后覆盖把过渡帧值盖回静态声明。
        view.style.width = Some(frame);
        view.style_decl.width = Some(Some(frame));
    }
    // 选中高度时读取目标并写回当前帧值。
    if let Some(slot) = values.height.as_mut() {
        // 目标节点必须继续提供固定高度。
        let target = view.style.height.ok_or(UixTransitionError::MissingHeight)?;
        // 原位重定向后覆盖声明目标高度。
        let frame = slot.value_for(target, spec);
        // 与宽度同理：声明快照跟随最终帧值。
        view.style.height = Some(frame);
        view.style_decl.height = Some(Some(frame));
    }
    // 选中圆角时读取目标并写回当前帧值。
    if let Some(slot) = values.border_radius.as_mut() {
        // 保存状态分支叠加后的目标圆角。
        let target = view.style.border_radius;
        // 原位重定向后覆盖声明目标圆角。
        let frame = slot.value_for(target, spec);
        // 标量圆角动画按单值输入落地，显式清除四角形式。
        view.style.border_radius = frame;
        view.style.border_radius_corners = None;
        view.style_decl.border_radius = Some(frame);
        view.style_decl.border_radius_corners = Some(None);
    }
    // 选中透明度时读取目标并写回当前帧值。
    if let Some(slot) = values.opacity.as_mut() {
        // 保存状态分支叠加后的目标透明度。
        let target = view.style.opacity;
        // 原位重定向后覆盖声明目标透明度。
        let frame = slot.value_for(target, spec);
        view.style.opacity = frame;
        view.style_decl.opacity = Some(frame);
    }
    // 选中前景色时解析目标并写回当前帧值。
    if let Some(slot) = values.color.as_mut() {
        // 语义主题色按当前有效主题解析，主题切换在下一轮构建形成新目标。
        let target = resolved_color(view.style.color);
        // 原位重定向后覆盖声明目标前景色。
        let frame = ColorValue::Custom(slot.value_for(target, spec));
        view.style.color = frame;
        view.style_decl.color = Some(frame);
    }
    // 选中背景色时解析目标并写回当前帧值。
    if let Some(slot) = values.background_color.as_mut() {
        // 缺失背景不得绕过编译期值约束。
        let target = view
            // 借用目标样式。
            .style
            // 读取可选背景。
            .background
            // 按当前有效主题解析具体颜色。
            .map(resolved_color)
            // 否则报告契约错配。
            .ok_or(UixTransitionError::MissingBackgroundColor)?;
        // 原位重定向后覆盖声明目标背景色。
        let frame = ColorValue::Custom(slot.value_for(target, spec));
        view.style.background = Some(frame);
        view.style_decl.background = Some(Some(frame));
    }
    // 返回写入当前动画帧值的同一节点。
    Ok(view)
}

#[cfg(test)]
#[path = "../../../tests-src/ui/view/declarative_transition_tests.rs"]
mod tests;

