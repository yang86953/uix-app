//! 鑷畾涔?widget 绀轰緥 鈥?`define_widget!` + `prelude` 缁樺埗 API銆?
use uix::prelude::*;

// 鈹€鈹€ Counter 鈥?鑷畾涔?widget 绀轰緥 鈹€鈹€

define_widget! {
    pub struct Counter { pub count: u32 }
    @new -> Self { Self { count: 0 } }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size { Size::new(120.0, 36.0) }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event { SystemEvent::PointerDown { .. } => { self.count += 1; EventResult::Handled } _ => EventResult::NotHandled }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_primary_bg();
        let color = ctx.tokens().color_primary();
        ctx.fill_rect(frame, bg, None);
        ctx.draw_text(&format!("Count: {}", self.count), Point::new(frame.x + 8.0, frame.y + 8.0), color, 14.0);
    }
}

// 鈹€鈹€ PulseRing 鈥?鑴夊啿鍔ㄧ敾 鈹€鈹€

define_widget! {
    pub struct PulseRing {
        pub time: f32,
    }
    @new -> Self { Self { time: 0.0 } }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(48.0, 48.0)
    }



    // 鍙繑鍥炶剦鍐插渾瀹為檯瑕嗙洊鐨勫尯鍩燂紝閬垮厤娓呴櫎鏁翠釜 frame 瀵艰嚧鍥涜鐧界嚎
    dirty_rect => (&self, frame: Rect) -> Rect {
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let max_r = 24.0; // 鏈€澶ц剦鍐插崐寰?鈮?6 + 16 + 3(stroke)
        Rect::new(cx - max_r, cy - max_r, max_r * 2.0, max_r * 2.0)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let phase = (self.time * 1.5).sin() * 0.5 + 0.5; // 0..1
        let r = 6.0 + phase * 16.0;
        let alpha = (1.0 - phase * 0.6) * 255.0;
        let base = ctx.tokens().color_primary();
        let c = Color::from_rgba(base.r, base.g, base.b, alpha as u8);
        let eng = ctx.canvas_2d();
        eng.stroke_circle(cx, cy, r, c, 3.0);
        if r > 10.0 {
            eng.fill_circle(cx, cy, r * 0.3, c);
        }
    }
}

// 鈹€鈹€ BounceBall 鈥?寮硅烦鍔ㄧ敾 鈹€鈹€

define_widget! {
    pub struct BounceBall {
        pub time: f32,
    }
    @new -> Self { Self { time: 0.0 } }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(200.0, 60.0)
    }



    // 鍙繑鍥炵悆 + 闃村奖鐨勮竟鐣屾锛岄伩鍏嶆竻闄ゆ暣涓?frame
    dirty_rect => (&self, frame: Rect) -> Rect {
        let cx = frame.x + frame.w * 0.5;
        let ball_top = frame.y + frame.h - 50.0;
        let ball_bot = frame.y + frame.h;
        let max_r = 14.0;
        Rect::new(cx - max_r, ball_top - max_r, max_r * 2.0, ball_bot - ball_top + max_r * 2.0)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let t = self.time / 2.0; // 0..1
        let bounce = if t < 0.5 {
            1.0 - (t * 2.0).powf(2.0)
        } else {
            -((t - 0.5) * 2.0 - 1.0).powf(2.0) + 1.0
        };
        let cy = frame.y + frame.h - 10.0 - bounce * 40.0;
        let cx = frame.x + frame.w * 0.5;
        let primary = ctx.tokens().color_primary();
        // 褰卞瓙锛堟牴鎹珮搴﹀彉鍖栧ぇ灏忓拰閫忔槑搴︼級
        let shadow_alpha = (0.3 + bounce * 0.5 * 0.7) * 255.0;
        let shadow_r = 6.0 + bounce * 8.0;
        let shadow_c = Color::from_rgba(0, 0, 0, shadow_alpha as u8);
        ctx.fill_circle(cx, frame.y + frame.h - 6.0, shadow_r, shadow_c);
        ctx.canvas_2d().fill_circle(cx, cy, 10.0, primary);
    }
}
