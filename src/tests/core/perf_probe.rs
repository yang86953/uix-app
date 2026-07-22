use crate::core::perf_probe::{
    record_present, set_internal_g5_scenario, take_present, with_internal_g5_scenario,
    PresentProbeSample,
};

#[test]
fn g5_scenario_is_normalized_and_borrowed_without_cloning() {
    set_internal_g5_scenario("modal feedback/暗色");
    with_internal_g5_scenario(|scenario| {
        assert_eq!(scenario, "modal_feedback___");
    });
}

#[test]
fn present_probe_preserves_wgpu_surface_measurement() {
    record_present(PresentProbeSample {
        present_us: 71,
        wgpu_surface_present_cpu_us: 9,
        drawable_pixels: 960_000,
        drawable_width: 1_200,
        drawable_height: 800,
        skipped: 0,
        ..PresentProbeSample::default()
    });
    let sample = take_present();
    assert_eq!(sample.present_us, 71);
    assert_eq!(sample.wgpu_surface_present_cpu_us, 9);
    assert_eq!(sample.drawable_pixels, 960_000);
    assert_eq!(sample.drawable_width, 1_200);
    assert_eq!(sample.drawable_height, 800);
    assert_eq!(sample.skipped, 0);
}
