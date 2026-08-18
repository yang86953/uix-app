# Fonts

- `NotoSansCJKsc-Regular.otf` — the unmodified Simplified Chinese Regular OTF
  from the official [`notofonts/noto-cjk`](https://github.com/notofonts/noto-cjk)
  repository at commit `f8d157532fbfaeda587e826d4cd5b21a49186f7c`.
  SHA-256:
  `2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b`.
  It is installed by `demo/uix-lang-demo` through `App::font_bundle`, so Latin
  and Simplified Chinese layout no longer depend on host font discovery. The
  font is licensed under SIL OFL 1.1; see [`OFL-NotoSansCJK.txt`](OFL-NotoSansCJK.txt).

- `lucide.ttf` — the unmodified `font/Lucide.ttf` asset from the official
  [`lucide-static` 1.17.0](https://www.npmjs.com/package/lucide-static/v/1.17.0)
  npm package. SHA-256:
  `68bde0eb63989cafdfbd64b15020030dfd129ba5b3136b68fe3f62a2bff2d338`.

`App` loads this font through `init_lucide_font` for the `Icon` component. The
font and Lucide icons are ISC-licensed; the upstream license also preserves an
MIT notice for its listed Feather-derived icons. See
[`THIRD_PARTY_NOTICES.md`](../../THIRD_PARTY_NOTICES.md) for the exact notices.
UIX itself remains proprietary and restricted to authorized internal use under
the root [`LICENSE`](../../LICENSE).
