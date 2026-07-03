//! uix-graphics Frame Graph 集成测试。
//! 覆盖资源注册、Pass 声明、编译裁剪、执行编排。

use uix_graphics::frame_graph::FrameGraph;
use uix_graphics::frame_graph::pass::PassNode;
use uix_graphics::frame_graph::resource::{PassId, ResourceId, Version};
use uix_graphics::null_engine::NullEngine;

// ════════════════════════════════════════════════════════════════════════════
// 基础构造
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn frame_graph_new() {
    let fg = FrameGraph::new();
    assert!(fg.passes().is_empty());
}

#[test]
fn frame_graph_default() {
    let fg = FrameGraph::default();
    assert!(fg.passes().is_empty());
}

// ════════════════════════════════════════════════════════════════════════════
// 资源管理
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn register_texture_returns_valid_id() {
    let mut fg = FrameGraph::new();
    let id = fg.register_texture("color", 800, 600);
    assert_eq!(fg.resource_name(id), "color");
    assert_eq!(fg.version_of(id), Version::initial());
}

#[test]
fn register_buffer_returns_valid_id() {
    let mut fg = FrameGraph::new();
    let id = fg.register_buffer("data", 4096);
    assert_eq!(fg.resource_name(id), "data");
    assert_eq!(fg.version_of(id), Version::initial());
}

#[test]
fn register_multiple_resources() {
    let mut fg = FrameGraph::new();
    let r1 = fg.register_texture("a", 100, 100);
    let r2 = fg.register_texture("b", 200, 200);
    let r3 = fg.register_buffer("c", 1024);
    assert_ne!(r1, r2);
    assert_ne!(r2, r3);
    assert_ne!(r1, r3);
}

#[test]
fn mark_resource_dirty_bumps_version() {
    let mut fg = FrameGraph::new();
    let id = fg.register_texture("color", 100, 100);
    assert_eq!(fg.version_of(id), Version::initial());
    fg.mark_resource_dirty(id);
    assert_ne!(fg.version_of(id), Version::initial());
}

#[test]
fn mark_resource_dirty_multiple_times() {
    let mut fg = FrameGraph::new();
    let id = fg.register_texture("color", 100, 100);
    let v0 = fg.version_of(id);
    fg.mark_resource_dirty(id);
    let v1 = fg.version_of(id);
    fg.mark_resource_dirty(id);
    let v2 = fg.version_of(id);
    assert!(v1 > v0);
    assert!(v2 > v1);
}

// ════════════════════════════════════════════════════════════════════════════
// Pass 管理
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn add_pass_returns_valid_id() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 100, 100);
    let _pid = fg.add_pass("Clear", |b| {
        b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(())))
    });
    assert_eq!(fg.passes().len(), 1);
    assert_eq!(fg.passes()[0].name, "Clear");
}

#[test]
fn add_multiple_passes() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 100, 100);
    let _p1 = fg.add_pass("Pass1", |b| b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(()))));
    let _p2 = fg.add_pass("Pass2", |b| b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(()))));
    assert_eq!(fg.passes().len(), 2);
}

#[test]
fn add_pass_node_directly() {
    let mut fg = FrameGraph::new();
    let pid = PassId(100);
    let node = PassNode {
        id: pid,
        name: "DirectPass".into(),
        reads: vec![],
        writes: vec![],
        execute: None,
    };
    fg.add_pass_node(node);
    assert_eq!(fg.passes().len(), 1);
}

// ════════════════════════════════════════════════════════════════════════════
// 编译
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn compile_empty_graph() {
    let mut fg = FrameGraph::new();
    let plan = fg.compile();
    assert!(plan.execution_order.is_empty());
    assert!(plan.barriers.is_empty());
    assert!(plan.culled_passes.is_empty());
    assert!(plan.zero_frame_cost);
}

#[test]
fn compile_single_pass_culled_when_no_consumer() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 100, 100);
    fg.add_pass("Clear", |b| {
        b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(())))
    });
    let plan = fg.compile();
    // 该 pass 只写入 color，无消费者 → 被裁剪
    assert!(plan.execution_order.is_empty());
    assert_eq!(plan.culled_passes.len(), 1);
}

#[test]
fn compile_pass_chain_with_dirty() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 100, 100);
    fg.mark_resource_dirty(color);
    fg.add_pass("Write", |b| b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(()))));
    fg.add_pass("Read", |b| b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(()))));
    let plan = fg.compile();
    // Write 写入 color，Read 读取 color → Write 不被裁剪，Read 也不被裁剪
    // （因为 Write 的输入是 dirty）
    assert_eq!(plan.execution_order.len(), 2);
    assert_ne!(plan.execution_order[0], plan.execution_order[1]);
}

#[test]
fn compile_culls_read_only_pass_when_input_unchanged() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 100, 100);
    fg.add_pass("ReadOnly", |b| {
        b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(())))
    });
    // 第一次编译：无历史记录，pass 执行
    let plan1 = fg.compile();
    assert_eq!(plan1.execution_order.len(), 1);
    // 第二次编译：历史版本匹配当前版本（未变更），pass 被裁剪
    let plan2 = fg.compile();
    assert_eq!(plan2.culled_passes.len(), 1);
    assert!(plan2.execution_order.is_empty());
}

#[test]
fn compile_runs_read_pass_when_input_is_dirty() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 100, 100);
    fg.mark_resource_dirty(color);
    fg.add_pass("ReadAfterDirty", |b| {
        b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(())))
    });
    let plan = fg.compile();
    // 输入 dirty，pass 需要执行
    assert_eq!(plan.execution_order.len(), 1);
    assert!(plan.culled_passes.is_empty());
}

#[test]
fn compile_generates_barrier_for_raw() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 100, 100);
    fg.mark_resource_dirty(color);
    fg.add_pass("Writer", |b| b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(()))));
    fg.add_pass("Reader", |b| b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(()))));
    let plan = fg.compile();
    // two passes alive → execution_order has 2
    assert_eq!(plan.execution_order.len(), 2);
    // Writer writes → Reader reads → RAW barrier
    assert!(!plan.barriers.is_empty(), "应有 RAW 屏障");
}

// ════════════════════════════════════════════════════════════════════════════
// 执行
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn execute_empty_plan() {
    let mut fg = FrameGraph::new();
    let mut engine = NullEngine::new();
    let plan = fg.compile();
    let result = fg.execute(&mut engine, &plan);
    assert!(result.is_ok());
}

#[test]
fn execute_single_pass() {
    let mut fg = FrameGraph::new();
    let mut engine = NullEngine::new();
    let color = fg.register_texture("color", 100, 100);
    fg.mark_resource_dirty(color);
    fg.add_pass("Clear", |b| {
        b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(())))
    });
    let plan = fg.compile();
    let result = fg.execute(&mut engine, &plan);
    assert!(result.is_ok());
}

#[test]
fn execute_pass_chain() {
    let mut fg = FrameGraph::new();
    let mut engine = NullEngine::new();
    let color = fg.register_texture("color", 100, 100);
    fg.mark_resource_dirty(color);

    fg.add_pass("Writer", |b| {
        b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(())))
    });

    let plan = fg.compile();
    let result = fg.execute(&mut engine, &plan);
    assert!(result.is_ok());
}

#[test]
fn execute_with_callback() {
    let mut fg = FrameGraph::new();
    let mut engine = NullEngine::new();
    let color = fg.register_texture("color", 100, 100);
    fg.mark_resource_dirty(color);
    // write pass + 下游 read pass 确保 write 不被裁剪
    fg.add_pass("Write", |b| b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(()))));
    fg.add_pass("Read", |b| b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(()))));
    let plan = fg.compile();

    let mut call_count = 0;
    let result = fg.execute_with(&mut engine, &plan, &mut |_pid, _eng, _res| {
        call_count += 1;
        Ok(())
    });
    assert!(result.is_ok());
    assert_eq!(call_count, 2);
}

#[test]
fn compile_and_execute() {
    let mut fg = FrameGraph::new();
    let mut engine = NullEngine::new();
    let color = fg.register_texture("color", 100, 100);
    fg.mark_resource_dirty(color);
    fg.add_pass("Test", |b| b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(()))));
    let result = fg.compile_and_execute(&mut engine);
    assert!(result.is_ok());
}

// ════════════════════════════════════════════════════════════════════════════
// 状态管理
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn clear_passes_retains_resources() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 100, 100);
    fg.add_pass("Clear", |b| b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(()))));
    fg.clear_passes();
    assert!(fg.passes().is_empty());
    // 资源仍然存在
    assert_eq!(fg.resource_name(color), "color");
}

#[test]
fn reset_clears_everything() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 100, 100);
    fg.add_pass("Clear", |b| b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(()))));
    fg.reset();
    assert!(fg.passes().is_empty());
}

// ════════════════════════════════════════════════════════════════════════════
// 统计
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn culled_and_executed_counts() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 100, 100);
    fg.mark_resource_dirty(color);
    // write + read 链，write 不被裁剪，read 因 input 未变被裁剪
    fg.add_pass("Write", |b| b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(()))));
    fg.add_pass("ReadUnchanged", |b| b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(()))));

    let plan = fg.compile();
    // ReadUnchanged 无法在第一次编译时被裁剪（input_unchanged=false，无历史记录）
    // 所以两个 pass 都执行
    // 第二次编译时 ReadUnchanged 可被裁剪
    assert_eq!(plan.execution_order.len(), 2);
}

// ════════════════════════════════════════════════════════════════════════════
// FrameResources
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn frame_resources_allocate_texture() {
    let mut res = uix_graphics::frame_graph::pass::FrameResources::new();
    let id = ResourceId(0);
    let pixels = res.allocate_texture(id, 10, 10, 0xFF0000FF);
    assert_eq!(pixels.len(), 100);
    assert_eq!(pixels[0], 0xFF0000FF);
    assert_eq!(res.texture_size(id), Some((10, 10)));
    assert!(res.texture_data(id).is_some());
}

#[test]
fn frame_resources_allocate_buffer() {
    let mut res = uix_graphics::frame_graph::pass::FrameResources::new();
    let id = ResourceId(0);
    let buf = res.allocate_buffer(id, 256, 0xAB);
    assert_eq!(buf.len(), 256);
    assert_eq!(buf[0], 0xAB);
    assert!(res.buffer_data(id).is_some());
}

#[test]
fn frame_resources_texture_data_mut() {
    let mut res = uix_graphics::frame_graph::pass::FrameResources::new();
    let id = ResourceId(0);
    res.allocate_texture(id, 5, 5, 0x00000000);
    let data = res.texture_data_mut(id).unwrap();
    data[0] = 0xFFFFFFFF;
    assert_eq!(res.texture_data(id).unwrap()[0], 0xFFFFFFFF);
}
