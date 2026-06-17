//! Frame Graph —— 统一测试集。
//!
//! 从 mod.rs、compile.rs、resource.rs 集中移入，控制各文件行数 ≤ 400。

#![cfg(test)]

use super::compile::BarrierKind;
use super::*;
use crate::graphics::engine::GraphicsEngine;
use crate::graphics::NullEngine;
use std::collections::HashMap;

// ════════════════════════════════════════════════════════════════════════════
// 辅助函数
// ════════════════════════════════════════════════════════════════════════════

fn dummy_pass(id: PassId, name: &str, reads: Vec<ResourceId>, writes: Vec<ResourceId>) -> PassNode {
    let pass_name = String::from(name);
    PassNode::new(
        id,
        pass_name,
        reads,
        writes,
        Box::new(move |_ctx: &mut PassContext| Ok(())),
    )
}

fn make_versions(pairs: &[(ResourceId, Version)]) -> HashMap<ResourceId, Version> {
    pairs.iter().copied().collect()
}

// ════════════════════════════════════════════════════════════════════════════
// FrameGraph 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn frame_graph_empty_compile() {
    let mut fg = FrameGraph::new();
    let plan = fg.compile();
    assert!(plan.zero_frame_cost);
    assert!(plan.execution_order.is_empty());
}

#[test]
fn frame_graph_single_pass_with_consumer() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 100, 100);

    // Producer + consumer 构成有意义管线
    fg.add_pass("Clear", |b| {
        b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(())))
    });
    fg.add_pass("Present", |b| {
        b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(())))
    });

    let plan = fg.compile();
    assert!(!plan.zero_frame_cost);
    assert_eq!(plan.execution_order.len(), 2);

    // 第二次编译：Clear 仍被消费，Present 输入未变 → 裁 Present
    let plan2 = fg.compile();
    assert!(!plan2.zero_frame_cost);
    assert_eq!(plan2.execution_order.len(), 1);
    // 稳定态：Clear 每次跑（output 被 Present 消费），Present 无变化时被裁
    let plan3 = fg.compile();
    assert!(!plan3.zero_frame_cost);
    assert_eq!(plan3.execution_order.len(), 1);
}

#[test]
fn frame_graph_execute() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 100, 100);
    let mut engine = NullEngine::new();
    let _ = <NullEngine as GraphicsEngine>::initialize(&mut engine, 100, 100);

    fg.add_pass("Clear", |b| {
        b.writes(&[color]).execute_with(Box::new(|mut ctx| {
            let _ = ctx.engine;
            Ok(())
        }))
    });
    fg.add_pass("Notify", |b| {
        b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(())))
    });

    let plan = fg.compile();
    assert!(!plan.zero_frame_cost);
    assert!(fg.execute(&mut engine, &plan).is_ok());
    // 执行后 color 版本应递增
    assert!(
        fg.version_of(color) > Version::initial(),
        "version should be bumped after execute: got {:?}",
        fg.version_of(color)
    );
}

#[test]
fn frame_graph_two_passes_chain() {
    let mut fg = FrameGraph::new();
    let a = fg.register_texture("a", 10, 10);
    let b = fg.register_texture("b", 10, 10);

    fg.add_pass("PassA", |pb| {
        pb.writes(&[a]).execute_with(Box::new(|_ctx| Ok(())))
    });
    fg.add_pass("PassB", |pb| {
        pb.reads(&[a])
            .writes(&[b])
            .execute_with(Box::new(|_ctx| Ok(())))
    });

    let plan = fg.compile();
    assert!(!plan.execution_order.is_empty());
    assert!(plan.execution_order.contains(&PassId(0)));
}

#[test]
fn frame_graph_zero_cost_when_idle() {
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 200, 200);

    fg.add_pass("Clear", |b| {
        b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(())))
    });
    fg.add_pass("Present", |b| {
        b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(())))
    });

    let plan1 = fg.compile();
    assert!(!plan1.zero_frame_cost);

    let plan2 = fg.compile();
    assert!(!plan2.zero_frame_cost);
    assert_eq!(plan2.execution_order.len(), 1);

    // 模拟外部变更 → Clear 触发 Present
    fg.mark_resource_dirty(color);
    let plan3 = fg.compile();
    assert!(!plan3.zero_frame_cost);
    assert_eq!(plan3.execution_order.len(), 2);
}

#[test]
fn frame_graph_execute_with_callback() {
    // 测试 execute_with：使用外部回调替代 Pass 内建 execute
    let mut fg = FrameGraph::new();
    let color = fg.register_texture("color", 50, 50);
    let dirty = fg.register_buffer("dirty", 1);

    // 仅结构化的 Pass（无 execute 函数）
    fg.add_pass_node(PassNode::new_structural(
        PassId(0),
        "Render",
        vec![dirty],
        vec![color],
    ));

    let mut engine = NullEngine::new();
    let _ = <NullEngine as GraphicsEngine>::initialize(&mut engine, 50, 50);
    let mut render_count = 0u32;

    // 第一次编译：无历史 → 执行
    let plan = fg.compile();
    assert!(!plan.zero_frame_cost);
    fg.execute_with(&mut engine, &plan, &mut |_pid, _eng, _res| {
        render_count += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(render_count, 1);

    // 第二次编译：dirty 未标记 → 输入未变 → 裁剪
    let plan2 = fg.compile();
    assert!(plan2.zero_frame_cost);
    assert!(plan2.execution_order.is_empty());

    // 标记 dirty → 再次执行
    fg.mark_resource_dirty(dirty);
    let plan3 = fg.compile();
    assert!(!plan3.zero_frame_cost);
    fg.execute_with(&mut engine, &plan3, &mut |_pid, _eng, _res| {
        render_count += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(render_count, 2);
}

// ════════════════════════════════════════════════════════════════════════════
// Compiler 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn no_history_forces_execution() {
    let mut compiler = Compiler::new();
    let res = ResourceId(0);

    let passes = vec![dummy_pass(PassId(0), "first_run", vec![res], vec![res])];
    let versions = make_versions(&[(res, Version::initial())]);

    let result = compiler.compile(&passes, &versions);
    assert!(!result.zero_frame_cost);
    assert_eq!(result.execution_order.len(), 1);

    // 第二次编译：历史版本匹配当前 → 输入未变；输出无外部消费者 → 裁剪
    let result2 = compiler.compile(&passes, &versions);
    assert!(result2.zero_frame_cost);
    assert!(result2.execution_order.is_empty());
}

#[test]
fn chain_passes() {
    let mut compiler = Compiler::new();
    let a = ResourceId(0);
    let b = ResourceId(1);

    let passes = vec![
        dummy_pass(PassId(0), "producer", vec![], vec![a]),
        dummy_pass(PassId(1), "consumer", vec![a], vec![b]),
    ];
    let versions = make_versions(&[(a, Version::initial()), (b, Version::initial())]);

    let result = compiler.compile(&passes, &versions);
    assert!(!result.zero_frame_cost);
    assert_eq!(result.execution_order.len(), 2);
    assert_eq!(result.execution_order[0], PassId(0));
    assert_eq!(result.execution_order[1], PassId(1));

    // 应有 RAW 屏障
    let has_raw = result.barriers.iter().any(|b| {
        b.from_pass == PassId(0) && b.to_pass == PassId(1) && b.kind == BarrierKind::ReadAfterWrite
    });
    assert!(has_raw);

    // 第二次编译：版本不变 → Pass0 输出被消费不裁，Pass1 输入未变 + 输出无消费 → 裁
    let result2 = compiler.compile(&passes, &versions);
    assert_eq!(result2.execution_order.len(), 1);
    assert_eq!(result2.execution_order[0], PassId(0));
    assert!(result2.culled_passes.contains(&PassId(1)));
}

#[test]
fn changed_input_triggers_execution() {
    let mut compiler = Compiler::new();
    let a = ResourceId(0);

    let passes = vec![dummy_pass(PassId(0), "dep", vec![a], vec![])];
    let versions = make_versions(&[(a, Version::initial())]);

    // 第一次编译：无历史 → 输入视为已变化 → 执行
    let r1 = compiler.compile(&passes, &versions);
    assert!(!r1.zero_frame_cost);
    assert_eq!(r1.execution_order, vec![PassId(0)]);

    // 第二次编译：历史版本 = current → 未变，输出无消费 → 裁剪
    let r2 = compiler.compile(&passes, &versions);
    assert!(r2.zero_frame_cost);
    assert!(r2.execution_order.is_empty());

    // 模拟外部变更
    let mut bumped = versions.clone();
    bumped.insert(a, Version::initial().advance());
    let r3 = compiler.compile(&passes, &bumped);
    assert!(!r3.zero_frame_cost);
    assert_eq!(r3.execution_order, vec![PassId(0)]);
}

#[test]
fn topological_order_complex() {
    let mut compiler = Compiler::new();
    let a = ResourceId(0);
    let b = ResourceId(1);
    let c = ResourceId(2);

    let passes = vec![
        dummy_pass(PassId(1), "pass1", vec![], vec![a]),
        dummy_pass(PassId(2), "pass2", vec![], vec![b]),
        dummy_pass(PassId(3), "pass3", vec![a, b], vec![c]),
    ];
    let mut versions = HashMap::new();
    versions.insert(a, Version::initial());
    versions.insert(b, Version::initial());
    versions.insert(c, Version::initial());

    let result = compiler.compile(&passes, &versions);

    // Pass1, Pass2 可并行（无依赖），Pass3 依赖两者 → Pass3 必须在最后
    assert!(!result.execution_order.is_empty());
    if let Some(pos3) = result.execution_order.iter().position(|&p| p == PassId(3)) {
        assert!(
            result.execution_order[..pos3].contains(&PassId(1)),
            "Pass3 should be after Pass1"
        );
        assert!(
            result.execution_order[..pos3].contains(&PassId(2)),
            "Pass3 should be after Pass2"
        );
    } else {
        panic!("Pass3 should be in execution order");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ResourceRegistry 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn resource_register_and_version() {
    let mut reg = ResourceRegistry::new();
    let tid = reg.register("color", ResourceKind::texture(800, 600));
    assert_eq!(reg.version_of(tid), Version::initial());

    let pid = reg.allocate_pass_id();
    let v1 = reg.bump_version(tid, pid);
    assert!(v1 > Version::initial());
    assert_eq!(reg.last_writer_of(tid), Some(pid));
    assert_eq!(reg.version_of(tid), v1);
}

#[test]
fn version_monotonic() {
    let mut reg = ResourceRegistry::new();
    let id = reg.register("buf", ResourceKind::buffer(256));
    let pid = reg.allocate_pass_id();
    let v0 = reg.version_of(id);
    let v1 = reg.bump_version(id, pid);
    let v2 = reg.bump_version(id, pid);
    assert!(v1 > v0);
    assert!(v2 > v1);
}

#[test]
fn unknown_resource_returns_initial_version() {
    let reg = ResourceRegistry::new();
    let unknown = ResourceId(999);
    assert_eq!(reg.version_of(unknown), Version::initial());
    assert_eq!(reg.last_writer_of(unknown), None);
}

#[test]
fn resource_name() {
    let mut reg = ResourceRegistry::new();
    let id = reg.register("shadow_map", ResourceKind::texture(256, 256));
    assert_eq!(reg.resource_name(id), "shadow_map");
}
