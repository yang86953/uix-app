# 布局系统

> measure 定尺寸；Flex + Grid 排布；Scroll 消化 Wheel。

**measure**（#29）在 **Constraints** `{ min, max, definite }`（#38）下返回 Size。盒模型：margin / padding / 四边 border / content。

v1 **Flex + Grid**（#53）。flex 对齐 gap 与 grid 轨道在 **Style**（#81、#84）；grid 容器用 View `grid`（#67）。ScrollContainer 消化 Wheel（#45）；View `scroll` 纵/横（#73）。

Layout 失效不 present；Paint 才 present。Active = 与祖先 clip/scroll 求交有可见像素（#19）。
