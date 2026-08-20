    // 引入统一错误分类。
    use crate::core::Errc;
    // 引入被测共享状态机。
    use super::RhiPassState;
    // 引入构造 pass 输入与资源身份所需的共享值。
    use crate::native::present::rhi::{
        DrawRasterState, LoadAction, RenderTargetHandle, RhiColor, RhiExtent, RhiScissor,
        RhiViewport, TextureHandle,
    };

    // 创建稳定的测试目标身份。
    const TARGET: RenderTargetHandle = RenderTargetHandle::for_test(TextureHandle::from_raw(7));
    // 创建与目标不同的采样纹理身份。
    const TEXTURE: TextureHandle = TextureHandle::from_raw(8);

    // 验证非法 begin 不会留下半打开状态。
    #[test]
    fn begin_rejects_invalid_target_facts_without_mutation() {
        // 创建空状态机。
        let mut state = RhiPassState::new();
        // 零宽目标必须被共享层拒绝。
        assert!(
            state
                // 尝试使用无效物理范围开始 pass。
                .begin(TARGET, RhiExtent::new(0, 20), LoadAction::Load)
                // 要求返回失败。
                .is_err()
        );
        // 失败后状态必须仍然关闭。
        assert!(!state.is_open());
        // 非预乘清屏颜色也必须被共享层拒绝。
        assert!(
            state
                // 红通道大于 alpha，违反预乘不变量。
                .begin(
                    TARGET,
                    RhiExtent::new(20, 20),
                    LoadAction::Clear(RhiColor::from_premultiplied_rgba([1.0, 0.0, 0.0, 0.5])),
                )
                // 要求返回失败。
                .is_err()
        );
        // 第二次失败后也不能留下活动目标。
        assert!(!state.is_open());
    }

    // 验证目标范围统一约束 viewport、scissor 和局部清理。
    #[test]
    fn active_target_owns_all_physical_geometry_validation() {
        // 创建空状态机。
        let mut state = RhiPassState::new();
        // 建立二十乘十的活动目标。
        state
            // 使用保留载荷开始 pass。
            .begin(TARGET, RhiExtent::new(20, 10), LoadAction::Load)
            // 测试设置必须成功。
            .expect("pass should begin");
        // 完整 viewport 和裁剪必须组成当前 Draw 的单一栅格状态。
        let raster = DrawRasterState::new(
            // 使用与目标完全一致的物理范围。
            RhiViewport {
                // 使用完整目标宽度。
                width: 20.0,
                // 使用完整目标高度。
                height: 10.0,
            },
            // 当前 Draw 使用完整目标，不额外裁剪。
            None,
        );
        // 完整栅格状态必须合法。
        state
            // 验证当前 Draw 自带的完整物理事实。
            .validate_draw_raster(raster)
            // 共享几何门禁必须接收。
            .expect("raster should fit");
        // 越过目标的 viewport 必须失败。
        assert!(
            state
                // 验证 viewport 超出一个像素的 packet 状态。
                .validate_draw_raster(DrawRasterState::new(
                    // 创建越过目标的 viewport。
                    RhiViewport {
                        // 宽度越过目标。
                        width: 21.0,
                        // 高度保持合法。
                        height: 10.0,
                    },
                    // 不追加 scissor 干扰本次边界验证。
                    None,
                ))
                // 要求返回失败。
                .is_err()
        );
        // 创建一个合法的显式 scissor。
        let scissor = RhiScissor {
            // 从第二个像素开始。
            x: 1,
            // 从第三个像素行开始。
            y: 2,
            // 覆盖四个像素宽度。
            width: 4,
            // 覆盖五个像素高度。
            height: 5,
        };
        // 带 scissor 的完整 packet 栅格状态必须通过。
        state
            // 把 viewport 与 scissor 作为同一不可拆事实验证。
            .validate_draw_raster(DrawRasterState::new(raster.viewport(), Some(scissor)))
            // 合法区域必须被接受。
            .expect("scissor should fit");
        // 同一范围也必须通过局部清理门禁。
        state
            // 使用规范透明色验证区域。
            .validate_clear(RhiColor::transparent(), scissor)
            // 合法清理必须成功。
            .expect("clear should fit");
    }

    // 验证 pass 只核对当前 Draw 的采样输入与活动输出关系。
    #[test]
    fn sampled_texture_rejects_feedback_without_storing_binding() {
        // 创建并开始一个测试 pass。
        let mut state = RhiPassState::new();
        // 建立稳定目标范围。
        state
            // 使用目标七开始 pass。
            .begin(TARGET, RhiExtent::new(20, 10), LoadAction::Load)
            // 测试设置必须成功。
            .expect("pass should begin");
        // 从封闭目标中取得同一纹理身份以模拟反馈环。
        let target_texture = TARGET.texture().expect("target should be a texture");
        // 当前 render target 不能同时作为 sampled source。
        assert!(state.validate_sampled_texture(target_texture).is_err());
        // 与输出不同的 packet 采样纹理必须通过共享关系门禁。
        state
            // 只交付当前 Draw 自身携带的纹理身份。
            .validate_sampled_texture(TEXTURE)
            // 不同资源不得被 pass-local 历史状态影响。
            .expect("distinct sampled texture should pass");
    }

    // 验证活动目标销毁在两个 Adapter 之前共享同一生命周期错误。
    #[test]
    fn texture_destroy_rejects_only_the_active_target() {
        // 创建并开始一个离屏 texture pass。
        let mut state = RhiPassState::new();
        // 使用稳定物理范围建立活动目标。
        state
            // 使用共享封闭 texture target。
            .begin(TARGET, RhiExtent::new(20, 10), LoadAction::Load)
            // 测试设置必须成功。
            .expect("pass should begin");
        // 取得当前 target 携带的同一 texture 身份。
        let target_texture = TARGET.texture().expect("target should be a texture");
        // 在 pass 仍打开时销毁输出目标必须失败。
        let error = state
            // 调用两个 Adapter 共用的销毁前门禁。
            .validate_texture_destroy(target_texture)
            // 活动目标不得通过。
            .expect_err("active render target destroy must fail");
        // 该违例属于调用顺序错误，不是资源参数错误。
        assert_eq!(error.code(), Errc::InvalidState);
        // 同一 pass 中未作为输出的 texture 允许进入资源表销毁。
        state
            // 使用与目标不同的类型化 texture 身份。
            .validate_texture_destroy(TEXTURE)
            // 共享门禁必须放行。
            .expect("non-target texture destroy should pass");
        // 结束 pass 后旧目标不再被活动状态引用。
        state.end().expect("pass should end");
        // 已结束目标应允许正常销毁。
        state
            // 重新验证原 target texture。
            .validate_texture_destroy(target_texture)
            // 关闭状态不得拒绝。
            .expect("closed-pass target destroy should pass");
    }

    // 验证 end 原子清除所有 pass 级目标与几何事实。
    #[test]
    fn end_clears_target_and_geometry() {
        // 创建并开始一个测试 pass。
        let mut state = RhiPassState::new();
        // 建立稳定目标范围。
        state
            // 使用保留载荷开始 pass。
            .begin(TARGET, RhiExtent::new(20, 10), LoadAction::Load)
            // 测试设置必须成功。
            .expect("pass should begin");
        // 结束活动 pass。
        state.end().expect("pass should end");
        // end 后必须允许 pass 外命令。
        state
            // 检查共享关闭状态。
            .require_closed()
            // 状态机必须报告成功。
            .expect("state should be closed");
        // end 后不能再读取陈旧目标。
        assert!(state.target().is_err());
        // 重复 end 必须作为顺序错误被拒绝。
        assert!(state.end().is_err());
    }
