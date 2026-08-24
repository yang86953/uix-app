use crate::core::{Constraints, Point, Rect, Size};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{SnapshotFields, WidgetTree};
use crate::widget;
use qrcode::{EcLevel, QrCode, types::Color as QrModuleColor};

// ════════════════════════════════════════════════════════════════════════════
// QRCode
// ════════════════════════════════════════════════════════════════════════════

widget! {
    /// 将文本值编码并绘制为可配置尺寸与纠错等级的二维码组件。
    pub struct QRCode {
        value: String,
        size: f32,
        error_level: u8,
        modules: Vec<bool>,
        module_count: usize,
        encoding_error: Option<String>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(self.size, self.size))
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        // QR 码默认底色/前景色：黑白 token（可被用户配置覆盖）。
        let bg = ctx.tokens().color_white();
        let fg = ctx.tokens().color_black();
        ctx.fill_rect(frame, bg, None);

        if self.encoding_error.is_some() || self.module_count == 0 {
            // QR 错误提示字号：统一使用主题 font_size token。
            ctx.text_center("QR !", frame, ctx.tokens().color_error(), ctx.tokens().font_size());
            return;
        }

        // ISO/IEC 18004 要求四模块 quiet zone；使用正方形模块并居中，
        // 不叠加 logo 或圆角，以免破坏编码矩阵的可扫描性。
        const QUIET_ZONE: usize = 4;
        let symbol_modules = self.module_count + QUIET_ZONE * 2;
        let module_size = frame.w.min(frame.h) / symbol_modules as f32;
        if !module_size.is_finite() || module_size <= 0.0 {
            return;
        }
        let symbol_size = module_size * symbol_modules as f32;
        let origin = Point::new(
            frame.x + (frame.w - symbol_size) * 0.5 + QUIET_ZONE as f32 * module_size,
            frame.y + (frame.h - symbol_size) * 0.5 + QUIET_ZONE as f32 * module_size,
        );
        // 相邻深色模块合并为同一水平矩形，减少绘制命令且保持矩阵并集完全一致。
        self.for_each_dark_run(|y, start_x, run_length| {
            ctx.fill_rect(
                Rect::new(
                    origin.x + start_x as f32 * module_size,
                    origin.y + y as f32 * module_size,
                    run_length as f32 * module_size,
                    module_size,
                ),
                fg,
                None,
            );
        });
    }
}

// 把二维码编码矩阵与绘制内核融合为 UIX 声明的单一叶节点。
fn build_qrcode_view(kernel: QRCode) -> ViewNode {
    ViewNode::leaf(kernel)
}

impl View for QRCode {
    fn build(self) -> ViewNode {
        // UIX 拥有公开组件根，Rust 内核继续独占编码、缓存矩阵与绘制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/qrcode/qrcode.uix")
    }
}

impl QRCode {
    /// 创建承载指定文本且使用默认尺寸和纠错等级的二维码。
    pub fn new(value: &str) -> Self {
        let mut qr = Self {
            value: value.to_string(),
            size: 160.0,
            error_level: 1,
            modules: Vec::new(),
            module_count: 0,
            encoding_error: None,
        };
        qr.rebuild_encoding();
        qr
    }
    /// 设置二维码组件的方形边长。
    pub fn size(mut self, s: f32) -> Self {
        if s.is_finite() {
            self.size = s.max(1.0);
        }
        self
    }
    /// 设置从低到高编号为 `0..=3` 的纠错等级。
    pub fn error_level(mut self, lv: u8) -> Self {
        self.error_level = lv.min(3);
        self.rebuild_encoding();
        self
    }

    /// 返回标准 QR 矩阵的边长（不含四模块 quiet zone）。
    pub fn module_count(&self) -> usize {
        self.module_count
    }

    /// 编码失败时返回具体原因；成功时为 `None`。
    pub fn encoding_error(&self) -> Option<&str> {
        self.encoding_error.as_deref()
    }

    /// 返回当前内容是否已成功编码为非空二维码矩阵。
    pub fn is_valid(&self) -> bool {
        self.encoding_error.is_none() && self.module_count > 0
    }

    // 逐行流式交付连续深色模块，不分配中间区间集合。
    #[inline]
    fn for_each_dark_run(&self, mut visit: impl FnMut(usize, usize, usize)) {
        // 每行独立扫描，禁止跨行合并破坏二维码模块边界。
        for y in 0..self.module_count {
            let row_start = y * self.module_count;
            let mut x = 0;
            while x < self.module_count {
                // 跳过当前行的浅色模块。
                while x < self.module_count && !self.modules[row_start + x] {
                    x += 1;
                }
                let run_start = x;
                // 消费紧邻的全部深色模块。
                while x < self.module_count && self.modules[row_start + x] {
                    x += 1;
                }
                // 行尾没有深色模块时不提交空矩形。
                if run_start < x {
                    visit(y, run_start, x - run_start);
                }
            }
        }
    }

    // 测试目标保留二维码模块观测入口，供编码矩阵测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn module(&self, x: usize, y: usize) -> Option<bool> {
        (x < self.module_count && y < self.module_count)
            .then(|| self.modules[y * self.module_count + x])
    }

    fn rebuild_encoding(&mut self) {
        let level = match self.error_level {
            0 => EcLevel::L,
            1 => EcLevel::M,
            2 => EcLevel::Q,
            _ => EcLevel::H,
        };
        match QrCode::with_error_correction_level(self.value.as_bytes(), level) {
            Ok(code) => {
                self.module_count = code.width();
                self.modules = code
                    .into_colors()
                    .into_iter()
                    .map(|module| module == QrModuleColor::Dark)
                    .collect();
                self.encoding_error = None;
            }
            Err(error) => {
                self.modules.clear();
                self.module_count = 0;
                self.encoding_error = Some(error.to_string());
            }
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.value = next.value;
        self.size = next.size;
        self.error_level = next.error_level;
        self.modules = next.modules;
        self.module_count = next.module_count;
        self.encoding_error = next.encoding_error;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::QRCode {
            value: self.value.clone(),
            size: self.size,
            error_level: self.error_level,
            module_count: self.module_count,
            encoding_error: self.encoding_error.clone(),
        }
    }
}

// 集中验证 UIX 声明壳与二维码 Rust 内核的单叶契约。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/display/qrcode_tests.rs"]
mod tests;
