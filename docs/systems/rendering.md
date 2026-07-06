# 渲染系统

> ScenePaint 契约绘制；局部重绘上屏。不知具体组件类型。

GPU 优先，失败 CPU（#59）。Idle 跳过 present；Paint/Composite 才 present。失效：Layout 否 / Paint·Composite 是；队列可合并矩形。

ScenePaint 提供边界、绘制回调、滚动与 layer 元数据（app 桥接）。合成：LayerTree + DisplayList replay；滚动条带走 Composite。

深度 **≥ 4** 自动 Picture 缓存（#82、#86、#87）。绘制解析 ColorValue + TypographyToken。AnimationRegistry 驱帧（#83）。HiDPI：逻辑 px 布局/绘制，present 前 scale（#70）。
