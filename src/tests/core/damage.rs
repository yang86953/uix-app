use crate::core::damage::*;
use crate::tests::common::*;

#[test]
fn for_paint_clear_keeps_single_rect() {
    let region = DirtyRegion::area(Rect::new(10.0, 20.0, 30.0, 40.0));
    let paint = region.for_paint_clear();
    assert_eq!(paint.rects(), &[Rect::new(10.0, 20.0, 30.0, 40.0)]);
    assert!(!paint.full_frame);
}

#[test]
fn for_paint_clear_keeps_disjoint_rects_for_split_redraw() {
    let mut region = DirtyRegion::empty();
    region.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
    region.add_rect(Rect::new(0.0, 90.0, 10.0, 10.0));
    let paint = region.for_paint_clear();
    assert_eq!(paint.rects().len(), 2);
    assert_eq!(paint.rects()[0], Rect::new(0.0, 0.0, 10.0, 10.0));
    assert_eq!(paint.rects()[1], Rect::new(0.0, 90.0, 10.0, 10.0));
    // 空隙不在 dirty 集合内：逐矩形 clip 绘制时父背景不得填入中间干净像素
    assert!(!paint.intersects(Rect::new(0.0, 40.0, 10.0, 10.0)));
}

fn present_surface(dpr: f32) -> PresentSurface {
    PresentSurface::identity(200, 120, dpr, 1)
}

fn single_damage(x: f32, y: f32) -> DamageRegion {
    DamageRegion::from_rect(Rect::new(x, y, 4.0, 4.0))
}

#[test]
fn logical_damage_expands_fractional_edges_for_common_dpr_values() {
    let damage = DamageRegion::from_rect(Rect::new(1.2, 2.2, 2.0, 3.0));
    let cases = [
        (1.25, (1, 2, 3, 5)),
        (1.5, (1, 3, 4, 5)),
        (2.0, (2, 4, 5, 7)),
    ];

    for (dpr, expected) in cases {
        assert_eq!(
            damage.to_present_damage(present_surface(dpr)),
            PresentDamage::Partial(vec![expected]),
            "dpr={dpr}"
        );
    }
}

#[test]
fn logical_damage_clips_expanded_edges_to_drawable_extent() {
    let damage = DamageRegion::from_rect(Rect::new(-1.0, -2.0, 4.0, 5.0));
    let surface = PresentSurface::identity(4, 4, 1.5, 7);

    assert_eq!(
        damage.to_present_damage(surface),
        PresentDamage::Partial(vec![(0, 0, 4, 4)])
    );
}

#[test]
fn logical_damage_maps_rotated_surface_coordinates_before_clipping() {
    let damage = DamageRegion::from_rect(Rect::new(10.0, 20.0, 30.0, 40.0));
    let surface = |transform| PresentSurface::new(200, 100, 1.0, transform, 3);

    assert_eq!(
        damage.to_present_damage(surface(PresentTransform::Rotate90)),
        PresentDamage::Partial(vec![(140, 10, 40, 30)])
    );
    assert_eq!(
        damage.to_present_damage(surface(PresentTransform::Rotate180)),
        PresentDamage::Partial(vec![(160, 40, 30, 40)])
    );
    assert_eq!(
        damage.to_present_damage(surface(PresentTransform::Rotate270)),
        PresentDamage::Partial(vec![(20, 60, 40, 30)])
    );
    assert_eq!(
        damage.to_present_damage(surface(PresentTransform::HorizontalMirrorRotate270)),
        PresentDamage::Partial(vec![(20, 10, 40, 30)])
    );
}

#[test]
fn invalid_damage_or_surface_degrades_to_full() {
    let invalid_damage = DamageRegion::from_rect(Rect::new(f32::INFINITY, 1.0, 2.0, 3.0));
    assert_eq!(
        invalid_damage.to_present_damage(present_surface(1.0)),
        PresentDamage::Full
    );
    assert_eq!(
        single_damage(1.0, 1.0).to_present_damage(PresentSurface::identity(100, 100, 0.0, 0)),
        PresentDamage::Full
    );
}

#[test]
fn retained_buffer_requires_full_after_surface_signature_changes() {
    let mut tracker = PresentDamageTracker::new();
    let damage = single_damage(5.0, 6.0);
    let surface = present_surface(1.0);

    assert_eq!(
        tracker
            .plan(PresentCoherency::RetainedBuffer, surface, None, &damage)
            .present_damage,
        PresentDamage::Full
    );
    tracker.commit(PresentCoherency::RetainedBuffer, surface, None, &damage);
    assert_eq!(
        tracker
            .plan(PresentCoherency::RetainedBuffer, surface, None, &damage)
            .present_damage,
        PresentDamage::Partial(vec![(5, 6, 4, 4)])
    );

    let rebuilt = PresentSurface {
        generation: 2,
        ..surface
    };
    assert_eq!(
        tracker
            .plan(PresentCoherency::RetainedBuffer, rebuilt, None, &damage)
            .present_damage,
        PresentDamage::Full
    );
    let changed_dpr = PresentSurface {
        device_pixel_ratio: 1.25,
        ..surface
    };
    assert_eq!(
        tracker
            .plan(PresentCoherency::RetainedBuffer, changed_dpr, None, &damage)
            .present_damage,
        PresentDamage::Full
    );
}

#[test]
fn tracked_double_buffer_repairs_every_change_since_each_image_was_presented() {
    let mut tracker = PresentDamageTracker::new();
    let surface = present_surface(1.0);
    let image = |index| Some(PresentImage::new(index, 2));
    let a = single_damage(0.0, 0.0);
    let b = single_damage(10.0, 10.0);
    let c = single_damage(20.0, 20.0);
    let d = single_damage(30.0, 30.0);

    assert_eq!(
        tracker
            .plan(PresentCoherency::TrackedSwapchain, surface, image(0), &a)
            .draw_damage,
        PresentDamage::Full
    );
    tracker.commit(PresentCoherency::TrackedSwapchain, surface, image(0), &a);
    assert_eq!(
        tracker
            .plan(PresentCoherency::TrackedSwapchain, surface, image(1), &b)
            .draw_damage,
        PresentDamage::Full
    );
    tracker.commit(PresentCoherency::TrackedSwapchain, surface, image(1), &b);

    let repair_image_0 = tracker.plan(PresentCoherency::TrackedSwapchain, surface, image(0), &c);
    assert_eq!(
        repair_image_0.draw_damage,
        PresentDamage::Partial(vec![(10, 10, 4, 4), (20, 20, 4, 4)])
    );
    assert_eq!(repair_image_0.present_damage, repair_image_0.draw_damage);
    tracker.commit(PresentCoherency::TrackedSwapchain, surface, image(0), &c);

    assert_eq!(
        tracker
            .plan(PresentCoherency::TrackedSwapchain, surface, image(1), &d)
            .draw_damage,
        PresentDamage::Partial(vec![(20, 20, 4, 4), (30, 30, 4, 4)])
    );
}

#[test]
fn tracked_triple_buffer_repairs_continuous_damage_without_gaps() {
    let mut tracker = PresentDamageTracker::new();
    let surface = present_surface(1.0);
    let image = |index| Some(PresentImage::new(index, 3));
    let damages = [
        single_damage(0.0, 0.0),
        single_damage(10.0, 10.0),
        single_damage(20.0, 20.0),
        single_damage(30.0, 30.0),
        single_damage(40.0, 40.0),
    ];

    for (index, damage) in damages.iter().take(3).enumerate() {
        assert_eq!(
            tracker
                .plan(
                    PresentCoherency::TrackedSwapchain,
                    surface,
                    image(index),
                    damage,
                )
                .draw_damage,
            PresentDamage::Full
        );
        tracker.commit(
            PresentCoherency::TrackedSwapchain,
            surface,
            image(index),
            damage,
        );
    }

    assert_eq!(
        tracker
            .plan(
                PresentCoherency::TrackedSwapchain,
                surface,
                image(0),
                &damages[3],
            )
            .draw_damage,
        PresentDamage::Partial(vec![(10, 10, 4, 4), (20, 20, 4, 4), (30, 30, 4, 4),])
    );
    tracker.commit(
        PresentCoherency::TrackedSwapchain,
        surface,
        image(0),
        &damages[3],
    );
    assert_eq!(
        tracker
            .plan(
                PresentCoherency::TrackedSwapchain,
                surface,
                image(1),
                &damages[4],
            )
            .draw_damage,
        PresentDamage::Partial(vec![(20, 20, 4, 4), (30, 30, 4, 4), (40, 40, 4, 4),])
    );
}

#[test]
fn tracked_swapchain_without_valid_image_identity_degrades_to_full() {
    let mut tracker = PresentDamageTracker::new();
    let surface = present_surface(1.0);
    let damage = single_damage(1.0, 1.0);
    tracker.commit(
        PresentCoherency::TrackedSwapchain,
        surface,
        Some(PresentImage::new(0, 2)),
        &damage,
    );

    assert_eq!(
        tracker
            .plan(PresentCoherency::TrackedSwapchain, surface, None, &damage)
            .present_damage,
        PresentDamage::Full
    );
    assert_eq!(
        tracker
            .plan(
                PresentCoherency::TrackedSwapchain,
                surface,
                Some(PresentImage::new(2, 2)),
                &damage,
            )
            .present_damage,
        PresentDamage::Full
    );
    assert_eq!(
        tracker
            .plan(
                PresentCoherency::TrackedSwapchain,
                surface,
                Some(PresentImage::new(0, 9)),
                &damage,
            )
            .present_damage,
        PresentDamage::Full
    );
}
