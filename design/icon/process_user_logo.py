"""处理用户提供的两张高清 logo：去水印 → 应用图标 / 随主题换色的横版标识。"""

import os

from PIL import Image

BASE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(BASE, "source", "user")
OUT = os.path.join(BASE, "final", "user")
WM = (850, 990, 1080, 1080)          # 右下角水印区域（纯背景，直接填白）
DARK_TEXT = (15, 23, 42)             # 原稿文字色（浅色主题用）
LIGHT_TEXT = (250, 251, 255)         # 深色主题用文字色


def dewatermark(im):
    px = im.load()
    for y in range(WM[1], WM[3]):
        for x in range(WM[0], WM[2]):
            px[x, y] = (255, 255, 255)
    return im


def content_bbox(im, pad=8):
    px = im.load()
    w, h = im.size
    x0, y0, x1, y1 = w, h, 0, 0
    for y in range(h):
        for x in range(w):
            if min(px[x, y]) < 246:
                x0, y0 = min(x0, x), min(y0, y)
                x1, y1 = max(x1, x), max(y1, y)
    return (max(0, x0 - pad), max(0, y0 - pad), min(w, x1 + pad), min(h, y1 + pad))


def build_app_icon():
    im = dewatermark(Image.open(os.path.join(SRC, "mark-hd.png")).convert("RGB"))
    os.makedirs(OUT, exist_ok=True)
    im.save(os.path.join(OUT, "mark-app-1024.png"))
    print("mark-app-1024", im.size)


def build_lockup():
    im = dewatermark(Image.open(os.path.join(SRC, "lockup-hd.png")).convert("RGB"))
    im = im.crop(content_bbox(im))
    w, h = im.size
    px = im.load()

    # 标记与文字之间是一条空白列，取它作为分界（避免按亮度误判标记的深蓝部分）
    blanks = [x for x in range(w)
              if all(min(px[x, y]) > 245 for y in range(0, h, 4))]
    mid = [x for x in blanks if w * 0.25 < x < w * 0.6]
    split = mid[len(mid) // 2] if mid else int(w * 0.45)
    print("split at", split, "of", w)

    for name, text in (("lockup-light.png", DARK_TEXT), ("lockup-dark.png", LIGHT_TEXT)):
        img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        op = img.load()
        for y in range(h):
            for x in range(w):
                r, g, b = px[x, y]
                if x < split:                      # 标记：保留原色，按饱和度抠底
                    sat = max(r, g, b) - min(r, g, b)
                    op[x, y] = (r, g, b, min(255, max(0, (sat - 12) * 6)))
                else:                              # 文字：按目标色着色
                    luma = (r * 299 + g * 587 + b * 114) // 1000
                    a = min(255, max(0, int((235 - luma) * 1.45)))
                    if a:
                        op[x, y] = text + (a,)
        img.save(os.path.join(OUT, name))
        print(name, img.size)


if __name__ == "__main__":
    build_app_icon()
    build_lockup()
