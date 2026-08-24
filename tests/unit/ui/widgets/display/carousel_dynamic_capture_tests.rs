// 复用父模块私有 Carousel 类型与固定动态身份。
use super::{CAROUSEL_VISUAL_REF, Carousel, CarouselEffect, SelectionSource};
// 导入真实声明建树和原位协调入口。
use crate::ui::adapter::ViewAdapter;
// 导入组件私有状态 scope 与槽位创建入口。
use crate::ui::widget_state::{uix_widget_scope, uix_widget_state};
// 导入运行时节点读取与组件 patch 所需契约。
use crate::ui::widget_runtime::widget::WidgetCore;
// 导入最小声明节点类型。
use crate::ui::view::{View, ViewNode};
// 导入幻灯片与箭头测试承载组件。
use crate::ui::widgets::Label;
// 导入动态捕获、动画、Effect 与树生命周期测试能力。
use crate::ui::{Animated, Easing, Effect, State, Transition, WidgetId, WidgetTree};
// 导入同线程保存 previous/next 动作所需容器。
use std::cell::RefCell;
// 导入工厂动作与记录的共享所有权。
use std::rc::Rc;
// 导入跨 Effect 调度共享的原子计数器。
use std::sync::atomic::{AtomicUsize, Ordering};
// 导入可由 Effect 闭包安全持有的共享状态记录。
use std::sync::{Arc, Mutex};

// 验证 UIX 根桥接保持 Carousel 动态类型与有序幻灯片子树。
#[test]
fn uix_root_preserves_carousel_kernel_and_slides() {
    // 构造两个有序幻灯片并经代码生成桥接进入 Carousel UIX 根。
    let slides = vec![
        ViewNode::leaf(Label::new("first")),
        ViewNode::leaf(Label::new("second")),
    ];
    let node = Carousel::new().build_view_with_children(slides);
    // UIX 声明不得增加包装或复制幻灯片节点。
    assert_eq!(node.children.len(), 2);
    // 根动态类型必须继续是拥有计时器、选择与动画机制的 Carousel。
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Carousel>()
        .expect("UIX 根必须保留 Carousel 内核");
    // UIX 视觉配置必须进入真实内核，而不是只保留根节点壳。
    assert_eq!(
        kernel.visual_contract_for_test(),
        (300.0, 200.0, 30.0, 16.0, 18.0, 6.0, 0.3)
    );
    // 公开 View 入口的空轮播同样保持零子节点形状。
    assert!(View::build(Carousel::new()).children.is_empty());
}

// UIX 默认控制只填充未显式设置字段，且实例共享静态视觉表。
#[test]
fn uix_defaults_preserve_authored_carousel_controls_and_share_visuals() {
    let authored = Carousel::new()
        .show_dots(false)
        .show_arrows(false)
        .effect(CarouselEffect::Fade);
    assert!(std::ptr::eq(authored.visual, CAROUSEL_VISUAL_REF));
    let authored = View::build(authored);
    let defaults = View::build(Carousel::new());
    let authored = authored
        .widget
        .as_any()
        .downcast_ref::<Carousel>()
        .expect("作者 Carousel 必须保留内核");
    let defaults = defaults
        .widget
        .as_any()
        .downcast_ref::<Carousel>()
        .expect("默认 Carousel 必须保留内核");
    assert_eq!(
        (authored.show_dots, authored.show_arrows, authored.effect),
        (false, false, CarouselEffect::Fade)
    );
    assert_eq!(
        (defaults.show_dots, defaults.show_arrows, defaults.effect),
        (true, true, CarouselEffect::Slide)
    );
    assert!(authored.shares_visual_with_for_test(defaults));
    assert!(std::ptr::eq(authored.visual, CAROUSEL_VISUAL_REF));
}

// 淡入淡出透明度必须严格复用 UIX 关键点，并保持三角峰值质量。
#[test]
fn fade_opacity_uses_uix_motion_keypoints() {
    let node = View::build(Carousel::new().effect(CarouselEffect::Fade));
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Carousel>()
        .expect("淡入淡出 Carousel 必须保留内核");
    kernel.runtime.set_child_count(2);
    kernel.runtime.select(1, SelectionSource::User);
    assert_eq!(kernel.runtime.fade_overlay_opacity(), 0.0);
    kernel.runtime.fade_progress.set(0.25);
    assert_eq!(kernel.runtime.fade_overlay_opacity(), 0.5);
    kernel.runtime.fade_progress.set(0.5);
    assert_eq!(kernel.runtime.fade_overlay_opacity(), 1.0);
    kernel.runtime.fade_progress.set(0.75);
    assert_eq!(kernel.runtime.fade_overlay_opacity(), 0.5);
}

// 读取当前 Carousel 根 owner 身份。
fn carousel_root(
    // 接收已经完成真实挂载的运行时树。
    tree: &WidgetTree,
    // 返回唯一根组件身份。
) -> WidgetId {
    // 本文件每棵测试树只声明一个 Carousel 根。
    tree.root_id().expect("Carousel 必须拥有运行时根")
}

// 在 Carousel 直接子节点中定位框架固定自定义箭头。
fn custom_arrows_child(
    // 接收当前运行时树。
    tree: &WidgetTree,
    // 接收仍属于该树的 Carousel owner。
    root: WidgetId,
    // 返回固定动态箭头身份。
) -> WidgetId {
    // 只按产品保留 key 查找，避免把 authored slide 当作箭头。
    tree.get(root)
        // owner 在查找期间必须保持可寻址。
        .expect("Carousel owner 必须存在")
        // 遍历全部直接子节点，包括 pending leave 墓碑。
        .children()
        // 借用子节点序列。
        .iter()
        // 复制轻量 WidgetId。
        .copied()
        // 查找唯一固定身份。
        .find(|child| {
            // 读取运行时 key 并与 Carousel 保留值比较。
            tree.get(*child).and_then(|node| node.key())
                // 固定箭头不得依赖声明顺序。
                == Some(Carousel::CUSTOM_ARROWS_CHILD_KEY)
        })
        // 启用自定义箭头时必须已经物化。
        .expect("Carousel 自定义箭头必须存在")
}

// 从工厂共享记录取得最近一次捕获的私有状态句柄。
fn captured_state(
    // 接收工厂写入的线程安全共享槽位。
    states: &Arc<Mutex<Option<State<i32>>>>,
    // 返回可脱离记录锁使用的 State 句柄。
) -> State<i32> {
    // 正常完成的箭头工厂必须已经回传状态。
    states
        // 取得短期记录锁。
        .lock()
        // 测试仍需读取 poisoned 锁中的确定值。
        .unwrap_or_else(|error| error.into_inner())
        // 借用最近状态。
        .as_ref()
        // 克隆公开句柄后立即释放锁。
        .cloned()
        // 缺少状态表示工厂没有进入捕获边界。
        .expect("Carousel 箭头工厂必须捕获私有 State")
}

// 构造两个 authored slides，供所有动态箭头测试复用。
fn carousel_view(
    // 接收要挂载或协调的 Carousel 声明。
    carousel: Carousel,
    // 返回包含稳定有序幻灯片的声明根。
) -> ViewNode {
    // 幻灯片始终由调用方声明，动态箭头只能追加在其后。
    ViewNode::new(
        // 使用调用方提供的 Carousel 配置。
        carousel,
        // 声明两个可观察的普通幻灯片。
        vec![
            // 第一页使用稳定 authored key。
            ViewNode::leaf(Label::new("first")).key("carousel-slide-first"),
            // 第二页使用不同稳定 authored key。
            ViewNode::leaf(Label::new("second")).key("carousel-slide-second"),
        ],
    )
}

// 只替换 live Carousel 声明字段，供预发布失败路径直接触发刷新。
fn sync_runtime_carousel(
    // 接收当前运行时树。
    tree: &mut WidgetTree,
    // 接收实际 Carousel owner。
    root: WidgetId,
    // 接收下一版组件声明。
    next: Carousel,
) {
    // 只有当前真实 Carousel 可以接纳测试工厂。
    tree.get_mut(root)
        // owner 必须仍然有效。
        .expect("Carousel owner 必须存在")
        // 取得组件可变契约。
        .widget_mut()
        // 下转型入口不泄漏到产品公开 API。
        .as_any_mut()
        // 验证运行时类型。
        .downcast_mut::<Carousel>()
        // 测试设置期间不允许 owner 类型变化。
        .expect("运行时 owner 必须是 Carousel")
        // 沿真实原位 patch 逻辑复制运行态并安装新工厂。
        .sync_from(next);
}

// 构造携带完整动态输出与可选 leave 的自定义箭头 Carousel。
fn captured_carousel(
    // 接收用于区分新旧工厂输出的显示文本。
    label: &'static str,
    // 接收工厂调用次数记录。
    factory_calls: Arc<AtomicUsize>,
    // 接收私有状态句柄回传记录。
    states: Arc<Mutex<Option<State<i32>>>>,
    // 接收 Effect 响应依赖。
    effect_dependency: State<i32>,
    // 接收 Effect 实际运行次数记录。
    effect_runs: Arc<AtomicUsize>,
    // 接收必须归动态箭头 owner 的动画源。
    animation: Animated<f32>,
    // 接收可选离场过渡。
    leave: Option<Transition>,
    // 接收 next 窄动作回传槽位。
    next_action: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    // 返回启用自定义箭头的 Carousel 声明。
) -> Carousel {
    // 克隆调用记录供可重复工厂持有。
    let renderer_calls = Arc::clone(&factory_calls);
    // 克隆状态记录供每轮捕获回传最新句柄。
    let renderer_states = Arc::clone(&states);
    // 克隆 Effect 计数器供副作用闭包长期持有。
    let renderer_effect_runs = Arc::clone(&effect_runs);
    // 返回只改变自定义箭头能力的 Carousel。
    Carousel::new().arrows(move |_previous, next| {
        // 每次真实物化或父协调重捕获都必须可观察。
        renderer_calls.fetch_add(1, Ordering::Relaxed);
        // 回传不泄漏 Carousel 实现的下一页窄动作。
        *next_action.borrow_mut() = Some(next);
        // 为固定动态箭头声明稳定组件私有 scope。
        let scope = uix_widget_scope("carousel-custom-arrows-dynamic-capture-test", 1);
        // 在 owner、槽位与固定 key 限定的命名空间中取得 State。
        let state = uix_widget_state(&scope, 1, || 0_i32);
        // 保存本轮状态句柄供协调后断言。
        *renderer_states
            // 取得短期写锁。
            .lock()
            // 测试继续恢复 poisoned 锁中的记录。
            .unwrap_or_else(|error| error.into_inner()) = Some(state.clone());
        // 读取私有状态，使箭头声明显式交接所属树的结构协调订阅。
        let _ = state.get();
        // 克隆依赖供 Effect 闭包独立持有。
        let dependency = effect_dependency.clone();
        // 克隆计数器供 Effect 生命周期持有。
        let effect_runs = Arc::clone(&renderer_effect_runs);
        // 创建必须随动态箭头替换或移除的 Effect。
        let _ = Effect::new(move || {
            // 读取 State 以建立自动依赖订阅。
            let _ = dependency.get();
            // 记录首次与后续调度。
            effect_runs.fetch_add(1, Ordering::Relaxed);
        });
        // 读取动画值以让捕获事务交接 AnimatedSource。
        let _ = animation.value();
        // 建立承载私有 scope 的最小箭头根。
        let node = ViewNode::leaf(Label::new(label)).uix_widget_scope(scope, 0);
        // 仅在生命周期测试中安装非零 leave。
        match leave {
            // 非零 leave 允许观察墓碑与同 key 重入。
            Some(leave) => node.leave(leave),
            // 普通场景保持立即移除。
            None => node,
        }
    })
}

// 验证父协调复用固定身份、私有状态和完整动态输出。
#[test]
// next 窄动作仍必须驱动 Carousel 自身选择策略。
fn carousel_custom_arrows_dynamic_capture_reuses_identity_and_complete_outputs() {
    // 建立首版工厂调用记录。
    let first_calls = Arc::new(AtomicUsize::new(0));
    // 建立首版状态回传槽位。
    let first_states = Arc::new(Mutex::new(None));
    // 建立首版 Effect 依赖。
    let first_dependency = State::new(0_i32);
    // 建立首版 Effect 调度记录。
    let first_runs = Arc::new(AtomicUsize::new(0));
    // 建立首版动画源。
    let first_animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 建立 next 动作回传槽位。
    let first_next = Rc::new(RefCell::new(None));
    // 通过真实声明建树入口挂载两个 slides 与自定义箭头。
    let mut tree = ViewAdapter::build(carousel_view(captured_carousel(
        // 首版箭头显示标记。
        "first-arrows",
        // 交接首版工厂记录。
        Arc::clone(&first_calls),
        // 交接首版状态记录。
        Arc::clone(&first_states),
        // Effect 使用独立依赖。
        first_dependency.clone(),
        // 交接首版 Effect 记录。
        Arc::clone(&first_runs),
        // 交接首版动画源。
        first_animation,
        // 首版无需离场。
        None,
        // 回传首版 next 动作。
        Rc::clone(&first_next),
    )));
    // 保存稳定 Carousel owner。
    let root = carousel_root(&tree);
    // 保存固定动态箭头身份。
    let arrows = custom_arrows_child(&tree, root);
    // 首次工厂必须只执行一次。
    assert_eq!(first_calls.load(Ordering::Relaxed), 1);
    // 自定义箭头不得被计入幻灯片数量。
    assert_eq!(
        tree.get(root)
            // owner 必须存在。
            .expect("Carousel owner 必须存在")
            // 取得只读组件。
            .widget()
            // 进入类型擦除读取。
            .as_any()
            // 恢复 Carousel。
            .downcast_ref::<Carousel>()
            // 运行时类型必须稳定。
            .expect("根必须是 Carousel")
            // 读取产品幻灯片计数。
            .slide_count(),
        // authored slides 恰好有两个。
        2
    );
    // 初始私有状态必须使用声明初值。
    assert_eq!(captured_state(&first_states).get(), 0);
    // 写入必须跨下一轮父协调保持的状态。
    captured_state(&first_states).set(23);
    // State.set 必须请求所属树协调。
    assert!(tree.take_reconcile_requested());
    // 初建 Effect 必须执行一次。
    assert_eq!(first_runs.load(Ordering::Relaxed), 1);
    // 动态箭头必须登记唯一动画源。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 保存首版动画工作身份供替换断言。
    let first_animation_id = tree.animated_source_registrations()[0].0;
    // 调用工厂收到的 next 窄动作。
    first_next
        // 借用动作槽位。
        .borrow()
        // 取得已回传动作。
        .as_ref()
        // 工厂必须收到 next 动作。
        .expect("箭头工厂必须收到 next 动作")();
    // next 动作必须由 Carousel 运行态推进到第二页。
    assert_eq!(
        tree.get(root)
            // owner 仍必须存在。
            .expect("Carousel owner 必须存在")
            // 读取组件。
            .widget()
            // 下转型入口。
            .as_any()
            // 恢复 Carousel。
            .downcast_ref::<Carousel>()
            // 类型必须保持稳定。
            .expect("根必须是 Carousel")
            // 读取当前选择索引。
            .current_index(),
        // 第二页索引为一。
        1
    );
    // 建立新版工厂状态记录。
    let next_states = Arc::new(Mutex::new(None));
    // 建立新版 Effect 依赖。
    let next_dependency = State::new(0_i32);
    // 建立新版 Effect 运行记录。
    let next_runs = Arc::new(AtomicUsize::new(0));
    // 建立新版工厂调用记录。
    let next_calls = Arc::new(AtomicUsize::new(0));
    // 建立新版动作回传槽位。
    let next_action = Rc::new(RefCell::new(None));
    // 用相同 authored slides 与新版箭头工厂协调同一 owner。
    ViewAdapter::reconcile(
        // 借用当前树执行真实原位协调。
        &mut tree,
        // 构造新版完整声明。
        carousel_view(captured_carousel(
            // 新版箭头显示标记。
            "next-arrows",
            // 交接新版工厂记录。
            Arc::clone(&next_calls),
            // 交接新版状态记录。
            Arc::clone(&next_states),
            // 使用新版 Effect 依赖。
            next_dependency.clone(),
            // 交接新版 Effect 记录。
            Arc::clone(&next_runs),
            // 使用不同动画源证明替换交接。
            Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear),
            // 新版无需离场。
            None,
            // 回传新版动作。
            next_action,
        )),
    );
    // 同类型父协调必须保留 Carousel owner。
    assert_eq!(carousel_root(&tree), root);
    // 固定 key 必须复用原箭头 WidgetId。
    assert_eq!(custom_arrows_child(&tree, root), arrows);
    // 新工厂只执行一次。
    assert_eq!(next_calls.load(Ordering::Relaxed), 1);
    // 新工厂必须取得先前写入的同一状态槽。
    assert_eq!(captured_state(&next_states).get(), 23);
    // Carousel 选择运行态必须跨声明 patch 保持。
    assert_eq!(
        tree.get(root)
            // 读取 live owner。
            .expect("Carousel owner 必须存在")
            // 读取运行时组件。
            .widget()
            // 进入下转型。
            .as_any()
            // 恢复 Carousel。
            .downcast_ref::<Carousel>()
            // 类型必须稳定。
            .expect("根必须是 Carousel")
            // 读取保留的当前索引。
            .current_index(),
        // 仍停留第二页。
        1
    );
    // 旧 Effect 依赖变化不得再形成当前树工作。
    first_dependency.set(1);
    // 旧 Effect 已被新版动态声明替换。
    assert!(!tree.has_pending_effects());
    // 新 Effect 依赖变化必须进入树级调度。
    next_dependency.set(1);
    // tick 必须执行当前动态箭头 Effect。
    assert!(tree.tick_effects());
    // 新 Effect 包含首次与本次更新两次运行。
    assert_eq!(next_runs.load(Ordering::Relaxed), 2);
    // 动画表仍只能保留当前箭头的一项工作。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 新版动画必须替换首版工作身份。
    assert_ne!(
        tree.animated_source_registrations()[0].0,
        first_animation_id
    );
}

// 验证固定箭头 leave 墓碑、同 key 重入与 shutdown 生命周期。
#[test]
// 重入必须复用 WidgetId 和已提交私有 State。
fn carousel_custom_arrows_dynamic_capture_leave_reentry_and_shutdown() {
    // 建立共享状态记录。
    let states = Arc::new(Mutex::new(None));
    // 建立 Effect 依赖。
    let dependency = State::new(0_i32);
    // 建立 Effect 运行记录。
    let runs = Arc::new(AtomicUsize::new(0));
    // 建立工厂调用记录。
    let calls = Arc::new(AtomicUsize::new(0));
    // 建立不需要读取的动作槽位。
    let actions = Rc::new(RefCell::new(None));
    // 挂载带长离场过渡的自定义箭头。
    let mut tree = ViewAdapter::build(carousel_view(captured_carousel(
        // 初始显示标记。
        "leaving-arrows",
        // 交接调用记录。
        Arc::clone(&calls),
        // 交接状态记录。
        Arc::clone(&states),
        // 交接 Effect 依赖。
        dependency.clone(),
        // 交接 Effect 记录。
        Arc::clone(&runs),
        // 交接动态动画源。
        Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear),
        // 设置非零 leave。
        Some(Transition::fade_out(10.0)),
        // 交接动作槽位。
        Rc::clone(&actions),
    )));
    // 保存 owner 与固定箭头身份。
    let root = carousel_root(&tree);
    // 保存离场前动态子节点。
    let arrows = custom_arrows_child(&tree, root);
    // 写入重入后必须保留的状态。
    captured_state(&states).set(41);
    // 用未启用自定义箭头的同源声明启动 leave。
    ViewAdapter::reconcile(&mut tree, carousel_view(Carousel::new()));
    // 非零 leave 期间固定箭头仍必须是直接子节点。
    assert_eq!(custom_arrows_child(&tree, root), arrows);
    // 动态箭头必须进入 pending-removal。
    assert!(
        tree.get(arrows)
            // leave 墓碑必须仍可寻址。
            .expect("离场箭头必须存在")
            // 读取离场标志。
            .pending_removal()
    );
    // 重建同固定身份的自定义箭头声明。
    ViewAdapter::reconcile(
        // 在同一树内协调。
        &mut tree,
        // 恢复带相同 scope 的箭头。
        carousel_view(captured_carousel(
            // 重入显示标记。
            "reentered-arrows",
            // 复用工厂调用记录。
            Arc::clone(&calls),
            // 复用状态记录。
            Arc::clone(&states),
            // 复用 Effect 依赖。
            dependency,
            // 复用 Effect 记录。
            Arc::clone(&runs),
            // 提供重入后的新动画声明。
            Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear),
            // 继续保留 leave 契约。
            Some(Transition::fade_out(10.0)),
            // 复用动作槽位。
            actions,
        )),
    );
    // 同 key 重入不得分配新 WidgetId。
    assert_eq!(custom_arrows_child(&tree, root), arrows);
    // 重入必须取消 pending-removal。
    assert!(
        !tree
            // 读取重入后的原节点。
            .get(arrows)
            // 原节点必须仍然存在。
            .expect("重入箭头必须存在")
            // 读取离场状态。
            .pending_removal()
    );
    // 已提交私有 State 必须跨 leave/reentry 保持。
    assert_eq!(captured_state(&states).get(), 41);
    // 初建与重入父协调各调用一次工厂。
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    // 关闭完整树以验证最终 owner teardown。
    tree.shutdown();
    // shutdown 必须释放全部动态动画工作。
    assert!(tree.animated_source_registrations().is_empty());
    // shutdown 后刷新必须在工厂调用前拒绝。
    assert!(!tree.refresh_carousel_custom_arrows_widget(root));
    // 拒绝不得额外调用应用工厂。
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

// 验证 authored 保留 key、工厂根 key、预发布 panic 与 stale owner 拒绝。
#[test]
// 所有预发布失败都不得留下半发布动态箭头或污染后续状态槽。
fn carousel_custom_arrows_dynamic_capture_rejects_invalid_identity_and_rolls_back() {
    // 建立 authored key 冲突路径的工厂调用记录。
    let conflict_calls = Rc::new(RefCell::new(0_usize));
    // 克隆调用记录供冲突工厂持有。
    let renderer_conflict_calls = Rc::clone(&conflict_calls);
    // 捕获 authored slide 占用框架固定 key 的稳定失败。
    let conflict = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 构造启用箭头且 authored child 冲突的声明根。
        let root = ViewNode::new(
            // 工厂若被错误执行会留下调用记录。
            Carousel::new().arrows(move |_, _| {
                // 记录任何越过 authored key 预检的错误调用。
                *renderer_conflict_calls.borrow_mut() += 1;
                // 返回最小合法箭头根。
                ViewNode::leaf(Label::new("conflict-arrows"))
            }),
            // authored slide 故意占用固定动态身份。
            vec![
                // 使用产品保留 key 制造冲突。
                ViewNode::leaf(Label::new("conflict-slide"))
                    // 占用框架固定身份。
                    .key(Carousel::CUSTOM_ARROWS_CHILD_KEY),
            ],
        );
        // 真实建树必须在执行箭头工厂前拒绝。
        let _ = ViewAdapter::build(root);
    }));
    // authored key 冲突必须向调用方传播。
    assert!(conflict.is_err());
    // 预检不得执行箭头工厂。
    assert_eq!(*conflict_calls.borrow(), 0);
    // 先挂载没有自定义箭头的安全 Carousel。
    let mut tree = ViewAdapter::build(carousel_view(Carousel::new()));
    // 保存可重试 owner。
    let root = carousel_root(&tree);
    // 安装会返回自设根 key 的非法工厂。
    sync_runtime_carousel(
        // 修改当前 live owner。
        &mut tree,
        // 使用稳定 owner 身份。
        root,
        // 仅启用非法自定义箭头。
        Carousel::new().arrows(|_, _| {
            // 用户根 key 不得覆盖框架固定身份。
            ViewNode::leaf(Label::new("invalid-key")).key("factory-owned-key")
        }),
    );
    // 捕获发布前根 key 校验失败。
    let invalid_key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 首次刷新尚未改写真实子树。
        let _ = tree.refresh_carousel_custom_arrows_widget(root);
    }));
    // 非法工厂根 key 必须被明确拒绝。
    assert!(invalid_key.is_err());
    // 发布前失败不得使树进入 fail-stop。
    assert!(!tree.is_fail_stopped());
    // 非法工厂不得留下固定动态子节点。
    assert_eq!(
        tree.get(root)
            // owner 必须继续存在。
            .expect("Carousel owner 必须存在")
            // 读取全部 authored slides。
            .children()
            // 只有两个普通幻灯片。
            .len(),
        // 固定箭头没有发布。
        2
    );
    // 建立 provisional State 记录。
    let failed_states = Arc::new(Mutex::new(None));
    // 克隆记录供失败工厂持有。
    let renderer_failed_states = Arc::clone(&failed_states);
    // 安装会在返回 ViewNode 前 panic 的工厂。
    sync_runtime_carousel(
        // 修改同一可重试树。
        &mut tree,
        // 使用同一 owner。
        root,
        // 启用失败工厂。
        Carousel::new().arrows(move |_, _| -> ViewNode {
            // 声明与恢复工厂相同的私有 scope。
            let scope = uix_widget_scope("carousel-custom-arrows-pre-publish-panic-test", 1);
            // 创建尚未提交的 provisional State。
            let state = uix_widget_state(&scope, 1, || 0_i32);
            // 写入可辨识错误值。
            state.set(77);
            // 回传 provisional 句柄供异常后诊断。
            *renderer_failed_states
                // 取得记录锁。
                .lock()
                // 恢复 poisoned 锁数据。
                .unwrap_or_else(|error| error.into_inner()) = Some(state);
            // 在发布任何节点前触发异常。
            panic!("Carousel 箭头工厂发布前异常")
        }),
    );
    // 捕获工厂直接 panic。
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // 动态捕获应在 append 发布前失败。
        let _ = tree.refresh_carousel_custom_arrows_widget(root);
    }));
    // 工厂 panic 必须向调用方传播。
    assert!(failed.is_err());
    // 发布前异常不得 fail-stop 整棵树。
    assert!(!tree.is_fail_stopped());
    // provisional 句柄只保留局部写入值。
    assert_eq!(captured_state(&failed_states).get(), 77);
    // 建立安全恢复状态记录。
    let recovered_states = Arc::new(Mutex::new(None));
    // 克隆恢复记录供安全工厂持有。
    let renderer_recovered_states = Arc::clone(&recovered_states);
    // 在同一 owner 安装使用相同 scope 的安全工厂。
    sync_runtime_carousel(
        // 修改仍可服务的树。
        &mut tree,
        // 使用原 owner。
        root,
        // 启用恢复工厂。
        Carousel::new().arrows(move |_, _| {
            // 使用失败工厂相同的动态组件 scope。
            let scope = uix_widget_scope("carousel-custom-arrows-pre-publish-panic-test", 1);
            // 正确回滚后必须重新取得初值 State。
            let state = uix_widget_state(&scope, 1, || 0_i32);
            // 回传恢复状态句柄。
            *renderer_recovered_states
                // 取得记录锁。
                .lock()
                // 恢复 poisoned 锁数据。
                .unwrap_or_else(|error| error.into_inner()) = Some(state);
            // 返回承载同一 scope 的合法根。
            ViewNode::leaf(Label::new("recovered-arrows")).uix_widget_scope(scope, 0)
        }),
    );
    // 安全刷新必须成功发布固定动态箭头。
    assert!(tree.refresh_carousel_custom_arrows_widget(root));
    // 恢复不得复用失败 journal 写入的七十七。
    assert_eq!(captured_state(&recovered_states).get(), 0);
    // 保存已恢复的固定箭头身份。
    let _ = custom_arrows_child(&tree, root);
    // 真实移除 Carousel owner 使 generation 失效。
    tree.remove(root);
    // stale owner 必须在调用任何工厂前拒绝刷新。
    assert!(!tree.refresh_carousel_custom_arrows_widget(root));
}
