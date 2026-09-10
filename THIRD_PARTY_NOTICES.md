# 第三方组件声明 / Third-Party Notices

本文件列出 UIX 分发内容中包含的第三方材料，以及构建期依赖的许可证。
UIX 自身以 MIT 许可证授权，第三方材料仍归各自许可条款约束。

This file identifies third-party materials included in UIX distributions.
It defines the scope of the third-party carve-out referenced by `LICENSE`.

---

## 一、随源码分发的第三方材料 / Bundled third-party materials

### 1. UIX Test Body（测试字体子集 / test-only font subset）

- 文件 / Files：`tests/fixtures/fonts/uix-test-body.ttf`
- 许可证 / License：SIL Open Font License 1.1（OFL-1.1）
- 版权 / Copyright：
  - Copyright 2021-2026 LXGW (<https://github.com/lxgw/LxgwWenKai>)，保留字体名称
    '霞鹜'、'霞鶩'、'落霞孤鹜'、'落霞孤鶩'、'LXGW'
  - Copyright 2020 The Klee Project Authors (<https://github.com/fontworks-fonts/Klee>)
- 说明：该文件是从 LXGW WenKai（其本身衍生自 Klee One）子集化并改名的确定性
  测试 fixture，仅覆盖自动化测试文本所需字形，按 OFL 保留字体名称条款改用
  "UIX Test Body"。它只用于测试，不是产品默认字体；UIX 不随仓库或 crate 分发
  完整正文字体，应用按 `README.md` 的字体说明自行选择系统字体或显式字体包。

### 2. Lucide 图标字体

- 文件 / Files：`assets/fonts/lucide.ttf`
- 许可证 / License：ISC License
- 版权 / Copyright：Copyright (c) 2026 Lucide Icons and Contributors
- 附加说明：该字体中部分图标源自 Feather 项目，依 MIT 许可证授权，
  Copyright (c) 2013-present Cole Bemis。对应的 MIT 条款见下。

### 3. tree-sitter 运行时头文件与生成的解析器

- 文件 / Files：
  - `editors/zed/grammars/uix/src/tree_sitter/alloc.h`
  - `editors/zed/grammars/uix/src/tree_sitter/array.h`
  - `editors/zed/grammars/uix/src/tree_sitter/parser.h`
  - `editors/zed/grammars/uix/src/parser.c`（由 tree-sitter 从本仓库的
    `grammar.js` 生成）
- 许可证 / License：MIT License
- 版权 / Copyright：Copyright (c) 2018 Max Brunsfeld

---

## 二、构建期依赖 / Build-time dependencies

UIX 依赖 130 个 crates.io 包，全部为宽松许可证，不含 GPL/AGPL/MPL 等 copyleft 条款。
这些依赖不随本仓库分发，由 Cargo 在构建时按 `Cargo.lock` 获取。

UIX depends on 130 crates.io packages, all under permissive licenses; none are
copyleft. They are not redistributed in this repository but fetched by Cargo at build time.

### 许可证汇总 / License summary

| 许可证 / License | 包数 / Packages |
| --- | --- |
| `MIT OR Apache-2.0` | 67 |
| `MIT` | 27 |
| `MIT/Apache-2.0` | 7 |
| `Zlib OR Apache-2.0 OR MIT` | 5 |
| `Apache-2.0` | 4 |
| `Unlicense OR MIT` | 3 |
| `Apache-2.0 OR MIT` | 3 |
| `MIT OR Apache-2.0 OR Zlib` | 3 |
| `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | 2 |
| `MIT OR Zlib OR Apache-2.0` | 2 |
| `BSD-3-Clause OR Apache-2.0` | 2 |
| `Zlib` | 2 |
| `0BSD OR MIT OR Apache-2.0` | 1 |
| `ISC` | 1 |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | 1 |

所有依赖均声明了许可证。/ All dependencies declare a license.

### 依赖清单 / Package list

| 包 / Package | 版本 / Version | 许可证 / License |
| --- | --- | --- |
| ab_glyph | 0.2.32 | `Apache-2.0` |
| ab_glyph_rasterizer | 0.1.10 | `Apache-2.0` |
| adler2 | 2.0.1 | `0BSD OR MIT OR Apache-2.0` |
| aho-corasick | 1.1.5 | `Unlicense OR MIT` |
| arrayvec | 0.7.8 | `MIT OR Apache-2.0` |
| ash | 0.38.0+1.3.281 | `MIT OR Apache-2.0` |
| autocfg | 1.5.1 | `Apache-2.0 OR MIT` |
| bitflags | 2.13.2 | `MIT OR Apache-2.0` |
| bumpalo | 3.20.3 | `MIT OR Apache-2.0` |
| bytemuck | 1.25.2 | `Zlib OR Apache-2.0 OR MIT` |
| byteorder-lite | 0.1.0 | `Unlicense OR MIT` |
| cc | 1.4.5 | `MIT OR Apache-2.0` |
| cfg-if | 1.0.4 | `MIT OR Apache-2.0` |
| core_maths | 0.1.1 | `MIT` |
| crc32fast | 1.5.1 | `MIT OR Apache-2.0` |
| dispatch2 | 0.3.1 | `Zlib OR Apache-2.0 OR MIT` |
| dlib | 0.5.3 | `MIT` |
| downcast-rs | 1.2.1 | `MIT/Apache-2.0` |
| earcut | 0.4.11 | `MIT OR Apache-2.0` |
| errno | 0.3.14 | `MIT OR Apache-2.0` |
| fdeflate | 0.3.7 | `MIT OR Apache-2.0` |
| find-msvc-tools | 0.1.12 | `MIT OR Apache-2.0` |
| flate2 | 1.1.10 | `MIT OR Apache-2.0` |
| futures-core | 0.3.34 | `MIT OR Apache-2.0` |
| futures-task | 0.3.34 | `MIT OR Apache-2.0` |
| futures-util | 0.3.34 | `MIT OR Apache-2.0` |
| glow | 0.18.0 | `MIT OR Apache-2.0 OR Zlib` |
| image | 0.25.10 | `MIT OR Apache-2.0` |
| itoa | 1.0.18 | `MIT OR Apache-2.0` |
| js-sys | 0.3.105 | `MIT OR Apache-2.0` |
| khronos-egl | 6.0.0 | `MIT/Apache-2.0` |
| lazy_static | 1.5.0 | `MIT OR Apache-2.0` |
| libc | 0.2.189 | `MIT OR Apache-2.0` |
| libloading | 0.8.9 | `ISC` |
| libm | 0.2.16 | `MIT` |
| linux-raw-sys | 0.12.1 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| log | 0.4.34 | `MIT OR Apache-2.0` |
| matchers | 0.2.0 | `MIT` |
| memchr | 2.8.3 | `Unlicense OR MIT` |
| memmap2 | 0.9.11 | `MIT OR Apache-2.0` |
| miniz_oxide | 0.8.9 | `MIT OR Zlib OR Apache-2.0` |
| miniz_oxide | 0.9.1 | `MIT OR Zlib OR Apache-2.0` |
| moxcms | 0.8.1 | `BSD-3-Clause OR Apache-2.0` |
| nu-ansi-term | 0.50.3 | `MIT` |
| num-bigint | 0.4.8 | `MIT OR Apache-2.0` |
| num-integer | 0.1.47 | `MIT OR Apache-2.0` |
| num-rational | 0.4.2 | `MIT OR Apache-2.0` |
| num-traits | 0.2.19 | `MIT OR Apache-2.0` |
| objc2 | 0.6.4 | `MIT` |
| objc2-core-foundation | 0.3.2 | `Zlib OR Apache-2.0 OR MIT` |
| objc2-encode | 4.1.0 | `MIT` |
| objc2-foundation | 0.3.2 | `MIT` |
| objc2-metal | 0.3.2 | `Zlib OR Apache-2.0 OR MIT` |
| objc2-quartz-core | 0.3.2 | `Zlib OR Apache-2.0 OR MIT` |
| once_cell | 1.21.4 | `MIT OR Apache-2.0` |
| owned_ttf_parser | 0.25.1 | `Apache-2.0` |
| pin-project-lite | 0.2.17 | `Apache-2.0 OR MIT` |
| pkg-config | 0.3.34 | `MIT OR Apache-2.0` |
| png | 0.18.1 | `MIT OR Apache-2.0` |
| proc-macro2 | 1.0.107 | `MIT OR Apache-2.0` |
| pxfm | 0.1.30 | `BSD-3-Clause OR Apache-2.0` |
| qrcode | 0.14.1 | `MIT OR Apache-2.0` |
| quick-xml | 0.41.0 | `MIT` |
| quote | 1.0.47 | `MIT OR Apache-2.0` |
| regex | 1.13.1 | `MIT OR Apache-2.0` |
| regex-automata | 0.4.18 | `MIT OR Apache-2.0` |
| regex-syntax | 0.8.11 | `MIT OR Apache-2.0` |
| rustix | 1.1.4 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` |
| rustversion | 1.0.23 | `MIT OR Apache-2.0` |
| rustybuzz | 0.20.1 | `MIT` |
| scoped-tls | 1.0.1 | `MIT/Apache-2.0` |
| serde | 1.0.229 | `MIT OR Apache-2.0` |
| serde_core | 1.0.229 | `MIT OR Apache-2.0` |
| serde_derive | 1.0.229 | `MIT OR Apache-2.0` |
| serde_json | 1.0.151 | `MIT OR Apache-2.0` |
| sharded-slab | 0.1.7 | `MIT` |
| shlex | 2.0.1 | `MIT OR Apache-2.0` |
| simd-adler32 | 0.3.10 | `MIT` |
| slab | 0.4.12 | `MIT` |
| slotmap | 1.1.1 | `Zlib` |
| smallvec | 1.16.0 | `MIT OR Apache-2.0` |
| syn | 2.0.119 | `MIT OR Apache-2.0` |
| syn | 3.0.5 | `MIT OR Apache-2.0` |
| thread_local | 1.1.10 | `MIT OR Apache-2.0` |
| tracing | 0.1.44 | `MIT` |
| tracing-attributes | 0.1.31 | `MIT` |
| tracing-core | 0.1.36 | `MIT` |
| tracing-log | 0.2.0 | `MIT` |
| tracing-subscriber | 0.3.23 | `MIT` |
| ttf-parser | 0.25.1 | `MIT OR Apache-2.0` |
| unicode-bidi | 0.3.18 | `MIT OR Apache-2.0` |
| unicode-bidi-mirroring | 0.4.0 | `MIT/Apache-2.0` |
| unicode-ccc | 0.4.0 | `MIT/Apache-2.0` |
| unicode-ident | 1.0.24 | `(MIT OR Apache-2.0) AND Unicode-3.0` |
| unicode-linebreak | 0.1.5 | `Apache-2.0` |
| unicode-properties | 0.1.4 | `MIT/Apache-2.0` |
| unicode-script | 0.5.8 | `MIT OR Apache-2.0` |
| unicode-segmentation | 1.13.3 | `MIT OR Apache-2.0` |
| unicode-width | 0.1.14 | `MIT OR Apache-2.0` |
| valuable | 0.1.1 | `MIT` |
| version_check | 0.9.5 | `MIT/Apache-2.0` |
| vte | 0.15.0 | `Apache-2.0 OR MIT` |
| wasm-bindgen | 0.2.128 | `MIT OR Apache-2.0` |
| wasm-bindgen-macro | 0.2.128 | `MIT OR Apache-2.0` |
| wasm-bindgen-macro-support | 0.2.128 | `MIT OR Apache-2.0` |
| wasm-bindgen-shared | 0.2.128 | `MIT OR Apache-2.0` |
| wayland-backend | 0.3.17 | `MIT` |
| wayland-client | 0.31.15 | `MIT` |
| wayland-protocols | 0.32.13 | `MIT` |
| wayland-protocols-plasma | 0.3.12 | `MIT` |
| wayland-protocols-wlr | 0.3.12 | `MIT` |
| wayland-scanner | 0.31.11 | `MIT` |
| wayland-sys | 0.31.11 | `MIT` |
| web-sys | 0.3.105 | `MIT OR Apache-2.0` |
| windows | 0.62.2 | `MIT OR Apache-2.0` |
| windows-collections | 0.3.2 | `MIT OR Apache-2.0` |
| windows-core | 0.62.2 | `MIT OR Apache-2.0` |
| windows-future | 0.3.2 | `MIT OR Apache-2.0` |
| windows-implement | 0.60.2 | `MIT OR Apache-2.0` |
| windows-interface | 0.59.3 | `MIT OR Apache-2.0` |
| windows-link | 0.2.1 | `MIT OR Apache-2.0` |
| windows-numerics | 0.3.1 | `MIT OR Apache-2.0` |
| windows-result | 0.4.1 | `MIT OR Apache-2.0` |
| windows-strings | 0.5.1 | `MIT OR Apache-2.0` |
| windows-sys | 0.61.2 | `MIT OR Apache-2.0` |
| windows-threading | 0.2.1 | `MIT OR Apache-2.0` |
| zlib-rs | 0.6.7 | `Zlib` |
| zmij | 1.0.23 | `MIT` |
| zune-core | 0.5.3 | `MIT OR Apache-2.0 OR Zlib` |
| zune-jpeg | 0.5.15 | `MIT OR Apache-2.0 OR Zlib` |

---

## 三、许可证全文 / Full license texts

### SIL Open Font License 1.1（UIX Test Body，源自 LXGW WenKai / Klee）

```text
Copyright 2021-2026 LXGW (https://github.com/lxgw/LxgwWenKai), with Reserved Font Name '霞鹜', '霞鶩', '落霞孤鹜', '落霞孤鶩' and 'LXGW'. [ADDITIONAL PERMISSION] The Reserved Font Names '霞鹜', '霞鶩', '落霞孤鹜', '落霞孤鶩' and 'LXGW' may continue to be used in Modified Versions recompiled from the Original Version, without modifications to the font source code; or in Modified Versions subsetted or converted to other formats (e.g., WOFF/WOFF2) solely for web font delivery, provided such Modified Versions are not made available as installable desktop fonts (e.g., on mainstream platforms like Google Fonts, or third-party non-commercial platforms recognized by the author @lxgw; other web font platforms please contact the author @lxgw for confirmation).
Copyright 2020 The Klee Project Authors (https://github.com/fontworks-fonts/Klee)

This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
https://openfontlicense.org


-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded, 
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```

### ISC License（Lucide）

```text
ISC License

Copyright (c) 2026 Lucide Icons and Contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

---

The following Lucide icons are derived from the Feather project:

airplay, alert-circle, alert-octagon, alert-triangle, aperture, arrow-down-circle, arrow-down-left, arrow-down-right, arrow-down, arrow-left-circle, arrow-left, arrow-right-circle, arrow-right, arrow-up-circle, arrow-up-left, arrow-up-right, arrow-up, at-sign, calendar, cast, check, chevron-down, chevron-left, chevron-right, chevron-up, chevrons-down, chevrons-left, chevrons-right, chevrons-up, circle, clipboard, clock, code, columns, command, compass, corner-down-left, corner-down-right, corner-left-down, corner-left-up, corner-right-down, corner-right-up, corner-up-left, corner-up-right, crosshair, database, divide-circle, divide-square, dollar-sign, download, external-link, feather, frown, hash, headphones, help-circle, info, italic, key, layout, life-buoy, link-2, link, loader, lock, log-in, log-out, maximize, meh, minimize, minimize-2, minus-circle, minus-square, minus, monitor, moon, more-horizontal, more-vertical, move, music, navigation-2, navigation, octagon, pause-circle, percent, plus-circle, plus-square, plus, power, radio, rss, search, server, share, shopping-bag, sidebar, smartphone, smile, square, table-2, tablet, target, terminal, trash-2, trash, triangle, tv, type, upload, x-circle, x-octagon, x-square, x, zoom-in, zoom-out

The MIT License (MIT) (for the icons listed above)

Copyright (c) 2013-present Cole Bemis

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### MIT License（tree-sitter）

```text
The MIT License (MIT)

Copyright (c) 2018 Max Brunsfeld

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
