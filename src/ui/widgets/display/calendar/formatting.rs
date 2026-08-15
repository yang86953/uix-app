//! 日历标题本地化与受约束字号适配。

// 标题模板消费 UI System 已解析的区域设置。
use crate::ui::component::locale::Locale;
// 字号适配只通过规范绘制上下文测量文字。
use crate::ui::component::paint_context::PaintContext;

// 生成当前区域设置下的年月标题。
pub(super) fn localized_month_title(locale: &Locale, year: i32, month: usize) -> String {
    // 把外部月份限制到本地化月份表范围。
    let month_index = month.saturating_sub(1).min(11);
    // 数字月份用于带年月后缀的区域设置。
    let numeric_month = (month_index + 1).to_string();
    // 年份先转为模板可替换的文本。
    let year = year.to_string();
    // 年或月格式带后缀时优先保留数字月份。
    let affixed_numeric = locale.year_format != "{0}" || locale.month_format != "{0}";
    // 按区域设置选择两个模板参数。
    let (first, second) = if affixed_numeric {
        // 带后缀格式使用年份与数字月份。
        (year.as_str(), numeric_month.as_str())
    } else {
        // 无后缀格式使用本地化月份名称与年份。
        (locale.months_long[month_index], year.as_str())
    };
    // 模板无效时回退到稳定、可读的年月格式。
    replace_two_placeholders(locale.month_year_format, first, second).unwrap_or_else(|| {
        // 带后缀区域设置回退为数字年月。
        if affixed_numeric {
            // 月份固定补齐两位。
            format!("{year}-{:02}", month_index + 1)
        } else {
            // 其它区域设置回退为月份名称加年份。
            format!("{} {year}", locale.months_long[month_index])
        }
    })
}

// 依次替换标题模板中的两个空占位符。
fn replace_two_placeholders(pattern: &str, first: &str, second: &str) -> Option<String> {
    // 只替换首个占位符。
    let with_first = pattern.replacen("{}", first, 1);
    // 缺少首个占位符表示模板不满足契约。
    if with_first == pattern {
        // 交给调用方选择稳定回退格式。
        return None;
    }
    // 在首轮结果中替换第二个占位符。
    let with_second = with_first.replacen("{}", second, 1);
    // 只有两个占位符都存在时交付结果。
    (with_second != with_first).then_some(with_second)
}

// 计算能同时放入宽高约束的有限字号。
pub(super) fn fitted_font_size(
    // 使用当前字体服务执行真实文字测量。
    ctx: &mut PaintContext,
    // 接收需要绘制的完整标题。
    text: &str,
    // 接收主题给出的首选字号。
    base_size: f32,
    // 接收标题区域的最大宽度。
    max_width: f32,
    // 接收标题区域的最大高度。
    max_height: f32,
) -> f32 {
    // 非有限或非正约束不能产生可绘制字号。
    if !base_size.is_finite()
        || base_size <= 0.0
        || !max_width.is_finite()
        || max_width <= 0.0
        || !max_height.is_finite()
        || max_height <= 0.0
    {
        // 返回零让绘制路径跳过标题。
        return 0.0;
    }
    // 先按首选字号测量完整标题。
    let measured = ctx.measure_text(text, base_size);
    // 计算宽度允许的缩放比例。
    let width_scale = if measured.w > 0.0 {
        // 有效测量按最大宽度收缩。
        max_width / measured.w
    } else {
        // 零宽文字无需因宽度收缩。
        1.0
    };
    // 计算高度允许的缩放比例。
    let height_scale = if measured.h > 0.0 {
        // 有效测量按最大高度收缩。
        max_height / measured.h
    } else {
        // 零高文字无需因高度收缩。
        1.0
    };
    // 只允许缩小，不放大超过主题首选字号。
    base_size * width_scale.min(height_scale).clamp(0.0, 1.0)
}
