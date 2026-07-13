use super::*;

fn snapshot_hash(pixels: &[u32]) -> u64 {
    pixels.iter().fold(0xcbf29ce484222325, |hash, pixel| {
        (hash ^ u64::from(*pixel)).wrapping_mul(0x100000001b3)
    })
}

#[test]
fn fake_presenter_records_framebuffer_and_damage_snapshot() {
    let mut presenter = FakePresenter::new();
    let pixels = [0xff000000, 0xffff0000, 0xff00ff00, 0xff0000ff];

    presenter
        .present(&pixels, 2, 2, PresentDamage::single(1, 0, 1, 2))
        .expect("fake present should succeed");

    assert_eq!(presenter.present_count(), 1);
    assert_eq!(presenter.state.last_pixels, pixels);
    assert_eq!(
        snapshot_hash(&presenter.state.last_pixels),
        0x03e6c7ef4e14a058
    );
    assert_eq!(presenter.state.present_calls[0].width, 2);
    assert_eq!(presenter.state.present_calls[0].height, 2);
    assert_eq!(presenter.state.present_calls[0].pixels_len, 4);
    assert_eq!(
        presenter.state.present_calls[0].damage,
        PresentDamage::Partial(vec![(1, 0, 1, 2)])
    );
}
