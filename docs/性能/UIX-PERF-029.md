# UIX-PERF-029 布局有效可见性门控评估与测量校准

## 结论

本轮针对 PERF-028 候选侧残留的哈希/身份查询集群（占全套件约 21%）做了第二次尝试：
把布局收敛循环 Phase 1 内 `layout_children_into` 对每个子节点的
`is_effectively_visible` 深度链查询（父链到根 + Provider 上下文包装 + 动态分发），
替换为本轮刚重建的 `effective_visible: HashSet<WidgetId>` 成员查询。等价性论证成立
（父节点已在集合内时其全部直接子节点必在遍历序列中），几何契约与全量测试面均通过。

同目录交替测量后判定**不实施**：27 场景全合计配对几何均值 `+1.16%`，呈双向分化——
宽容器场景小幅受益（Grid `-3.81%`、Responsive Grid `-3.79%`、Spanning Grid
`-2.84%`、Card `-2.57%`），窄子树文本场景小幅受损（Textarea `+5.45%`、Tabs
`+4.65%`、Menu `+4.58%`、Button `+3.61%`）。收益集中在每父多子的形态上，
不足以抵消另一形态的同量级回退与新增门控概念复杂度；源码改动已全部还原。

同样重要的是方法论结论：本机不同目标目录构建的二进制即使源码相同，场景耗时也可能
出现两位数百分比级别的系统性偏移；跨目录 A/B 此前在本轮曾把同一候选误读为
`+12%～+46%` 的全面回归。小于该尺度的性能裁决必须采用「同一 worktree、同一
target 目录内 git 状态交替 + 仅状态切换时重建」的方式，PERF-028 的 `-80%`
级结论远高于此噪声层，不受影响。

## 剖析与候选选择

以 PERF-028 候选符号版二进制重采调用链（1241 样本），把命中哈希/身份原语叶子
的样本按最近业务父帧聚合（265 样本）：`with_widget_context` 谓词闭包 20.4%、
`finish <- hash_one<&WidgetId>` 15.5%、`hash<DefaultHasher>` 链 15.1%、
`is_pending_removal_subtree` 4.5%、`visible_visual_rect_for` 4.2%、
`arrange_positioned_children_into` 3.4%。其中最大簇沿
`layout_children_into -> all(closure) -> is_effectively_visible ->
with_widget_context(fn(&dyn Widget)->bool)` 展开，与
`fill_effective_visibility` 每轮已有的一次性集合形成对照，构成本轮候选。

## 实现与 SMC 边界（已还原）

候选实现保持既有所有权：`LayoutFrameScratch` 仍由 `WidgetTree::layout` 独占，
Phase 1 调用点改走 `layout_children_into_where(frame, children, tree, scratch,
&|child_id| effective_visible.contains(&child_id))` 泛型谓词端口；无集合的其余
调用方继续经原签名委托深度链路径。判定不实施后两处文件均已 `git checkout HEAD`
还原，主分支不含任何本轮源码变更。

## 测量方法校准

三组证据链：

1. 跨目录正式 ABBA（base target vs 主仓 target，各 10 次 × 2 轮）：全部场景
   `+0.8%～+45.7%` 同向回归，幅度随场景绝对耗时变小而放大，呈每轮固定常数加成形态；
2. 把补丁复制进 base worktree 同目录重建后快速复测，Splitter/Menu 即回到基线水平；
3. 新建全新空 target 目录并以干净环境构建，跨目录 ABBA 依旧给出
   `+5%～+35%` 的系统性差异——证实偏差来自构建身份（fat-LTO 代码布局随目录
   历史变化）而非环境噪声或源码问题。

最终协议：单 worktree 内按 `A B B A A B B A A B` 序列做 git 状态切换，仅状态
切换时增量重建并立即测量，6 轮共 60 次运行、27 场景。同二进制 AA 对照确认
运行间噪声地板仍在 ±1.4% 内，上表 18 组配对/场景的差异具备归因效力。
`taskset -c 6` 单核绑定贯穿全程；所有场景稳态分配保持为 0。

## 未通过与未实施候选

- 有效可见性集合门控（本轮主体）：见结论，双向分化、全合计 +1.16%，还原；
- 早期设想的哈希指纹缓存键：无法证明零碰撞，且与本机先例一致被拒。

## 停止边界

可见性查询路径本轮到此为止：剩余 top 自耗（`get_raw` 槽位扫描、provider TLS
进入退出、`set_layout_frame` damage 集合探测）分属不同的组件边界，需要各自的
负载建模与归因，不在本轮范围内顺带处理。

## 验证

- 候选态下恢复的两套契约断言用例 `4 passed + 1 passed`；完整测试面 128 个结果行
  全部 ok、零失败（随后已还原测试文件，不入提交）。
- 方法论证据日志 `/tmp/uix-perf029-same-dir.log`（本次登记后不再保留，统计脚本
  与口径均已收录于本文档表格）。
