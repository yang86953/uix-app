# UIX-PERF-030 动画延续帧 cadence 门控（页面切换卡顿修复）

## 结论

主演示快速切换页面、悬停过渡与点击涟漪期间，窗口以不受 vsync 约束的全速构建帧：
帧尾续帧路径把任何残留 paint 失效都武装为「立即帧」，而 `FrameScheduler::request_frame`
对已武装请求只接受「更早 deadline 收紧」。动画 tick 在每帧内产生的新失效因此把本应
按 cadence 到期的动画帧反复收紧为「立即」，形成 `帧构建 ≈ 2ms → 立即续帧 → tick → 失效`
的全速链，直到动画结束才收敛。实测基线在动画期间以平均 1.5～3ms 的帧连续构建
（诊断 `fps` 打印 457～639，`text_layout_calls` 每秒 3400+），桌面负载重叠时单帧
最高拉长到 619.6 ms（layout 211 + render 391 + submit 350），对应的页面切换请求延迟
最高 615 ms——即主人报告的「切换页面卡顿」；带调试 HUD 时同链路表现为常驻 220 fps。

现把帧尾续帧按「是否仍有已注册动画」分流：有动画时走 `request_animation_frame`
（fallback cadence ≈ 显示节奏，且 `request_frame` 语义保证不收紧已武装请求）；空闲
探测路径（`has_frame_work` / `next_deadline`）在已有请求时不再重复武装；只有帧入口
的外部唤醒（输入、定时器、Agent 命令）保留对已武装请求的立即收紧，使输入仍在
同一轮渲染。修复后动画期帧回到 17～33 ms 均值（vsync 节奏）、动画结束约 1 秒内
收敛、空闲安静；页面切换延迟（23.7～50.6 ms）与修复前一致；三页真窗截屏与基线
逐位一致（0.000% 像素差）。

## 场景与复现

主演示（`uix-lang-demo --agent-control`，KWin/Wayland，Vulkan 路径）：

```bash
UIX_SLOW_FRAME_MS=25 RUST_LOG='uix::diagnostics=info,uix=warn' \
  ./demo/target/release/uix-lang-demo --agent-control &
# Agent 通道无间隔连发侧栏切换（pointer_move + invoke 交替），观察动画秒摘要
```

诊断口径注意：`frame_summary` 的 `fps` 字段是 `1000 / 平均帧耗时`，不是每秒真实帧数；
判定吞吐要改看 `text_layout_calls`（每秒 shaping 次数）与摘要出现的秒数。

## 剖析归因

帧生命周期（`window_driver/frame.rs`）中动画帧的续帧链：

1. cadence 帧到期 → 帧入口 `arm_visual_request`（immediate）→ 消费机会、推进动画 tick；
2. tick 经 `WidgetAnimation::update_animation` 返回活跃并标记 `dirty_bounds`
   （如 Button 涟漪的 `ripple_dirty`）→ paint 失效入队；
3. 帧中段 `request_animation_frame` 武装下一 cadence 请求（native 回调 + fallback）；
4. **帧尾 `arm_visual_request` 见 `has_invalidation_work` 为真 → `request_immediate` →
   `request_frame` 按「更早 deadline」把第 3 步的 cadence 请求收紧为「立即」**；
5. 下一帧立即运行、dt≈0、tick 再产生失效 → 回到 4，全速链成立。

同样的收紧也发生在空闲循环探测 `has_frame_work` / `next_deadline` 的
`arm_visual_request` 调用里（循环每轮都会经过），仅改帧尾一处不能断链。

开放 widget 动画（涟漪、loading 旋转器等）在 `ActiveWorkRegistry` 中登记为
`deadline: None`——它们不产生独立 tick deadline，完全依赖帧调度器 pacing；因此
帧尾/探测的收紧链就是它们唯一的时间源，链不断则 pacing 不存在。

## 变更与 SMC 边界

变更限定在 WindowDriver Module 的帧 pacing 策略（`driver.rs` + `frame.rs`），
不改 `FrameScheduler` 契约、动画语义、脏区计算与呈现管线：

- `arm_visual_request` 增加 `animation_continue` 与 `allow_tighten` 两个调用方事实：
  - `animation_continue = true`（帧尾仍有已注册动画）→ `request_animation_frame`，
    天然不收紧已武装请求；
  - 否则仅当 `allow_tighten`（帧入口外部唤醒）或当前无未决请求时才
    `request_immediate`；
- 帧尾调用传 `(有动画, false)`；`has_frame_work` / `next_deadline` 探测传
  `(false, false)`；帧入口传 `(false, true)`，保持输入同轮渲染的既有延迟语义。

失效语义：帧尾残留失效若来自动画 tick，下一 cadence 帧（≤ 一个显示周期）渲染；
「paint/present 期间产生更新状态」的全幅兜底路径（`reset_invalidation` +
`mark_full_frame_dirty`）同样顺延到下一 cadence 帧结算，仍在一个刷新周期内。

## A/B 对照

同一构建协议、同一 Agent 请求序列（绑定桌面环境，KWin/Plasma 6）：

| 指标 | 基线（8cdf8d62） | 修复后 |
|---|---|---|
| 页面切换延迟（agent invoke，冷/温、间隔/连发） | 24.4～50.6 ms | 23.7～47.0 ms（不变） |
| 动画期帧耗时均值（涟漪/过渡秒） | 1.55～5.7 ms（全速链） | 17.19～31.76 ms（cadence） |
| 动画期 shaping 调用（`text_layout_calls`/秒） | 3467～6416 | 85～176 |
| 动画结束收敛 | 持续到动画结束的全速帧 | 约 1 秒内收敛，其后安静 |
| 桌面负载重叠时单帧峰值（run3 记录） | 619.6 ms | 未复现（同协议 23～50 ms） |
| >25 ms 慢帧（48 次无间隔连发） | 20 | 6 |
| 空闲（动画结束后 10～30 s） | 安静 | 安静（摘要数不增） |
| 通用/反馈/覆盖清单三页真窗截屏 | — | 与基线逐位一致（0.000% 差异） |
| 公开 API 测试 | — | 40 个文件 77 用例全绿 |

## 停止边界与已知限制

- 调试模式（`UIX_DEBUG=1` / HUD）下 HUD 状态逐帧变化叠加「调试帧全幅重绘」，
  仍会形成自持渲染；这是调试模式的既有设计，不在本记录范围。
- 永续动画（loading 旋转器等不收敛动画）修复后按显示节奏持续占帧，上限为一个
  刷新周期的一帧预算；动画登记泄漏（若有）属组件生命周期问题，另行处理。
- 桌面高负载时单帧仍可被调度延迟拉长（run3 的 615 ms 尖峰与当时桌面负载重叠，
  同协议受控复测未再复现）；修复把动画期 UI 线程占用降低约一个数量级，此类
  争用窗口随之显著缩小。
