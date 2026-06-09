#[cfg(test)]
mod tests {
    use fontdue::{Font, FontSettings};
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    
    #[test]
    fn test_coverage_layout() {
        let mut log = OpenOptions::new().create(true).append(true).open("/tmp/ttf_cov.txt").unwrap();
        
        let data = fs::read("/usr/share/fonts/liberation-sans-fonts/LiberationSans-Regular.ttf").unwrap();
        let font = Font::from_bytes(data, FontSettings::default()).unwrap();
        let fs = 20.0;
        
        let (metrics, cov) = font.rasterize('H', fs);
        let w = metrics.width;
        let h = metrics.height;
        writeln!(log, "'H': ymin={} height={} width={} xmin={}", metrics.ymin, h, w, metrics.xmin).unwrap();
        
        if h > 2 && w > 2 {
            let mid_col = w / 2;
            let top_pixel = cov[mid_col];
            let bot_pixel = cov[(h-1) * w + mid_col];
            writeln!(log, "  top_pixel(row=0,col={})={} bot_pixel(row={},col={})={}", 
                mid_col, top_pixel, h-1, mid_col, bot_pixel).unwrap();
        }
        
        let half_row = h / 2;
        let mut top_half_pixels = 0u32;
        let mut bot_half_pixels = 0u32;
        for row in 0..h {
            for col in 0..w {
                if cov[row * w + col] > 0 {
                    if row < half_row { top_half_pixels += 1; }
                    else { bot_half_pixels += 1; }
                }
            }
        }
        writeln!(log, "  top_half(row<{})={} bot_half(row>={})={}", 
            half_row, top_half_pixels, half_row, bot_half_pixels).unwrap();
        
        if top_half_pixels > bot_half_pixels {
            writeln!(log, "  => coverage[0] = TOP of glyph").unwrap();
        } else {
            writeln!(log, "  => coverage[0] = BOTTOM of glyph").unwrap();
        }
        
        writeln!(log, "  ymin(bottom)={} top=ymin-height+1={}", metrics.ymin, metrics.ymin - h as i32 + 1).unwrap();
        
        // 也检查 CJK 字符 '你' 在 LXGWWenKai 中的数据
        let wk_data = fs::read("/home/yang86/.local/share/fonts/l/LXGWWenKai_Regular.ttf").unwrap();
        let wk_font = Font::from_bytes(wk_data, FontSettings::default()).unwrap();
        let (m2, cov2) = wk_font.rasterize('你', 14.0);
        writeln!(log, "'你' LXGWWenKai: ymin={} height={} width={} xmin={}", m2.ymin, m2.height, m2.width, m2.xmin).unwrap();
        
        let mut top_px = 0u32;
        let mut bot_px = 0u32;
        let half2 = m2.height / 2;
        for row in 0..m2.height {
            for col in 0..m2.width {
                if cov2[row * m2.width + col] > 0 {
                    if row < half2 { top_px += 1; }
                    else { bot_px += 1; }
                }
            }
        }
        writeln!(log, "  top_half={} bot_half={}", top_px, bot_px).unwrap();
        writeln!(log, "  => coverage row 0 在 {} 有更多像素", if top_px > bot_px { "上方(TOP)" } else { "下方(BOTTOM)" }).unwrap();
    }
}
