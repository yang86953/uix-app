// 各项自带 cfg(test)，在源模块作用域 include! 展开。

impl<'a> PaintContext<'a> {
pub(crate) fn new_for_test(
        canvas_2d: &'a mut dyn Canvas2D,
        font: FontHandle,
        font_service: &'a FontService,
        image_service: &'a ImageService,
        dpi: f32,
        device_pixel_ratio: f32,
        orientation: Orientation,
        surface_w: i32,
        surface_h: i32,
    ) -> Self {
        Self::new(
            canvas_2d,
            font,
            font_service,
            image_service,
            PaintSurfaceConfig {
                dpi,
                device_pixel_ratio,
                orientation,
                surface_w,
                surface_h,
            },
        )
    }
}
