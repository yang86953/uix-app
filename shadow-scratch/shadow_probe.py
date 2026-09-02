#!/usr/bin/env python3
"""桌面阴影测量：检测窗口精确边界，采样四边/四角阴影衰减剖面。

用法: shadow_probe.py <fg.png> <bg.png> [--label L]
fg: 含窗口的桌面截图; bg: 同屏无窗口(最小化)截图。逐位差分得到阴影 alpha。
"""
import sys

from PIL import Image

NEAR_WHITE = 235  # 窗口浅色内容判定阈值


def detect_window_rect(im):
    """在大片彩色壁纸上定位近白矩形窗口的边界。"""
    w, h = im.size
    px = im.load()

    def row_white_run(y):
        run = best = 0
        start = best_start = 0
        for x in range(w):
            r, g, b = px[x, y][:3]
            if r > NEAR_WHITE and g > NEAR_WHITE and b > NEAR_WHITE:
                if run == 0:
                    start = x
                run += 1
                if run > best:
                    best, best_start = run, start
            else:
                run = 0
        return best, best_start

    # 从上向下找第一条 >800px 的近白行作为标题栏顶
    top = None
    for y in range(0, h // 2):
        run, _ = row_white_run(y)
        if run > 800:
            top = y
            break
    if top is None:
        return None
    # 从下向上找最后一条
    bottom = None
    for y in range(h - 1, h // 2, -1):
        run, _ = row_white_run(y)
        if run > 800:
            bottom = y
            break
    if bottom is None:
        return None
    midy = (top + bottom) // 2
    row = [px[x, midy][:3] for x in range(w)]
    left = next(x for x in range(w) if all(c > NEAR_WHITE for c in row[x]))
    right = next(x for x in range(w - 1, -1, -1) if all(c > NEAR_WHITE for c in row[x]))
    return left, top, right, bottom


def main():
    fg = Image.open(sys.argv[1]).convert("RGB")
    bg = Image.open(sys.argv[2]).convert("RGB")
    label = "unlabeled"
    if "--label" in sys.argv:
        label = sys.argv[sys.argv.index("--label") + 1]
    rect = detect_window_rect(fg)
    if rect is None:
        print("!! 未检测到窗口")
        return 1
    left, top, right, bottom = rect
    print(f"[{label}] window rect: ({left},{top})-({right},{bottom}) "
          f"size={right-left+1}x{bottom-top+1}")
    df = fg.load()
    db = bg.load()

    def band_alpha(x, y):
        """黑色阴影的 alpha 估计: 1 - fg/bg (逐通道均值)。"""
        fr, fgc, fb = df[x, y]
        br, bgc, bb = db[x, y]
        if min(br, bgc, bb) < 30:
            return None  # 背景过暗不可靠
        ratios = [fr / br, fgc / bgc, fb / bb]
        if any(r > 1.05 for r in ratios):
            return None  # 非?暗化像素(内容变化)
        return 1.0 - sum(ratios) / 3.0

    for dist in (4, 8, 16, 24, 31, 40):
        cy, cx = (top + bottom) // 2, (left + right) // 2
        vals = {
            "L": band_alpha(left - 1 - dist, cy),
            "T": band_alpha(cx, top - 1 - dist),
            "R": band_alpha(right + 1 + dist, cy),
            "B": band_alpha(cx, bottom + 1 + dist),
        }
        pretty = " ".join(
            f"{k}={v:.3f}" if v is not None else f"{k}=n/a" for k, v in vals.items())
        print(f"  dist={dist:3d}px  {pretty}")

    # 四角: 对角距离 (d, d)
    for d in (4, 12, 24):
        vals = {
            "TL": band_alpha(left - 1 - d, top - 1 - d),
            "TR": band_alpha(right + 1 + d, top - 1 - d),
            "BL": band_alpha(left - 1 - d, bottom + 1 + d),
            "BR": band_alpha(right + 1 + d, bottom + 1 + d),
        }
        pretty = " ".join(
            f"{k}={v:.3f}" if v is not None else f"{k}=n/a" for k, v in vals.items())
        print(f"  corner d={d:3d}px  {pretty}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
