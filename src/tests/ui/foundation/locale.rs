use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::view::{column, label, ViewAdapter};
use crate::ui::{en_us, use_locale, zh_cn, LocaleProvider};

crate::component! {
    struct LocaleRenderProbe {
        #[snapshot(skip)]
        observed: Arc<Mutex<String>>,
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {
        *self
            .observed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = use_locale().ok_text.to_string();
    }
}

#[test]
fn locale_provider_builds_nested_views_with_nearest_locale() {
    let tree = ViewAdapter::build(LocaleProvider::new(en_us()).child(|| {
        column((
            label(use_locale().ok_text),
            LocaleProvider::new(zh_cn()).child(|| label(use_locale().ok_text)),
        ))
    }));

    let labels = tree.find_all_by_type::<crate::ui::Label>();
    assert_eq!(labels.len(), 2);
    assert_eq!(labels[0].1.text(), "OK");
    assert_eq!(labels[1].1.text(), "确定");
    assert_eq!(use_locale().ok_text, "确定");
}

#[test]
fn locale_provider_context_survives_until_render() {
    let observed = Arc::new(Mutex::new(String::new()));
    let probe_observed = observed.clone();
    let mut tree = ViewAdapter::build(LocaleProvider::new(en_us()).child(move || {
        crate::ui::view::embed(LocaleRenderProbe {
            observed: probe_observed,
        })
    }));
    let id = tree
        .find_by_type::<LocaleRenderProbe>()
        .expect("locale render probe");

    let mut canvas = CpuCanvas2D::new(PixelSurface::new(1, 1));
    let fonts = FontService::new();
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        1,
        1,
    );

    tree.get(id).expect("locale render node").render(
        Rect::new(0.0, 0.0, 1.0, 1.0),
        &mut ctx,
        &tree,
    );

    assert_eq!(
        observed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_str(),
        "OK"
    );

    let probe_observed = observed.clone();
    ViewAdapter::reconcile(
        &mut tree,
        LocaleProvider::new(zh_cn()).child(move || {
            crate::ui::view::embed(LocaleRenderProbe {
                observed: probe_observed,
            })
        }),
    );
    let id = tree
        .find_by_type::<LocaleRenderProbe>()
        .expect("reconciled locale render probe");
    tree.get(id).expect("reconciled locale render node").render(
        Rect::new(0.0, 0.0, 1.0, 1.0),
        &mut ctx,
        &tree,
    );
    assert_eq!(
        observed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_str(),
        "确定"
    );
}
