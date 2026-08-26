// 引入被测 damage 规划类型。
use super::{
    DamageRegion, DirtyRegion, PresentCoherency, PresentDamage, PresentDamageTracker, PresentImage,
    PresentSurface,
};
// 引入逻辑矩形值。
use crate::core::Rect;

// 构造单位 DPR 的稳定测试 surface。
fn surface(generation: u64) -> PresentSurface {
    // 使用 100×100 物理 drawable，便于直接断言坐标。
    PresentSurface::identity(100, 100, 1.0, generation)
}

// 构造单矩形逻辑 damage。
fn damage(x: f32, y: f32) -> DamageRegion {
    // 每个区域固定为互不接触的 5×5 矩形。
    DamageRegion::from_rect(Rect::new(x, y, 5.0, 5.0))
}

// 验证覆盖矩形原地替换冗余子区域，供滚动 viewport 复用 exposed strip 容量。
#[test]
fn dirty_region_replaces_fully_covered_rectangles() {
    let mut region = DirtyRegion::area(Rect::new(0.0, 36.0, 64.0, 4.0));
    let capacity = region.rects.capacity();
    region.add_rect(Rect::new(0.0, 0.0, 64.0, 40.0));
    assert_eq!(region.rects(), &[Rect::new(0.0, 0.0, 64.0, 40.0)]);
    assert_eq!(region.rects.capacity(), capacity);

    // 已被现有 viewport 覆盖的子区域不应重新进入集合。
    region.add_rect(Rect::new(4.0, 4.0, 8.0, 8.0));
    assert_eq!(region.rects().len(), 1);
}

// 验证双缓冲轮换会修复当前 image 错过的已提交历史。
#[test]
fn tracked_swapchain_repairs_missed_image_history() {
    // 创建尚无成功提交历史的 tracker。
    let mut tracker = PresentDamageTracker::new();
    // 声明双缓冲 image 0。
    let image0 = Some(PresentImage::new(0, 2));
    // 声明双缓冲 image 1。
    let image1 = Some(PresentImage::new(1, 2));
    // 首次看到 image 0 时必须完整绘制。
    assert_eq!(
        tracker
            .plan(
                PresentCoherency::TrackedSwapchain,
                surface(1),
                image0,
                &damage(0.0, 0.0),
            )
            .draw_damage,
        PresentDamage::Full
    );
    // 模拟 image 0 的最终 native present 成功。
    tracker.commit(
        PresentCoherency::TrackedSwapchain,
        surface(1),
        image0,
        &damage(0.0, 0.0),
    );
    // 首次看到 image 1 时同样必须完整绘制。
    assert_eq!(
        tracker
            .plan(
                PresentCoherency::TrackedSwapchain,
                surface(1),
                image1,
                &damage(20.0, 20.0),
            )
            .draw_damage,
        PresentDamage::Full
    );
    // 模拟 image 1 的最终 native present 成功。
    tracker.commit(
        PresentCoherency::TrackedSwapchain,
        surface(1),
        image1,
        &damage(20.0, 20.0),
    );
    // image 0 再次可写时必须合并它错过的 image 1 damage 与当前 damage。
    let plan = tracker.plan(
        PresentCoherency::TrackedSwapchain,
        surface(1),
        image0,
        &damage(40.0, 40.0),
    );
    // draw 和 present 必须消费同一个修复集合。
    assert_eq!(plan.draw_damage, plan.present_damage);
    // 互不接触的两个矩形应保持离散，避免多写 dirty rect 外像素。
    assert_eq!(
        plan.draw_damage,
        PresentDamage::Partial(vec![(20, 20, 5, 5), (40, 40, 5, 5)])
    );
}

// 验证失败或遮挡帧不调用 commit 时不会污染 image history。
#[test]
fn failed_present_without_commit_does_not_advance_history() {
    // 创建 tracker 与两个 image 身份。
    let mut tracker = PresentDamageTracker::new();
    // 保存 image 0 身份。
    let image0 = Some(PresentImage::new(0, 2));
    // 保存 image 1 身份。
    let image1 = Some(PresentImage::new(1, 2));
    // 建立 image 0 的成功基线。
    tracker.commit(
        PresentCoherency::TrackedSwapchain,
        surface(1),
        image0,
        &damage(0.0, 0.0),
    );
    // 建立 image 1 的后续成功基线。
    tracker.commit(
        PresentCoherency::TrackedSwapchain,
        surface(1),
        image1,
        &damage(20.0, 20.0),
    );
    // 规划一个随后因 occluded/device-lost 失败的 image 0 帧。
    let failed = tracker.plan(
        PresentCoherency::TrackedSwapchain,
        surface(1),
        image0,
        &damage(40.0, 40.0),
    );
    // 失败帧原本需要历史修复。
    assert_eq!(
        failed.draw_damage,
        PresentDamage::Partial(vec![(20, 20, 5, 5), (40, 40, 5, 5)])
    );
    // 故意不调用 commit，模拟 native present 失败。
    let retried = tracker.plan(
        PresentCoherency::TrackedSwapchain,
        surface(1),
        image0,
        &damage(60.0, 60.0),
    );
    // 重试仍须修复最后一次成功后错过的 image 1 damage。
    assert_eq!(
        retried.draw_damage,
        PresentDamage::Partial(vec![(20, 20, 5, 5), (60, 60, 5, 5)])
    );
}

// 验证 resize/recreate 的 generation 变化会清除旧 image 证明。
#[test]
fn surface_rebuild_forces_full_present() {
    // 创建 tracker 并提交第一代 image 0。
    let mut tracker = PresentDamageTracker::new();
    // 保存双缓冲 image 0 身份。
    let image0 = Some(PresentImage::new(0, 2));
    // 提交第一代成功帧。
    tracker.commit(
        PresentCoherency::TrackedSwapchain,
        surface(1),
        image0,
        &damage(0.0, 0.0),
    );
    // 同一 image index 在新 surface generation 中不得复用旧历史。
    let plan = tracker.plan(
        PresentCoherency::TrackedSwapchain,
        surface(2),
        image0,
        &damage(20.0, 20.0),
    );
    // resize/recreate 后第一帧必须完整绘制和提交。
    assert_eq!(plan.draw_damage, PresentDamage::Full);
    // present damage 同样必须完整。
    assert_eq!(plan.present_damage, PresentDamage::Full);
}

// 验证准备令牌只在成功提交时推进历史，失败丢弃不会污染后续修复集合。
#[test]
fn prepared_present_commits_once_and_failed_token_is_discarded() {
    // 创建 tracker 与双缓冲身份。
    let mut tracker = PresentDamageTracker::new();
    let image0 = Some(PresentImage::new(0, 2));
    let image1 = Some(PresentImage::new(1, 2));
    // 两个成功令牌建立各自 image 的基线。
    let first = tracker.prepare(
        PresentCoherency::TrackedSwapchain,
        surface(1),
        image0,
        &damage(0.0, 0.0),
    );
    let (_, first_commit) = first.into_parts();
    tracker.commit_prepared(first_commit);
    let second = tracker.prepare(
        PresentCoherency::TrackedSwapchain,
        surface(1),
        image1,
        &damage(20.0, 20.0),
    );
    let (_, second_commit) = second.into_parts();
    tracker.commit_prepared(second_commit);
    // 规划后直接丢弃，模拟原生 present 失败。
    let failed = tracker.prepare(
        PresentCoherency::TrackedSwapchain,
        surface(1),
        image0,
        &damage(40.0, 40.0),
    );
    let (failed_plan, failed_commit) = failed.into_parts();
    assert_eq!(
        failed_plan.draw_damage,
        PresentDamage::Partial(vec![(20, 20, 5, 5), (40, 40, 5, 5)])
    );
    drop(failed_commit);
    // 下一次准备仍只基于最后成功的 image 1 历史，不包含失败帧损伤。
    let retried = tracker.prepare(
        PresentCoherency::TrackedSwapchain,
        surface(1),
        image0,
        &damage(60.0, 60.0),
    );
    let (retried_plan, retried_commit) = retried.into_parts();
    assert_eq!(
        retried_plan.draw_damage,
        PresentDamage::Partial(vec![(20, 20, 5, 5), (60, 60, 5, 5)])
    );
    tracker.commit_prepared(retried_commit);
}
