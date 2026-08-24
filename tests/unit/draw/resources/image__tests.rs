use super::*;

#[test]
fn cached_path_returns_existing_valid_handle() {
    let service = ImageService::new();
    let path = Path::new("uix-image-cache-hit-does-not-read.png");
    let key = path.to_string_lossy().into_owned();
    let handle = service.insert_slot(ImageSlot::from_decoded(
        1,
        1,
        vec![0xff12_3456],
        Some(key.clone()),
    ));
    service.path_cache.borrow_mut().insert(key, handle);

    assert_eq!(
        service
            .load_from_path(path)
            .expect("valid cached path should not touch the filesystem"),
        handle
    );
    assert_eq!(
        service
            .poll_load_from_path(path)
            .expect("valid cached poll should not start background I/O"),
        Some(handle)
    );
}
