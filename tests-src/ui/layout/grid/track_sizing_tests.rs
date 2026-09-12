//! `ui/layout/grid/track_sizing.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

fn resolve(tracks: &[GridTrack], available: f32) -> Vec<f32> {
    let mut sizes = Vec::new();
    resolve_tracks_into(tracks, available, 0.0, &[], &mut sizes);
    sizes
}

fn minmax(min: GridTrackMin, max: GridTrackMax) -> GridTrack {
    GridTrack::MinMax(min, max)
}

// 0fr 上界合法：保留下界、不参与分配，且下界不得重复送入分配池。
#[test]
fn zero_fr_minmax_keeps_floor_without_feeding_the_pool() {
    assert_eq!(
        resolve(
            &[
                minmax(GridTrackMin::Px(100.0), GridTrackMax::Fr(0.0)),
                GridTrack::Fr(1.0),
            ],
            1000.0,
        ),
        vec![100.0, 900.0]
    );
    assert_eq!(
        resolve(
            &[
                minmax(GridTrackMin::Px(50.0), GridTrackMax::Fr(0.0)),
                minmax(GridTrackMin::Px(50.0), GridTrackMax::Fr(0.0)),
                GridTrack::Fr(1.0),
            ],
            1000.0,
        ),
        vec![50.0, 50.0, 900.0]
    );
    // 零权重、固定上界与正权重混合：固定上界先增长，剩余全部归正权重轨道。
    assert_eq!(
        resolve(
            &[
                minmax(GridTrackMin::Px(100.0), GridTrackMax::Fr(0.0)),
                minmax(GridTrackMin::Px(100.0), GridTrackMax::Px(300.0)),
                GridTrack::Fr(1.0),
            ],
            1000.0,
        ),
        vec![100.0, 300.0, 600.0]
    );
    // 正权重 minmax 的下界仍按原规则放回分配池并在份额不足时冻结。
    assert_eq!(
        resolve(
            &[
                minmax(GridTrackMin::Px(600.0), GridTrackMax::Fr(1.0)),
                minmax(GridTrackMin::Px(100.0), GridTrackMax::Fr(0.0)),
                GridTrack::Fr(1.0),
            ],
            1000.0,
        ),
        vec![600.0, 100.0, 300.0]
    );
}
