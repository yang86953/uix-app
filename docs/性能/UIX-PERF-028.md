# UIX-PERF-028 可搜索 Select 稳态行计数快路

## 结论

本轮以用户态调用链基线确认：布局契约 27 场景剖析测试中，`Select::label_matches_search`
自耗时达 `6.96%`，其父栈为 `set_frame_dirty -> apply_frame_paint ->
visual_subtree_bounds -> is_overlay_node -> overlay_entry -> dropdown_row_count ->
visible_row_count -> for_each_visible_row`。即每帧对视子树的身份类查询都会把
128 个选项按当前查询词重新过滤一遍；打开的可搜索选择器把派生的可见行计数变成了
高频重复工作。

最终候选在 `Select` 内部为 `visible_row_count` 增加一份「精确查询快照」计数缓存：
查询词经字符串相等性比对命中即直接返回；`sync_from` 整体替换 options/optgroups/
loading/search 时显式置空。实测打开的 Select 场景配对几何均值改善 `-80.2%`
（绑核复测两轮分别 `-82.5%`、`-81.3%`），Plain Select 同步受益（中位 `-82.9%`），
关闭状态 Select 也因 damage/hit 路径共享同一计数入口获得 `-20.1%`。四场景资源契约
保持稳态零分配不变。

## 剖析与候选选择

```bash
CARGO_PROFILE_RELEASE_STRIP=none CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
RUSTFLAGS='-C force-frame-pointers=yes' \
rtk cargo build --release -p uix --features test-harness --test layout_frame_allocation_contract
rtk proxy perf record -F 999 -g --call-graph fp \
  -o /tmp/uix-perf028-layout.data ./target/release/deps/layout_frame_allocation_contract-* \
  --ignored --exact --nocapture profile_realistic_layout_scenarios
```

1353 样本（perf 7.2、帧指针调用链）。确认的关键父栈：

```text
set_frame_dirty -> apply_frame_paint
  -> visual_subtree_bounds
    -> is_overlay_node -> node_is_visual_root
      -> overlay_entry        # Select 实现，is_present 早退之后
        -> dropdown_row_count -> visible_row_count
          -> for_each_visible_row -> label_matches_search
```

同一派生计数还被 `hit_test_frame`、`dropdown_damage_rect`、
`layout_select_children_into` 与事件路径消费，单帧内多次进入。查询与选项集合在
稳态帧之间完全不变，重算纯属浪费。候选只处理该计数的读路径；不改动行遍历语义，
也不触及协调、命中排序等既有已优化区域。

## 实现与 SMC 边界

`Select` Component 唯一拥有自己的选项集合与搜索查询，缓存作为其私有字段存在，
不引入跨 Module 状态或新所有者：

1. 新增 `#[snapshot(skip)] visible_row_count_cache: RefCell<Option<VisibleRowCountEntry>>`；
   条目只含精确查询快照与原始可见行数，`dropdown_row_count` 的 `.max(1)` 归一仍发生在读取侧；
2. 读路径先比较 `search_query` 全串再返回缓存；未命中才走既有过滤并回填；
3. 失效契约限定两个运行期写入面：事件路径修改查询由快照比对自然捕获；`sync_from`
   对 options/optgroups/loading/search 做相等性判定后整体置空（沿用同函数
   `intrinsic_width` 缓存的既有失效先例）；构造期链式设置都发生在首次读取之前；
4. 快照字段经 `#[snapshot(skip)]` 排除出协调签名，不改变组件替换语义。

正确性由新增四个内联单元测试锁定：稳定重复读取、查询编辑收窄/放宽、`sync_from`
替换选项、loading 切换强制归零。

## 基线与候选

- A：`2b464b77` 的临时 worktree（base target 内 release 二进制）。
- B：本候选源码的主工作区 release 构建。
- 正式采集用 `taskset -c 6` 单核绑定，逐次完整执行 27 场景剖析测试后解析
  `PROFILE Select:` 行。

会话一（桌面常规负载、未绑核）五对 ABBA（`A,B,B,A,A,B,B,A,A,B`）：

| 对 | A（ns/次） | B（ns/次） | 变化 |
|---|---:|---:|---:|
| 1 | 114,430 | 22,444 | -80.386% |
| 2 | 114,533 | 23,128 | -79.807% |
| 3 | 113,882 | 22,465 | -80.273% |
| 4 | 113,012 | 22,302 | -80.266% |
| 5 | 113,271 | 22,138 | -80.456% |

配对几何均值改善 `-80.239%`，B 胜 `5/5`；A 侧 frame/layout 中位数 `3110/110772 ns`，
B 侧 `304/22140 ns`——`set_frame_dirty` 阶段的开销一并消失。

绑核复测（两组各五对独立进程）：

| 轮次 | 中位数 A → B（ns/次） | 配对几何均值 |
|---|---:|---:|
| 第一轮 | 127,769 → 22,343 | -82.455%（B 胜 5/5） |
| 第二轮 | 118,331 → 22,512 | -81.349%（B 胜 5/5） |

同二进制 AA 对照伪配对的各场景几何均值噪声地板为 `-0.82% ～ +1.42%`，
核心三场景收益高出地板约两个数量级。

## 资源与空间

剖析测试在测量窗口内断言全部场景稳态分配为 0；A/B 两边日志均无任何非零申请行
（缓存命中路径只做借用与字符串相等比较，回填只在内容变化后发生一次）。

等价结构检查通过忽略入口观测：`size_of::<Select>()` 由 `744 B` 增至 `784 B`
（+40 B，为 `RefCell<Option<VisibleRowCountEntry>>` 内联部分；32 B 条目仅在首次
计数读取后进入堆）。相对每实例节省的每帧重复过滤，属于可接受的一次性驻留增量。

## 未通过与未实施候选

- 早期版本考虑过哈希指纹作缓存键，因无法证明零碰撞被放弃；仓库先例也拒绝弱摘要
  （见 PERF-027），改用全串相等性比对。
- 关联场景之外的子表位移（如 Spanning Grid 在不同构建间呈 `-2.6% ～ +9.8%`
  漂移）经验证属 fat-LTO 构建间代码布局方差类：同为无 Select 场景时对几乎相同
  源码的不同重建亦可复现同等幅度漂移，且对 AA 同二进制对照不可见，故不予归因。

## 验证

- 内联单元测试 `select::search::tests::`：`4 passed`（稳定读取、查询编辑、
  sync 替换、loading 切换）加 1 个忽略型尺寸观测入口。
- 完整公开 API 测试面 `cargo test --release`：40 个目标全部 ok，无失败用例。
- 候选侧 perf 复核：`label_matches_search` 从热点榜消失，剩余顶部均为通用
  树/哈希基础设施符号，方向性符合预期；正式裁决以本机交错 A/B 为准。
- 三个修改源文件经 `rustfmt --edition 2024` 定向格式化；`git diff --check` 通过。
