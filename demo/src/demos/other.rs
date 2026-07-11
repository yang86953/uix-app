//! 组件库页面 — page_other（Transfer / Upload / DesignTokens）。

use uix::prelude::*;

use crate::common::page::PageBuilder;
use crate::common::showcase::{flow_row, sample_block};
use crate::demos::context::DemoCtx;

pub fn page_other(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    PageBuilder::new(tk)
        .gap()
        .block(
            "Transfer",
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
        )
        .block(
            "Upload",
            embed(Upload::new().accept(".png,.jpg").drag(true).multiple(true)),
        )
        .block(
            "QRCode / Watermark",
            embed(
                flow_row(96.0)
                    .child(sample_block(tk, "QRCode", QRCode::new("https://uix.dev")))
                    .child(sample_block(tk, "Watermark", Watermark::new("UIX Demo"))),
            ),
        )
        .block(
            "DesignTokens — 语义色",
            embed(
                flow_row(56.0)
                    .child(sample_block(
                        tk,
                        "Primary",
                        Container::new()
                            .size(72.0, 28.0)
                            .bg(ColorValue::Palette(PaletteColor::PrimaryBg))
                            .rounded(4.0),
                    ))
                    .child(sample_block(
                        tk,
                        "Success",
                        Container::new()
                            .size(72.0, 28.0)
                            .bg(ColorValue::Palette(PaletteColor::SuccessBg))
                            .rounded(4.0),
                    ))
                    .child(sample_block(
                        tk,
                        "Warning",
                        Container::new()
                            .size(72.0, 28.0)
                            .bg(ColorValue::Palette(PaletteColor::WarningBg))
                            .rounded(4.0),
                    ))
                    .child(sample_block(
                        tk,
                        "Error",
                        Container::new()
                            .size(72.0, 28.0)
                            .bg(ColorValue::Palette(PaletteColor::ErrorBg))
                            .rounded(4.0),
                    ))
                    .child(sample_block(
                        tk,
                        "Info",
                        Container::new()
                            .size(72.0, 28.0)
                            .bg(ColorValue::Palette(PaletteColor::InfoBg))
                            .rounded(4.0),
                    )),
            ),
        )
        .block(
            "Fill / Border 层级",
            embed(
                flow_row(48.0)
                    .child(sample_block(
                        tk,
                        "Fill",
                        Container::new()
                            .size(56.0, 20.0)
                            .bg(ColorValue::Neutral(NeutralRole::Fill))
                            .rounded(2.0),
                    ))
                    .child(sample_block(
                        tk,
                        "Fill2",
                        Container::new()
                            .size(56.0, 20.0)
                            .bg(ColorValue::Neutral(NeutralRole::FillSecondary))
                            .rounded(2.0),
                    ))
                    .child(sample_block(
                        tk,
                        "Fill3",
                        Container::new()
                            .size(56.0, 20.0)
                            .bg(ColorValue::Neutral(NeutralRole::FillTertiary))
                            .rounded(2.0),
                    ))
                    .child(sample_block(
                        tk,
                        "Border",
                        Container::new()
                            .size(56.0, 20.0)
                            .bg(ColorValue::Neutral(NeutralRole::Border))
                            .rounded(2.0),
                    )),
            ),
        )
        .build()
}
