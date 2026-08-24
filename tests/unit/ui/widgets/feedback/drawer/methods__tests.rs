// 引入原子计数器验证受控写回次数。
use std::sync::Arc;
// 引入宽松内存序与原子无符号计数器。
use std::sync::atomic::{AtomicUsize, Ordering};

// 引入被测 Drawer 组件。
use super::{DRAWER_VISUAL_REF, Drawer};
// 引入受控状态公开契约。
use crate::ui::State;
// 引入事件与动画窄契约以验证 Escape 关闭路径。
use crate::ui::widget_runtime::traits::{EventHandler, WidgetAnimation};
// 引入键盘事件值。
use crate::ui::{KeyCode, KeyMod, SystemEvent};

// 验证外部状态能驱动 Drawer 的完整进退场。
#[test]
// 声明受控状态同步测试。
fn controlled_drawer_follows_external_state() {
    // 创建默认关闭的唯一业务状态源。
    let state = State::new(false);
    // 构造绑定该状态的 Drawer。
    let mut drawer = Drawer::new("").controlled_open(&state);
    // 初始假值不得呈现。
    assert!(!drawer.is_present());

    // 外部把业务事实切换为打开。
    state.set(true);
    // 下一动画帧同步受控状态。
    drawer.update_animation(0.016);
    // Drawer 应进入稳定打开态。
    assert!(drawer.is_visible());

    // 外部把业务事实切换为关闭。
    state.set(false);
    // 下一动画帧启动正常离场。
    drawer.update_animation(0.016);
    // 稳定可见性应立即关闭。
    assert!(!drawer.is_visible());
    // 离场期间仍保留呈现资格。
    assert!(drawer.is_present());
    // 推进足够时间完成离场。
    drawer.update_animation(10.0);
    // 离场完成后释放呈现资格。
    assert!(!drawer.is_present());
}

// 验证用户关闭只写回一次且离场不能重入。
#[test]
// 声明关闭写回幂等性测试。
fn controlled_user_close_writes_false_once() {
    // 创建初始打开的唯一业务状态源。
    let state = State::new(true);
    // 创建线程安全写回计数器。
    let writes = Arc::new(AtomicUsize::new(0));
    // 克隆计数器供 State 观察器持有。
    let observed_writes = writes.clone();
    // 监听每次真实 State::set。
    state.watch(move |_| {
        // 记录受控状态写入次数。
        observed_writes.fetch_add(1, Ordering::Relaxed);
    });
    // 构造初始呈现的受控 Drawer。
    let mut drawer = Drawer::new("").controlled_open(&state);

    // 用户路径请求关闭。
    drawer.close();
    // 离场期间重复关闭不得重启动画或再次写回。
    drawer.close();
    // 受控同步读取已写回的 false，不得形成反馈循环。
    drawer.update_animation(0.016);
    // 唯一业务状态应已经关闭。
    assert!(!state.get());
    // 整个关闭序列只允许一次写回。
    assert_eq!(writes.load(Ordering::Relaxed), 1);
    // 推进足够时间完成离场。
    drawer.update_animation(10.0);
    // Drawer 应完全退出呈现。
    assert!(!drawer.is_present());
}

// 验证 closable 只控制关闭按钮，不阻断与 Modal 一致的 Escape 关闭语义。
#[test]
fn escape_closes_drawer_when_close_button_is_hidden() {
    // 构造隐藏关闭按钮但已经呈现的 Drawer。
    let mut drawer = Drawer::new("").closable(false).show();
    // 发送不携带修饰键的 Escape。
    let result = EventHandler::on_event(
        // 把事件直接交给 Drawer 的组件边界。
        &mut drawer,
        // 使用标准键盘关闭事件。
        &SystemEvent::KeyDown {
            // Escape 必须独立于关闭按钮可见性。
            key: KeyCode::Escape,
            // 本用例不携带组合修饰键。
            mods: KeyMod::NONE,
        },
    );
    // Drawer 必须消费关闭手势。
    assert_eq!(result, crate::ui::EventResult::Handled);
    // 稳定可见性必须立即切换为关闭。
    assert!(!drawer.is_visible());
    // 离场动画期间仍保留呈现资格。
    assert!(drawer.is_present());
}

// 验证声明重建替换绑定时不会写入旧 State。
#[test]
// 声明受控绑定 reconcile 测试。
fn sync_from_adopts_new_controlled_state_without_old_write_back() {
    // 创建旧声明持有的打开状态。
    let old_state = State::new(true);
    // 创建新声明持有的关闭状态。
    let new_state = State::new(false);
    // 创建旧状态写入计数器。
    let old_writes = Arc::new(AtomicUsize::new(0));
    // 克隆计数器供观察器持有。
    let observed_old_writes = old_writes.clone();
    // 监听旧状态是否被错误写回。
    old_state.watch(move |_| {
        // 记录旧状态的每次写入。
        observed_old_writes.fetch_add(1, Ordering::Relaxed);
    });
    // 构造使用旧状态的运行节点。
    let mut drawer = Drawer::new("").controlled_open(&old_state);
    // 构造使用新状态的下一声明。
    let next = Drawer::new("").controlled_open(&new_state);

    // 同步声明应走内部离场并替换状态句柄。
    drawer.sync_from(next);
    // 旧状态不得因 reconcile 被组件反向修改。
    assert!(old_state.get());
    // 旧状态不得收到任何写入。
    assert_eq!(old_writes.load(Ordering::Relaxed), 0);

    // 新声明把期望值切换为打开。
    new_state.set(true);
    // 下一动画帧必须从新绑定读取事实。
    drawer.update_animation(0.016);
    // Drawer 应重新进入稳定打开态。
    assert!(drawer.is_visible());
}

// 验证宽度构建器复用统一尺寸归一化。
#[test]
// 声明宽度边界测试。
fn width_builder_normalizes_invalid_dimensions() {
    // 正数宽度应原样保存。
    assert_eq!(Drawer::new("").width(420.0).width, 420.0);
    // 负数宽度应归一化为零。
    assert_eq!(Drawer::new("").width(-1.0).width, 0.0);
    // 非有限宽度应归一化为零。
    assert_eq!(Drawer::new("").width(f32::NAN).width, 0.0);
}

#[test]
fn declarative_visibility_sync_updates_runtime_lifecycle() {
    // 创建关闭的运行态 Drawer，模拟 G5 初始 overlay。
    let mut drawer = Drawer::new("");
    // 构造声明式打开配置，模拟 overlay_mode 切换到 Drawer。
    let open = Drawer::new("").visible(true);
    // 将声明式打开同步到复用中的运行节点。
    drawer.sync_from(open);
    // 打开后运行态必须参与呈现。
    assert!(drawer.is_present());
    // 构造声明式关闭配置，模拟 overlay_mode 离开 Drawer。
    let close = Drawer::new("").visible(false);
    // 将声明式关闭同步到同一个运行节点。
    drawer.sync_from(close);
    // 关闭请求应进入离场状态，而不是重新打开。
    assert!(drawer.is_present());
    // 再次同步相同关闭声明，验证离场动画不会被重复启动。
    drawer.sync_from(Drawer::new("").visible(false));
    // 推进足够长的时间完成离场动画。
    drawer.update_animation(10.0);
    // 离场完成后运行态必须完全释放呈现资格。
    assert!(!drawer.is_present());
}

// 验证直接构造和声明式 View 都共享 UIX 生成的唯一视觉静态项。
#[test]
fn view_build_uses_shared_uix_visual() {
    // 直接构造路径不得拥有第二份 Rust 默认表。
    let drawer = Drawer::new("标题");
    assert!(std::ptr::eq(drawer.visual, DRAWER_VISUAL_REF));

    // View 构建必须实际经过同目录 UIX 根并保留 Drawer 行为内核。
    let node = crate::ui::view::View::build(drawer);
    let drawer = node
        .widget
        .as_any()
        .downcast_ref::<Drawer>()
        .expect("UIX 根必须保留 Drawer Rust 内核");
    assert!(std::ptr::eq(drawer.visual, DRAWER_VISUAL_REF));
}

// 验证三档尺寸、面板分区和静态文案仅由 UIX 视觉项提供。
#[test]
fn uix_visual_preserves_drawer_defaults_and_geometry() {
    use crate::platform::windowing::ControlSize;

    assert_eq!(
        DRAWER_VISUAL_REF.defaults.dimensions(ControlSize::Small),
        (300.0, 200.0)
    );
    assert_eq!(
        DRAWER_VISUAL_REF.defaults.dimensions(ControlSize::Medium),
        (378.0, 300.0)
    );
    assert_eq!(
        DRAWER_VISUAL_REF.defaults.dimensions(ControlSize::Large),
        (600.0, 450.0)
    );
    assert_eq!(DRAWER_VISUAL_REF.layout.header_height, 48.0);
    assert_eq!(DRAWER_VISUAL_REF.layout.footer_height, 56.0);
    assert_eq!(DRAWER_VISUAL_REF.layout.body_padding, 24.0);
    assert_eq!(DRAWER_VISUAL_REF.chrome.trigger_label, "打开 Drawer");
}

// 验证声明重建会同步效果请求，不沿用旧实例的 backdrop blur。
#[test]
fn sync_from_adopts_new_backdrop_blur_request() {
    let mut drawer = Drawer::new("");
    let blur = crate::ui::OverlayBackdropBlur::radius(18.0);
    drawer.sync_from(Drawer::new("").backdrop_blur(blur));
    assert_eq!(drawer.backdrop_blur, Some(blur));
}
