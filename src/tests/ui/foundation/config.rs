use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::view::{column, embed, label, ViewAdapter};
use crate::ui::{
    ComponentOverrides, ConfigProvider, Input, List, SnapshotFields, Table, TokenPatch,
};

crate::component! {
    struct TokenRenderProbe {
        #[snapshot(skip)]
        observed: Arc<Mutex<(Color, Color)>>,
    }

    render => (&self, _frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        *self
            .observed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            (ctx.tokens().color_primary(), ctx.tokens().color_bg_container());
    }
}

#[test]
fn config_provider_inherits_outer_values_and_overrides_nearest_size() {
    let tree = ViewAdapter::build(
        ConfigProvider::new()
            .component_size(ControlSize::Large)
            .disabled(true)
            .child(|| {
                column((
                    embed(Input::new("outer")),
                    ConfigProvider::new()
                        .component_size(ControlSize::Small)
                        .child(|| embed(Input::new("inner"))),
                ))
            }),
    );

    let inputs = tree.find_all_by_type::<Input>();
    assert_eq!(inputs.len(), 2);
    assert!(matches!(
        inputs[0].1.snapshot_fields(),
        SnapshotFields::Input {
            input_size: ControlSize::Large,
            disabled: true,
            ..
        }
    ));
    assert!(matches!(
        inputs[1].1.snapshot_fields(),
        SnapshotFields::Input {
            input_size: ControlSize::Small,
            disabled: true,
            ..
        }
    ));
}

#[test]
fn config_provider_applies_component_constructor_overrides() {
    let mut overrides = ComponentOverrides::default();
    overrides.input.prefix = Some("¥".to_string());
    overrides.input.suffix = Some("CNY".to_string());

    let tree = ViewAdapter::build(
        ConfigProvider::new()
            .overrides(overrides)
            .child(|| embed(Input::new("amount"))),
    );
    let input = tree
        .find_all_by_type::<Input>()
        .into_iter()
        .next()
        .map(|(_, input)| input)
        .expect("configured input");

    assert!(matches!(
        input.snapshot_fields(),
        SnapshotFields::Input {
            prefix,
            suffix,
            ..
        } if prefix == "¥" && suffix == "CNY"
    ));
}

#[test]
fn component_token_patch_merges_with_subtree_theme_during_render() {
    let observed = Arc::new(Mutex::new((Color::transparent(), Color::transparent())));
    let probe_observed = observed.clone();
    let patched_primary = Color::rgba(12, 34, 56, 255);
    let dark = DesignTokens::antd_dark();
    let tree = ViewAdapter::build(
        ConfigProvider::new()
            .theme(Theme::new(dark.clone()))
            .component_tokens::<TokenRenderProbe>(TokenPatch {
                color_primary: Some(patched_primary),
                ..TokenPatch::default()
            })
            .child(move || {
                embed(TokenRenderProbe {
                    observed: probe_observed,
                })
            }),
    );
    let id = tree
        .find_by_type::<TokenRenderProbe>()
        .expect("token render probe");

    let mut canvas = CpuCanvas2D::new(PixelSurface::new(1, 1));
    let fonts = FontService::new();
    let images = ImageService::new();
    let root_tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fonts,
        &images,
        &root_tokens,
        96.0,
        1.0,
        Orientation::YDown,
        1,
        1,
    );

    tree.get(id)
        .expect("token render node")
        .render(Rect::new(0.0, 0.0, 1.0, 1.0), &mut ctx, &tree);

    assert_eq!(
        *observed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
        (patched_primary, dark.color_bg_container)
    );
    assert_eq!(ctx.tokens().color_primary(), root_tokens.color_primary);
}

#[test]
fn configured_empty_renderer_builds_views_for_list_and_table() {
    let list_tree = ViewAdapter::build(
        ConfigProvider::new()
            .render_empty(|context| label(format!("{} empty", context.component_name())))
            .child(List::new),
    );
    assert!(list_tree.find_by_type::<List>().is_none());
    assert_eq!(
        list_tree
            .find_all_by_type::<crate::ui::Label>()
            .first()
            .map(|(_, label)| label.text()),
        Some("List empty")
    );

    let table_tree = ViewAdapter::build(
        ConfigProvider::new()
            .render_empty(|context| label(format!("{} empty", context.component_name())))
            .child(Table::new),
    );
    assert!(table_tree.find_by_type::<Table>().is_none());
    assert_eq!(
        table_tree
            .find_all_by_type::<crate::ui::Label>()
            .first()
            .map(|(_, label)| label.text()),
        Some("Table empty")
    );
}

#[test]
fn non_empty_list_keeps_its_component_view() {
    let tree = ViewAdapter::build(
        ConfigProvider::new()
            .render_empty(|_| label("unused"))
            .child(|| List::new().items(vec!["one"])),
    );
    assert!(tree.find_by_type::<List>().is_some());
}
