//! `draw/painting/display_list.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use crate::draw::resources::font::text_backend::TextLayout;

// DisplayList 写时复制不得深拷贝已经共享的字形与行缓冲。
#[test]
fn cloned_glyph_op_shares_layout_storage() {
    let layout = Arc::new(TextLayout {
        glyphs: Vec::new(),
        lines: Vec::new(),
        width: 0.0,
        height: 0.0,
    });
    let operation = PaintOp::BlitGlyphLayout {
        layout: Arc::clone(&layout),
        pos: Point::new(1.0, 2.0),
        color: Color::black(),
        font_size: 14.0,
    };

    let cloned = operation.clone();

    let PaintOp::BlitGlyphLayout {
        layout: cloned_layout,
        ..
    } = cloned
    else {
        unreachable!("克隆后的操作类型必须保持不变");
    };
    assert!(Arc::ptr_eq(&layout, &cloned_layout));
}

// 唯一所有的显示列表重录应复用稳定文字，并截断本轮未覆盖的旧尾部。
#[test]
fn rewrite_reuses_text_storage_and_truncates_old_tail() {
    let text: Arc<str> = Arc::from("steady");
    let mut list = DisplayList::new();
    list.push(PaintOp::DrawText {
        text: Arc::clone(&text),
        pos: Point::new(1.0, 2.0),
        color: Color::black(),
        font_size: 12.0,
    });
    list.push(PaintOp::FillRect {
        rect: Rect::new(0.0, 0.0, 4.0, 4.0),
        color: Color::red(),
        radius: None,
    });

    list.begin_rewrite();
    list.push_reusing(
        |op| match op {
            PaintOp::DrawText {
                text: recorded_text,
                pos,
                color,
                font_size,
            } if recorded_text.as_ref() == "steady" => {
                *pos = Point::new(3.0, 4.0);
                *color = Color::blue();
                *font_size = 14.0;
                true
            }
            _ => false,
        },
        || unreachable!("稳定文字必须复用旧操作"),
    );
    list.finish_rewrite();

    let [
        PaintOp::DrawText {
            text: rewritten_text,
            pos,
            color,
            font_size,
        },
    ] = list.ops()
    else {
        panic!("重录后只应保留一条文字操作");
    };
    assert!(Arc::ptr_eq(&text, rewritten_text));
    assert_eq!(*pos, Point::new(3.0, 4.0));
    assert_eq!(*color, Color::blue());
    assert_eq!(*font_size, 14.0);
}

// 存在外部快照时，原位重录仍必须遵守 Arc 写时复制并保留旧内容。
#[test]
fn rewrite_preserves_shared_snapshot_content() {
    let mut list = DisplayList::new();
    list.push(PaintOp::FillRect {
        rect: Rect::new(0.0, 0.0, 4.0, 4.0),
        color: Color::red(),
        radius: None,
    });
    let snapshot = list.clone();

    list.begin_rewrite();
    list.push(PaintOp::FillRect {
        rect: Rect::new(1.0, 1.0, 2.0, 2.0),
        color: Color::blue(),
        radius: None,
    });
    list.finish_rewrite();

    let PaintOp::FillRect {
        rect: snapshot_rect,
        color: snapshot_color,
        ..
    } = &snapshot.ops()[0]
    else {
        panic!("快照应保留原矩形操作");
    };
    let PaintOp::FillRect {
        rect: rewritten_rect,
        color: rewritten_color,
        ..
    } = &list.ops()[0]
    else {
        panic!("重录列表应保留新矩形操作");
    };
    assert_eq!(*snapshot_rect, Rect::new(0.0, 0.0, 4.0, 4.0));
    assert_eq!(*snapshot_color, Color::red());
    assert_eq!(*rewritten_rect, Rect::new(1.0, 1.0, 2.0, 2.0));
    assert_eq!(*rewritten_color, Color::blue());
    assert!(!list.shares_operation_storage_with(&snapshot));
}

// 操作数量骤减后不应长期驻留峰值显示列表容量。
#[test]
fn rewrite_releases_oversized_operation_capacity_after_shrink() {
    let mut list = DisplayList::new();
    for x in 0..256 {
        list.push(PaintOp::FillRect {
            rect: Rect::new(x as f32, 0.0, 1.0, 1.0),
            color: Color::red(),
            radius: None,
        });
    }
    let peak_capacity = list.capacity();

    list.begin_rewrite();
    list.push(PaintOp::FillRect {
        rect: Rect::new(0.0, 0.0, 1.0, 1.0),
        color: Color::blue(),
        radius: None,
    });
    list.finish_rewrite();

    assert!(list.capacity() < peak_capacity);
    assert!(
        list.capacity()
            <= MIN_RETAINED_OP_CAPACITY.saturating_mul(MAX_RETAINED_OP_CAPACITY_RATIO)
    );
}

// —— 自源文件移入的扩展 impl（impl DisplayList） ——

impl DisplayList {
    // 测试目标观测容量提示是否进入新列表，不暴露为公共绘制契约。
    #[cfg(test)]
    pub(crate) fn capacity(&self) -> usize {
        self.ops.capacity()
    }

    // 测试目标观测原位重录是否保留同一操作数组分配。
    #[cfg(test)]
    pub(crate) fn operation_storage_ptr(&self) -> *const PaintOp {
        self.ops.as_ptr()
    }

    // 测试目标保留操作存储共享性观测入口，供写时复制测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn shares_operation_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.ops, &other.ops)
    }
}
