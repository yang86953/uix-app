// 复用父模块私有 Image 与加载状态以驱动确定性生命周期转换。
use super::{Image, ImageLoadState};
// 导入受控状态切换所需的最小矩形与 slot 句柄。
use crate::core::Rect;
// 导入位图句柄以构造确定性 source identity 变化。
use crate::draw::resources::image::BitmapHandle;
// 导入真实建树、协调与动态 owner 生命周期入口。
use crate::ui::adapter::ViewAdapter;
// 导入错误工厂中的组件私有状态 scope 与槽创建入口。
use crate::ui::component_state::{uix_component_scope, uix_component_state};
// 导入读取节点组件、父子关系与离场状态的核心契约。
use crate::ui::widget_runtime::widget::WidgetCore;
// 导入声明错误子树的 ViewNode。
use crate::ui::view::ViewNode;
// 导入最小占位与错误子组件。
use crate::ui::widgets::Label;
// 导入动画、Effect、状态、过渡与树级身份类型。
use crate::ui::{Animated, ComponentId, Easing, Effect, State, Transition, WidgetTree};
// 导入调用与副作用次数的原子记录。
use std::sync::atomic::{AtomicUsize, Ordering};
// 导入跨工厂协调轮次共享的测试记录。
use std::sync::{Arc, Mutex};

// 读取 Image 树的稳定运行时 owner。
fn image_root(
    // 接收已经成功挂载的运行时树。
    tree: &WidgetTree,
    // 返回当前 Image 根身份。
) -> ComponentId {
    // 每个测试都只创建一个声明根。
    tree.root_id().expect("Image 必须拥有运行时根")
}

// 从运行时节点恢复私有 Image 引用。
fn runtime_image(
    // 接收拥有目标 Image 的运行时树。
    tree: &WidgetTree,
    // 接收目标 Image 的实际组件身份。
    root: ComponentId,
    // 返回只读 Image 组件引用。
) -> &Image {
    // 目标节点在正常测试阶段必须仍可寻址。
    tree.get(root)
        // 缺失 owner 属于产品生命周期错误。
        .expect("Image owner 必须存在")
        // 取得节点当前组件。
        .component()
        // 进入类型擦除只读接口。
        .as_any()
        // 恢复 Image 私有类型。
        .downcast_ref::<Image>()
        // 根类型不得在同一测试阶段改变。
        .expect("运行时根必须是 Image")
}

// 将运行时 Image 切换到指定加载状态。
fn set_load_state(
    // 接收拥有 Image owner 的运行时树。
    tree: &WidgetTree,
    // 接收目标 Image 身份。
    root: ComponentId,
    // 接收本轮确定性加载状态。
    state: ImageLoadState,
) {
    // 通过私有入口写入状态并保留真实失效语义。
    runtime_image(tree, root).set_load_state(
        // 交付测试指定的下一状态。
        state,
        // 失效请求归属当前树。
        tree,
        // 固定有效 frame，避免依赖布局执行时序。
        Rect::new(0.0, 0.0, 100.0, 100.0),
    );
}

// 在 Image 直接子节点中按稳定 key 查找身份。
fn child_with_key(
    // 接收运行时树。
    tree: &WidgetTree,
    // 接收 Image owner 身份。
    root: ComponentId,
    // 接收产品定义的直接子节点 key。
    key: &str,
    // 返回当前匹配子节点身份。
) -> ComponentId {
    // 遍历 Image 当前直接子节点。
    tree.get(root)
        // Image owner 必须仍然存在。
        .expect("Image owner 必须存在")
        // 读取稳定父子顺序。
        .children()
        // 逐个检查直接子节点。
        .iter()
        // 复制轻量组件身份。
        .copied()
        // 按运行时 key 精确匹配。
        .find(|child| tree.get(*child).and_then(|node| node.key()) == Some(key))
        // 目标动态子节点必须已经物化。
        .expect("目标 Image 子节点必须存在")
}

// 读取工厂最近一次回传的私有状态句柄。
fn captured_state(
    // 接收跨协调轮次共享的状态记录。
    states: &Arc<Mutex<Option<State<i32>>>>,
    // 返回最近捕获的私有状态句柄。
) -> State<i32> {
    // 锁定记录只覆盖句柄克隆。
    states
        // 进入共享记录。
        .lock()
        // poison 后恢复内部值以保留失败诊断。
        .unwrap_or_else(|error| error.into_inner())
        // 借用最近状态句柄。
        .as_ref()
        // 克隆公开句柄以脱离锁。
        .cloned()
        // 实际工厂调用必须已经回传状态。
        .expect("错误工厂必须捕获私有状态")
}

// 构造携带 State、Effect、Animated 与可选 leave 的错误 View。
fn error_view(
    // 接收错误节点显示文本以区分 factory 版本。
    label: &'static str,
    // 接收组件私有 scope 名称。
    scope_name: &'static str,
    // 接收回传私有状态句柄的记录。
    states: &Arc<Mutex<Option<State<i32>>>>,
    // 接收 Effect 依赖。
    effect_dependency: &State<i32>,
    // 接收 Effect 运行次数记录。
    effect_runs: &Arc<AtomicUsize>,
    // 接收需要由错误节点拥有的动画源。
    animation: &Animated<f32>,
    // 接收可选非零离场过渡。
    leave: Option<Transition>,
    // 返回完整捕获输出的错误声明节点。
) -> ViewNode {
    // 为当前错误子组件声明稳定 scope marker。
    let scope = uix_component_scope(scope_name, 1);
    // 在 Image 固定动态命名空间中取得组件私有状态。
    let state = uix_component_state(&scope, 1, || 0_i32);
    // 保存本轮捕获状态句柄。
    *states
        // 锁定共享记录只覆盖句柄写入。
        .lock()
        // poison 后恢复内部值以保留失败诊断。
        .unwrap_or_else(|error| error.into_inner()) = Some(state);
    // 克隆 Effect 依赖供副作用闭包长期拥有。
    let effect_dependency = effect_dependency.clone();
    // 克隆运行记录供副作用闭包长期拥有。
    let effect_runs = Arc::clone(effect_runs);
    // 创建归属错误子节点生命周期的 Effect。
    let _ = Effect::new(move || {
        // 读取依赖以建立可观察订阅。
        let _ = effect_dependency.get();
        // 记录首次及后续 Effect 运行。
        effect_runs.fetch_add(1, Ordering::Relaxed);
    });
    // 读取动画值以让错误子节点捕获该工作源。
    let _ = animation.value();
    // 构造带 scope 的最小错误节点。
    let node = ViewNode::leaf(Label::new(label))
        // 让真实节点声明私有状态 scope。
        .uix_component_scope(scope, 0);
    // 仅在测试要求离场时追加非零过渡。
    match leave {
        // 非零 leave 使错误清除后节点保留为 pending removal。
        Some(leave) => node.leave(leave),
        // 缺失 leave 时保持立即移除语义。
        None => node,
    }
}

// 验证 placeholder 初建身份不受 Error 追加或解除影响。
#[test]
// 错误解除只能移除固定 error 子节点而不能重建 placeholder。
fn image_placeholder_identity_survives_error_append_and_clear() {
    // 构造同时拥有 placeholder 与错误工厂的 Image。
    let image = Image::new(100.0, 100.0)
        // 初建时物化稳定 placeholder 子节点。
        .placeholder(ViewNode::leaf(Label::new("placeholder")))
        // 错误状态出现后追加独立 error 子节点。
        .on_error(|_| ViewNode::leaf(Label::new("error")));
    // 通过真实建树路径挂载 Image 与 placeholder。
    let mut tree = ViewAdapter::build(ViewNode::leaf(image));
    // 读取稳定 Image owner。
    let root = image_root(&tree);
    // 保存初建 placeholder 身份。
    let placeholder = child_with_key(&tree, root, "uix:image:placeholder");
    // 进入确定性 Error 状态。
    set_load_state(
        // 使用当前运行时树。
        &tree,
        // 更新同一 Image owner。
        root,
        // 使用稳定错误文本。
        ImageLoadState::Error("controlled error".to_owned()),
    );
    // Error 刷新必须追加错误子树。
    assert!(tree.refresh_image_error_component(root));
    // 追加后 placeholder 继续复用初建身份。
    assert_eq!(
        child_with_key(&tree, root, "uix:image:placeholder"),
        placeholder
    );
    // 保存独立 error 子节点身份。
    let error_child = child_with_key(&tree, root, Image::ERROR_CHILD_KEY);
    // Error 解除为 Empty。
    set_load_state(&tree, root, ImageLoadState::Empty);
    // 刷新必须只移除错误子树。
    assert!(tree.refresh_image_error_component(root));
    // placeholder 身份在错误解除后仍保持不变。
    assert_eq!(
        child_with_key(&tree, root, "uix:image:placeholder"),
        placeholder
    );
    // 无 leave 的错误子节点应立即真实释放。
    assert!(tree.get(error_child).is_none());
    // Image 最终只保留原 placeholder 子节点。
    assert_eq!(
        tree.get(root).expect("Image owner 必须存在").children(),
        &[placeholder]
    );
}

// 验证非零 leave 期间错误子树保持可见资源并允许同 key 重入复用。
#[test]
// Error 重入必须取消 pending removal 且保留同一 id 与 State=23。
fn image_error_leave_reentry_reuses_visible_child_and_private_state() {
    // 建立私有状态记录。
    let states = Arc::new(Mutex::new(None));
    // 建立 Effect 依赖。
    let effect_dependency = State::new(0_i32);
    // 建立 Effect 运行记录。
    let effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立错误子树动画源。
    let animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 克隆状态记录供错误工厂拥有。
    let renderer_states = Arc::clone(&states);
    // 克隆 Effect 依赖供错误工厂拥有。
    let renderer_effect_dependency = effect_dependency.clone();
    // 克隆 Effect 记录供错误工厂拥有。
    let renderer_effect_runs = Arc::clone(&effect_runs);
    // 克隆动画供错误工厂拥有。
    let renderer_animation = animation.clone();
    // 构造带非零 leave 的错误子树。
    let image = Image::new(100.0, 100.0).on_error(move |_| {
        // 返回完整捕获且具有长离场过渡的错误节点。
        error_view(
            // 使用稳定错误文本。
            "leaving-error",
            // 使用本测试专属私有 scope。
            "image-error-leave-reentry-test",
            // 回传状态句柄。
            &renderer_states,
            // 建立 Effect 订阅。
            &renderer_effect_dependency,
            // 记录 Effect 运行。
            &renderer_effect_runs,
            // 捕获动画源。
            &renderer_animation,
            // 十秒离场保证测试期间不会自然完成。
            Some(Transition::fade_out(10.0)),
        )
    });
    // 挂载尚未错误的 Image。
    let mut tree = ViewAdapter::build(ViewNode::leaf(image));
    // 保存稳定 Image owner。
    let root = image_root(&tree);
    // 进入 Error 状态。
    set_load_state(
        // 使用当前运行时树。
        &tree,
        // 更新同一 Image owner。
        root,
        // 使用固定错误实例。
        ImageLoadState::Error("controlled error".to_owned()),
    );
    // 首次刷新物化错误子树。
    assert!(tree.refresh_image_error_component(root));
    // 保存错误子树身份。
    let child = child_with_key(&tree, root, Image::ERROR_CHILD_KEY);
    // 写入可区分私有状态。
    captured_state(&states).set(23);
    // 初始错误子树必须拥有一个动画注册。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 解除 Error 状态。
    set_load_state(&tree, root, ImageLoadState::Empty);
    // 刷新应启动非零 leave。
    assert!(tree.refresh_image_error_component(root));
    // 错误子树必须进入 pending removal。
    assert!(
        tree.get(child)
            .expect("离场错误子节点必须存在")
            .pending_removal()
    );
    // 执行真实布局，让 Image::child_visible 同步到子节点的父级可见门。
    tree.layout();
    // pending 错误子节点仍保留自身 visible 标志供离场绘制。
    assert!(tree.get(child).expect("离场错误子节点必须存在").visible());
    // 离场期间动画源必须继续由同一节点拥有。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 修改旧 Effect 依赖。
    effect_dependency.set(1);
    // pending 子树的 Effect 资源仍保留到真实 remove。
    assert!(tree.has_pending_effects());
    // 重入前恢复相同 Error 实例。
    set_load_state(
        // 使用当前运行时树。
        &tree,
        // 更新同一 Image owner。
        root,
        // 使用相同稳定错误文本。
        ImageLoadState::Error("controlled error".to_owned()),
    );
    // 重入刷新必须取消离场。
    assert!(tree.refresh_image_error_component(root));
    // 同 key 重入继续复用原错误子节点身份。
    assert_eq!(child_with_key(&tree, root, Image::ERROR_CHILD_KEY), child);
    // 重入后节点不再 pending removal。
    assert!(
        !tree
            .get(child)
            .expect("恢复错误子节点必须存在")
            .pending_removal()
    );
    // 重入后节点继续可见。
    assert!(tree.get(child).expect("恢复错误子节点必须存在").visible());
    // 重入不得重建或重置组件私有状态。
    assert_eq!(captured_state(&states).get(), 23);
    // 重入继续保留同一动画源注册。
    assert_eq!(tree.animated_source_registrations().len(), 1);
}

// 验证同源父协调采用最新工厂，同时固定 identity 保留 State 并替换运行时输出。
#[test]
// source 变化或 handler 关闭随后必须移除错误子树并释放新版资源。
fn image_parent_reconcile_replaces_factory_outputs_and_source_reset_releases_child() {
    // 建立首版工厂私有状态记录。
    let first_states = Arc::new(Mutex::new(None));
    // 建立首版 Effect 依赖。
    let first_effect_dependency = State::new(0_i32);
    // 建立首版 Effect 运行记录。
    let first_effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立首版动画源。
    let first_animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 克隆首版记录供工厂拥有。
    let renderer_first_states = Arc::clone(&first_states);
    // 克隆首版依赖供工厂拥有。
    let renderer_first_dependency = first_effect_dependency.clone();
    // 克隆首版 Effect 记录供工厂拥有。
    let renderer_first_runs = Arc::clone(&first_effect_runs);
    // 克隆首版动画供工厂拥有。
    let renderer_first_animation = first_animation.clone();
    // 构造首版 Image 错误工厂。
    let first_image = Image::new(100.0, 100.0).on_error(move |_| {
        // 返回首版完整错误子树。
        error_view(
            // 用文本区分首版工厂输出。
            "first",
            // 两版工厂使用相同组件 scope 以保持 state identity。
            "image-error-factory-replacement-test",
            // 回传首版状态。
            &renderer_first_states,
            // 建立首版 Effect。
            &renderer_first_dependency,
            // 记录首版 Effect。
            &renderer_first_runs,
            // 捕获首版动画。
            &renderer_first_animation,
            // 本场景使用立即移除语义。
            None,
        )
    });
    // 挂载首版 Image。
    let mut tree = ViewAdapter::build(ViewNode::leaf(first_image));
    // 保存稳定 Image owner。
    let root = image_root(&tree);
    // 进入 Error 状态。
    set_load_state(
        // 使用当前运行时树。
        &tree,
        // 更新同一 owner。
        root,
        // 使用固定错误实例。
        ImageLoadState::Error("controlled error".to_owned()),
    );
    // 物化首版错误子树。
    assert!(tree.refresh_image_error_component(root));
    // 保存固定错误子节点 identity。
    let child = child_with_key(&tree, root, Image::ERROR_CHILD_KEY);
    // 写入跨工厂替换应保留的私有状态。
    captured_state(&first_states).set(23);
    // 首版 Effect 已执行一次。
    assert_eq!(first_effect_runs.load(Ordering::Relaxed), 1);
    // 首版动画已注册。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 保存首版动画工作身份，后续必须由新版源替换而非仅维持数量。
    let first_animation_id = tree.animated_source_registrations()[0].0;
    // 建立新版状态句柄记录。
    let next_states = Arc::new(Mutex::new(None));
    // 建立新版 Effect 依赖。
    let next_effect_dependency = State::new(0_i32);
    // 建立新版 Effect 运行记录。
    let next_effect_runs = Arc::new(AtomicUsize::new(0));
    // 建立新版动画源。
    let next_animation = Animated::new(0.0_f32).to(1.0, 10.0, Easing::linear);
    // 克隆新版状态记录供工厂拥有。
    let renderer_next_states = Arc::clone(&next_states);
    // 克隆新版依赖供工厂拥有。
    let renderer_next_dependency = next_effect_dependency.clone();
    // 克隆新版 Effect 记录供工厂拥有。
    let renderer_next_runs = Arc::clone(&next_effect_runs);
    // 克隆新版动画供工厂拥有。
    let renderer_next_animation = next_animation.clone();
    // 用最新工厂原位协调同源 Image。
    ViewAdapter::reconcile(
        // 协调现有运行时树。
        &mut tree,
        // 保持相同 Image source identity 但替换应用工厂。
        ViewNode::leaf(Image::new(100.0, 100.0).on_error(move |_| {
            // 返回新版完整错误子树。
            error_view(
                // 用文本区分新版工厂输出。
                "next",
                // 使用相同组件 scope 保持私有 State identity。
                "image-error-factory-replacement-test",
                // 回传新版状态句柄。
                &renderer_next_states,
                // 建立新版 Effect。
                &renderer_next_dependency,
                // 记录新版 Effect。
                &renderer_next_runs,
                // 捕获新版动画源。
                &renderer_next_animation,
                // 本场景使用立即移除语义。
                None,
            )
        })),
    );
    // 固定错误子树 key 必须复用原 ComponentId。
    assert_eq!(child_with_key(&tree, root, Image::ERROR_CHILD_KEY), child);
    // 新工厂必须取得同一已提交私有 State 值。
    assert_eq!(captured_state(&next_states).get(), 23);
    // 新版 Effect 必须完成首次运行。
    assert_eq!(next_effect_runs.load(Ordering::Relaxed), 1);
    // 触发首版旧 Effect 依赖。
    first_effect_dependency.set(1);
    // 旧 Effect 已被节点原位协调替换，不得再进入 pending 集合。
    assert!(!tree.has_pending_effects());
    // 触发新版 Effect 依赖。
    next_effect_dependency.set(1);
    // 新版 Effect 必须仍归当前错误节点拥有。
    assert!(tree.has_pending_effects());
    // 消费新版 Effect 调度。
    assert!(tree.tick_effects());
    // 新版 Effect 恰好响应一次依赖变化。
    assert_eq!(next_effect_runs.load(Ordering::Relaxed), 2);
    // 动画注册表只保留新版节点输出而非叠加首版源。
    assert_eq!(tree.animated_source_registrations().len(), 1);
    // 新版工厂必须真正替换旧动画工作身份。
    assert_ne!(
        tree.animated_source_registrations()[0].0,
        first_animation_id
    );
    // 原位协调为新的 slot source identity，同时保留 handler 供移除路径判断。
    ViewAdapter::reconcile(
        // 协调同一运行时 Image owner。
        &mut tree,
        // slot 变化必须清除当前错误状态并移除错误子树。
        ViewNode::leaf(
            // 把更新后的 Image 组件包装为适配器可消费的声明节点。
            Image::new(100.0, 100.0)
                // 使用确定性不同 slot identity。
                .slot(BitmapHandle(1))
                // 保持错误 handler 启用以证明移除由 source reset 驱动。
                .on_error(|_| ViewNode::leaf(Label::new("unused"))),
        ),
    );
    // source reset 后错误子树必须立即移除。
    assert!(tree.get(child).is_none());
    // 新版动画源必须随真实子树移除释放。
    assert!(tree.animated_source_registrations().is_empty());
    // 清除前一轮 Effect 调度标记。
    let _ = tree.tick_effects();
    // 再次修改新版 Effect 依赖。
    next_effect_dependency.set(2);
    // 已移除错误节点不得继续持有新版 Effect。
    assert!(!tree.has_pending_effects());
}

// 验证真实 owner remove、正常 shutdown 与 owner pending-removal 的生命周期边界。
#[test]
// 离场 owner 的刷新必须拒绝且不调用工厂，最终 teardown 释放工厂资源。
fn image_owner_removal_shutdown_and_pending_refresh_release_and_reject() {
    // 建立 owner 离场场景的工厂调用记录。
    let factory_calls = Arc::new(AtomicUsize::new(0));
    // 建立只由测试与 Image 工厂共享的应用资源。
    let resource = Arc::new(());
    // 保存弱引用以观察 owner teardown 是否释放工厂捕获。
    let weak_resource = Arc::downgrade(&resource);
    // 克隆调用记录供工厂拥有。
    let renderer_calls = Arc::clone(&factory_calls);
    // 克隆应用资源供工厂闭包长期持有。
    let renderer_resource = Arc::clone(&resource);
    // 构造自身具有非零 leave 的 Image owner。
    let image = Image::new(100.0, 100.0)
        // 注册可观察但不应在 owner 离场期间调用的错误工厂。
        .on_error(move |_| {
            // 记录任何实际工厂调用。
            renderer_calls.fetch_add(1, Ordering::Relaxed);
            // 读取资源计数确保闭包真实捕获该资源。
            let _ = Arc::strong_count(&renderer_resource);
            // 返回最小错误节点。
            ViewNode::leaf(Label::new("error"))
        });
    // 用容器式声明节点包裹 Image 并设置 owner leave。
    let image = ViewNode::leaf(image)
        // 十秒离场保证拒绝刷新断言期间 owner 仍 pending。
        .leave(Transition::fade_out(10.0));
    // 挂载 Image owner。
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| image));
    // 保存 owner 身份。
    let root = image_root(&tree);
    // 释放测试侧强引用，仅让工厂闭包继续持有资源。
    drop(resource);
    // owner 存活时工厂资源必须仍然存在。
    assert!(weak_resource.upgrade().is_some());
    // 先进入 Error，使缺少 owner 离场门禁时刷新必然会调用应用工厂。
    set_load_state(
        // 使用当前运行时树。
        &tree,
        // 更新即将离场的 Image owner。
        root,
        // 使用确定性错误文本建立真实 renderer 需求。
        ImageLoadState::Error("owner pending error".to_owned()),
    );
    // 让根 owner 进入 pending removal。
    assert!(tree.start_leave_transition(root));
    // owner 必须进入离场状态。
    assert!(
        tree.get(root)
            .expect("离场 Image owner 必须存在")
            .pending_removal()
    );
    // owner 自身 pending-removal 时动态刷新必须拒绝。
    assert!(!tree.refresh_image_error_component(root));
    // 被拒绝刷新不得调用应用工厂。
    assert_eq!(factory_calls.load(Ordering::Relaxed), 0);
    // 显式真实 remove owner。
    tree.remove(root);
    // owner 移除后工厂捕获资源必须立即释放。
    assert!(weak_resource.upgrade().is_none());
    // 为正常 shutdown 建立另一份可观察工厂资源。
    let shutdown_resource = Arc::new(());
    // 保存正常关闭场景的弱引用。
    let weak_shutdown_resource = Arc::downgrade(&shutdown_resource);
    // 克隆资源供第二棵树的 Image 工厂拥有。
    let renderer_shutdown_resource = Arc::clone(&shutdown_resource);
    // 建立正常运行的独立 Image 树。
    let mut shutdown_tree = ViewAdapter::build(
        // 工厂持有资源直到 WidgetTree 正常关闭。
        ViewNode::leaf(Image::new(100.0, 100.0).on_error(move |_| {
            // 读取资源计数确保闭包真实捕获。
            let _ = Arc::strong_count(&renderer_shutdown_resource);
            // 返回最小错误节点。
            ViewNode::leaf(Label::new("shutdown-error"))
        })),
    );
    // 释放测试侧强引用。
    drop(shutdown_resource);
    // shutdown 前 Image 工厂仍持有资源。
    assert!(weak_shutdown_resource.upgrade().is_some());
    // 正常关闭完整运行树。
    shutdown_tree.shutdown();
    // shutdown 必须释放 Image 组件及其工厂捕获。
    assert!(weak_shutdown_resource.upgrade().is_none());
}
