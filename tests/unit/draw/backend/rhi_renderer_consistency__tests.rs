//! API 无关全图元规范场景的闭集与 ABI 门禁。

use super::*;
use crate::platform::presentation::rhi::PipelineVertexLayout;
use std::collections::HashSet;

#[test]
fn every_pipeline_has_one_valid_canonical_scene() {
    let scenes = canonical_scenes();
    assert_eq!(scenes.len(), CONSISTENCY_PIPELINES.len());
    let mut covered = HashSet::new();
    for scene in &scenes {
        assert!(covered.insert(scene.kind), "pipeline 场景不得重复");
        let contract = scene.kind.contract();
        assert_eq!(scene.vertex.layout(), contract.vertex);
        assert_eq!(scene.uniform.layout(), contract.uniform);
        assert!(scene.vertex.is_valid());
        assert!(scene.uniform.is_valid());
        assert!(!scene.samples.is_empty());
        for sample in &scene.samples {
            assert!(sample.x < CONSISTENCY_EXTENT.width);
            assert!(sample.y < CONSISTENCY_EXTENT.height);
            assert!(
                sample
                    .minimum
                    .into_iter()
                    .zip(sample.maximum)
                    .all(|(a, b)| a <= b)
            );
            assert!(
                sample.tolerance.amount() <= ConsistencyTolerance::Analytic.amount(),
                "每类容差必须保持显式且有界"
            );
        }
        match (&scene.texture, contract.sampling) {
            (None, crate::platform::presentation::rhi::PipelineSampling::None) => {}
            (Some(texture), sampling) => {
                assert!(sampling.accepts(texture.format, texture.sampler));
                assert_eq!(
                    texture.bytes.len(),
                    texture.extent.width as usize
                        * texture.extent.height as usize
                        * texture.format.bytes_per_pixel()
                );
            }
            _ => panic!("场景采样资源必须匹配 PipelineContract"),
        }
    }
    assert_eq!(covered.len(), 11);
    assert_eq!(
        scenes
            .iter()
            .filter(|scene| scene.source == ConsistencySource::DrawingBlur)
            .map(|scene| scene.kind)
            .collect::<Vec<_>>(),
        vec![PipelineKind::BlurPass]
    );
}

// Blur 规范场景必须覆盖非零原点、水平/垂直两 pass 及内外边界。
#[test]
fn blur_scenario_owns_nonzero_subregion_and_two_pass_invariants() {
    let scenario = blur_subregion_scenario();
    assert_eq!(
        scenario.vertex.layout(),
        PipelineVertexLayout::PositionUvF32
    );
    assert!(scenario.vertex.is_valid());
    assert!(scenario.horizontal.is_valid());
    assert!(scenario.vertical.is_valid());
    assert_eq!(
        scenario.scissor,
        RhiScissor {
            x: 2,
            y: 34,
            width: 10,
            height: 8,
        }
    );
    assert_eq!(scenario.horizontal_samples.len(), 4);
    assert_eq!(scenario.final_samples.len(), 4);
    assert!(
        scenario
            .horizontal_samples
            .iter()
            .chain(&scenario.final_samples)
            .any(|sample| sample.semantic.contains("outside destination"))
    );
    assert!(
        scenario
            .horizontal_samples
            .iter()
            .chain(&scenario.final_samples)
            .any(|sample| sample.semantic.contains("source boundary"))
    );
}
