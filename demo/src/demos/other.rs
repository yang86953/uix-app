//! 组件库页面 — page_other（Transfer / Upload / DesignTokens）。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder};
use crate::demos::context::DemoCtx;

pub fn page_other(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    PageBuilder::new(tk)
        .gap()
        .section("Transfer")
        .push(
            column_fit([
                label("Transfer")
                    .color(ColorValue::Neutral(NeutralRole::TextTertiary))
                    .font_size(11.0),
                embed(
                    Transfer::new().source(vec![
                        TransferItem {
                            key: "a".into(),
                            title: "选项 A".into(),
                            selected: false,
                        },
                        TransferItem {
                            key: "b".into(),
                            title: "选项 B".into(),
                            selected: false,
                        },
                        TransferItem {
                            key: "c".into(),
                            title: "选项 C".into(),
                            selected: true,
                        },
                    ]),
                ),
            ])
            .gap(8.0),
        )
        .section("Upload")
        .push(
            column_fit([
                label("Upload")
                    .color(ColorValue::Neutral(NeutralRole::TextTertiary))
                    .font_size(11.0),
                embed(Upload::new().accept(".png,.jpg").drag(true).multiple(true)),
            ])
            .gap(8.0),
        )
        .section("QRCode / Watermark")
        .push(demo_row(80.0).child(QRCode::new("https://uix.dev")))
        .push(demo_row(40.0).child(Watermark::new("UIX Demo")))
        .section("DesignTokens — 语义色")
        .push(
            demo_row(44.0)
                .child(
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(ColorValue::Palette(PaletteColor::PrimaryBg))
                        .rounded(4.0),
                )
                .child(
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(ColorValue::Palette(PaletteColor::SuccessBg))
                        .rounded(4.0),
                )
                .child(
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(ColorValue::Palette(PaletteColor::WarningBg))
                        .rounded(4.0),
                )
                .child(
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(ColorValue::Palette(PaletteColor::ErrorBg))
                        .rounded(4.0),
                )
                .child(
                    Container::new()
                        .size(72.0, 28.0)
                        .bg(ColorValue::Palette(PaletteColor::InfoBg))
                        .rounded(4.0),
                ),
        )
        .section("Fill / Border 层级")
        .push(
            demo_row(24.0)
                .child(
                    Container::new()
                        .size(50.0, 18.0)
                        .bg(ColorValue::Neutral(NeutralRole::Fill))
                        .rounded(2.0),
                )
                .child(
                    Container::new()
                        .size(50.0, 18.0)
                        .bg(ColorValue::Neutral(NeutralRole::FillSecondary))
                        .rounded(2.0),
                )
                .child(
                    Container::new()
                        .size(50.0, 18.0)
                        .bg(ColorValue::Neutral(NeutralRole::FillTertiary))
                        .rounded(2.0),
                )
                .child(
                    Container::new()
                        .size(50.0, 18.0)
                        .bg(ColorValue::Neutral(NeutralRole::Border))
                        .rounded(2.0),
                ),
        )
        .build()
}
