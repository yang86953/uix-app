    // 引入被测裁剪、写区分析和命令类型。
    use super::*;
    // 引入测试使用的源颜色。
    use crate::draw::Color;

    // Additive 写区分析必须只使用 rect 与 clip 的真实可见交集。
    #[test]
    fn additive_write_grouping_uses_clipped_bounds() {
        // 构造几何包围盒重叠、但 Additive 可见裁剪互不相交的命令流。
        let clipped_commands = [
            // 第一条命令的几何延伸到右侧，但只允许写入左侧三列。
            FrameCommand::Native {
                // 保存带局部裁剪的硬 Additive 矩形。
                operation: FrameRasterOp::FillRectAdditive {
                    // 几何包围盒覆盖 x=1..9。
                    rect: FrameRect::new(1, 1, 8, 4),
                    // 使用不透明绿色作为 Additive 源。
                    color: Color::green(),
                    // 实际写区只覆盖 x=1..4。
                    clip: FrameRect::new(1, 1, 3, 4),
                },
            },
            // 第二条半透明命令从 x=6 开始，因此与真实 Additive 写区无交集。
            FrameCommand::Native {
                // 保存一个不能充当不透明 cover 的普通矩形。
                operation: FrameRasterOp::FillRect {
                    // 该矩形只与未裁剪的 Additive 几何包围盒重叠。
                    rect: FrameRect::new(6, 1, 2, 4),
                    // 半透明颜色确保重叠时不能借助 opaque cover 放行。
                    color: Color::from_rgba(255, 0, 0, 128),
                },
            },
        ];
        // 真实写区互不覆盖时 Picture splice 分组应安全。
        assert!(source_over_commands_have_safe_grouping(
            &clipped_commands,
            12,
            8
        ));
        // 构造相同几何，但将 Additive clip 扩展到第二条命令上的命令流。
        let overlapping_commands = [
            // 第一条命令现在允许写入整个几何包围盒。
            FrameCommand::Native {
                // 保存会与后一命令产生真实写区重叠的 Additive 矩形。
                operation: FrameRasterOp::FillRectAdditive {
                    // 保持与前一个场景相同的几何。
                    rect: FrameRect::new(1, 1, 8, 4),
                    // 保持相同的 Additive 源色。
                    color: Color::green(),
                    // 扩大裁剪以包含 x=6..8 的重叠区域。
                    clip: FrameRect::new(1, 1, 8, 4),
                },
            },
            // 复用同一个半透明普通矩形。
            FrameCommand::Native {
                // 保存不能掩盖目标相关顺序的 SrcOver 写入。
                operation: FrameRasterOp::FillRect {
                    // 该矩形现在位于 Additive 的真实写区内。
                    rect: FrameRect::new(6, 1, 2, 4),
                    // 半透明源不提供不透明覆盖证明。
                    color: Color::from_rgba(255, 0, 0, 128),
                },
            },
        ];
        // 真实写区重叠且没有不透明 cover 时必须拒绝 Picture splice 分组。
        assert!(!source_over_commands_have_safe_grouping(
            &overlapping_commands,
            12,
            8
        ));
    }

    // Picture crop 平移必须收窄硬矩形，并为圆角矩形保留原始 SDF 几何。
    #[test]
    fn additive_picture_translation_preserves_clip_semantics() {
        // 定义从 12×8 Picture 裁出右侧区域并平移到更大目标的映射。
        let translation = PictureCropTranslation {
            // Picture 原始逻辑宽度。
            source_width: 12,
            // Picture 原始逻辑高度。
            source_height: 8,
            // crop 从 x=5 开始，进一步收窄原命令裁剪。
            source_crop: FrameRect::new(5, 0, 5, 8),
            // 目标位置向右平移十个逻辑像素。
            dx: 10,
            // 目标位置向下平移三个逻辑像素。
            dy: 3,
            // 目标宽度足以容纳完整平移几何。
            target_width: 30,
            // 目标高度足以容纳完整平移几何。
            target_height: 20,
        };
        // 构造同时受命令 clip 与 Picture crop 限制的硬 Additive 矩形。
        let hard_command = FrameCommand::Native {
            // 保存原始几何、颜色和局部裁剪。
            operation: FrameRasterOp::FillRectAdditive {
                // 原始矩形覆盖 x=2..10、y=1..7。
                rect: FrameRect::new(2, 1, 8, 6),
                // 使用绿色源色检查载荷保持。
                color: Color::green(),
                // 命令裁剪先将可见区域限制到 x=4..8、y=2..6。
                clip: FrameRect::new(4, 2, 4, 4),
            },
        };
        // 执行硬矩形的裁剪平移。
        let translated_hard =
            match crop_and_translate_source_over_command(&hard_command, &translation) {
                // 保存成功产生的命令。
                Ok(Some(command)) => command,
                // 非空且界内的映射不得被裁空或拒绝。
                result => panic!("hard additive translation should succeed: {result:?}"),
            };
        // 提取平移后的硬 Additive 载荷。
        let FrameCommand::Native {
            operation: FrameRasterOp::FillRectAdditive { rect, color, clip },
        } = translated_hard
        else {
            // 变体改变会丢失原始目标相关 blend 事实；附带回退值便于定位。
            panic!("expected translated hard additive command, got {translated_hard:?}");
        };
        // 硬矩形可以精确收窄为 clip 与 crop 的平移交集。
        assert_eq!(rect, FrameRect::new(15, 5, 3, 4));
        // 收窄后的 rect 本身也应成为精确裁剪。
        assert_eq!(clip, rect);
        // 平移不得改变调用方源色。
        assert_eq!(color, Color::green());
        // 构造相同几何和裁剪的圆角 Additive 命令。
        let rounded_command = FrameCommand::Native {
            // 保存共享 SDF 圆角载荷。
            operation: FrameRasterOp::FillRoundedRectAdditive {
                // 圆角中心依赖这一完整原始矩形。
                rect: FrameRect::new(2, 1, 8, 6),
                // 保持与硬矩形相同的源色。
                color: Color::green(),
                // 零圆角仍走圆角变体，并便于直接比较载荷。
                radius: FrameRadius::zero(),
                // 保持与硬矩形相同的命令裁剪。
                clip: FrameRect::new(4, 2, 4, 4),
            },
        };
        // 执行圆角矩形的裁剪平移。
        let translated_rounded =
            match crop_and_translate_source_over_command(&rounded_command, &translation) {
                // 保存成功产生的命令。
                Ok(Some(command)) => command,
                // 非空且界内的映射不得被裁空或拒绝。
                result => panic!("rounded additive translation should succeed: {result:?}"),
            };
        // 提取平移后的圆角 Additive 载荷。
        let FrameCommand::Native {
            operation:
                FrameRasterOp::FillRoundedRectAdditive {
                    rect,
                    color,
                    radius,
                    clip,
                },
        } = translated_rounded
        else {
            // 变体改变会绕开共享 SDF 几何；附带回退值便于定位。
            panic!("expected translated rounded additive command, got {translated_rounded:?}");
        };
        // 圆角矩形必须整体平移，不能像硬矩形一样收窄几何。
        assert_eq!(rect, FrameRect::new(12, 4, 8, 6));
        // 只有真实可见交集应成为平移后的局部裁剪。
        assert_eq!(clip, FrameRect::new(15, 5, 3, 4));
        // 平移不得改变调用方源色。
        assert_eq!(color, Color::green());
        // 平移不得改变共享 SDF 的圆角载荷。
        assert_eq!(radius, FrameRadius::zero());
    }
