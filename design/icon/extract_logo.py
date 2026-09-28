"""从品牌设计稿（1080×720 位图）中**原样提取**各个 logo。

不做任何重绘或改色：用颜色特征（品牌蓝青色 / 纯白底）在指定区域内定位目标
像素的外接框，再按该框裁切，输出到 `source/crops/`。

用法：py -3 extract_logo.py
"""

import os

from PIL import Image

BASE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(BASE, "source", "brand-sheet.png")
OUT = os.path.join(BASE, "source", "crops")


def hue(p):
    r, g, b = (v / 255 for v in p)
    mx, mn = max(r, g, b), min(r, g, b)
    d = mx - mn
    if d == 0:
        return 0.0
    if mx == r:
        h = ((g - b) / d) % 6
    elif mx == g:
        h = (b - r) / d + 2
    else:
        h = (r - g) / d + 4
    return h * 60


def is_brand(p):
    """品牌蓝青色（#3B82F6 / #22D3A6 系）：中高饱和 + 蓝青色相。"""
    mx, mn = max(p), min(p)
    if mx - mn < 40 or mx < 90:
        return False
    return 145 <= hue(p) <= 265


def is_white(p):
    return min(p) > 232


def is_ink(p):
    """稿件底色（浅灰白）之外的任何内容：彩色标记与深色文字都算。"""
    return max(abs(p[i] - v) for i, v in enumerate((250, 251, 253))) > 30


# (名称, 判定函数, 搜索区域 x0,y0,x1,y1) —— 基于 1080×720 原图坐标
REGIONS = [
    ("hero_mark", is_brand, (110, 60, 250, 210)),          # 左上大标记
    ("lockup_horizontal", is_ink, (20, 316, 305, 410)),    # 横版组合（标记 + Easy + 副标）
    ("lockup_vertical", is_ink, (215, 316, 300, 410)),     # 竖版组合
    ("app_icon", is_brand, (300, 310, 420, 412)),          # 应用图标（蓝渐变底）
    ("desktop_icon", is_white, (90, 490, 240, 630)),       # 桌面端图标（白底，最大）
    ("concept_mark", is_brand, (575, 120, 650, 195)),      # 设计理念：字母 E
    ("ui_mark", is_brand, (302, 508, 330, 540)),           # 使用场景里的标记
]


def bbox_of(img, box, pred):
    x0, y0, x1, y1 = box
    minx, miny, maxx, maxy = x1, y1, x0, y0
    for y in range(y0, y1):
        for x in range(x0, x1):
            if pred(img.getpixel((x, y))):
                minx, miny = min(minx, x), min(miny, y)
                maxx, maxy = max(maxx, x), max(maxy, y)
    return (minx, miny, maxx + 1, maxy + 1) if minx <= maxx else None


def main():
    os.makedirs(OUT, exist_ok=True)
    sheet = Image.open(SRC).convert("RGB")
    for name, pred, box in REGIONS:
        bbox = bbox_of(sheet, box, pred)
        if not bbox:
            print(f"{name}: 未找到内容")
            continue
        crop = sheet.crop(bbox)
        crop.save(os.path.join(OUT, f"{name}.png"))
        print(f"{name:<18} bbox={tuple(bbox)}  size={crop.size}")


if __name__ == "__main__":
    main()
