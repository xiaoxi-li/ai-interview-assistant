"""生成应用图标：去水印 → 抠标记 → 锐化放大 + 加深颜色 → 圆角方形底。

输出两版底：白底彩标记（忠于原稿）、品牌蓝底白标记（浅色任务栏下更醒目）。
"""

import os

from PIL import Image, ImageDraw, ImageFilter

BASE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(BASE, "source", "user", "mark-hd.png")
OUT = os.path.join(BASE, "final", "user")
WM = (850, 990, 1080, 1080)
SIZE = 1024
RADIUS = 0.2237
MARK_RATIO = 0.62
PAD = 6
SAT_GAIN, VAL_GAIN = 1.35, 0.86
BRAND_TOP, BRAND_BOTTOM = (39, 167, 254), (9, 98, 250)


def lut(k):
    return [max(0, min(255, int((v / 255 - 0.5) * k * 255 + 127.5))) for v in range(256)]


def upscale(mask, size, k=1.8):
    soft = mask.filter(ImageFilter.GaussianBlur(0.6))
    up = soft.resize(size, Image.Resampling.LANCZOS).point(lut(k))
    return up.filter(ImageFilter.GaussianBlur(0.5))


def enhance(rgb):
    h, s, v = rgb.convert("HSV").split()
    s = s.point(lambda x: min(255, int(x * SAT_GAIN)))
    v = v.point(lambda x: min(255, int(x * VAL_GAIN)))
    return Image.merge("HSV", (h, s, v)).convert("RGB")


def mark_layer(ratio=MARK_RATIO):
    im = Image.open(SRC).convert("RGB")
    px = im.load()
    for y in range(WM[1], WM[3]):
        for x in range(WM[0], WM[2]):
            px[x, y] = (255, 255, 255)
    w, h = im.size
    x0, y0, x1, y1 = w, h, 0, 0
    for y in range(h):
        for x in range(w):
            if min(px[x, y]) < 246:
                x0, y0 = min(x0, x), min(y0, y)
                x1, y1 = max(x1, x), max(y1, y)

    crop = im.crop((x0 - PAD, y0 - PAD, x1 + PAD, y1 + PAD))
    alpha = Image.new("L", crop.size, 0)
    cp, ap = crop.load(), alpha.load()
    for y in range(crop.height):
        for x in range(crop.width):
            r, g, b = cp[x, y]
            ap[x, y] = min(255, max(0, (max(r, g, b) - min(r, g, b) - 12) * 6))

    scale = SIZE * ratio / crop.height
    size = (round(crop.width * scale), round(crop.height * scale))
    out = enhance(crop).resize(size, Image.Resampling.LANCZOS).convert("RGBA")
    out.putalpha(upscale(alpha, size))
    return out


def gradient(size, c0, c1):
    t = Image.linear_gradient("L").resize((size, size), Image.Resampling.BILINEAR)
    chans = [t.point([int(c0[i] + (c1[i] - c0[i]) * v / 255) for v in range(256)])
             for i in range(3)]
    return Image.merge("RGB", chans)


def rounded(base, size=SIZE):
    card = base.convert("RGBA").copy()
    mask = Image.new("L", (size * 2, size * 2), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, size * 2 - 1, size * 2 - 1],
                                           radius=round(RADIUS * size * 2), fill=255)
    card.putalpha(mask.resize((size, size), Image.Resampling.LANCZOS))
    return card


def build_brand(ratio, name):
    """品牌蓝底 + 白色标记；ratio 控制标记占高比例。"""
    mark = mark_layer(ratio)
    brand = rounded(gradient(SIZE, BRAND_TOP, BRAND_BOTTOM))
    solid = Image.new("RGBA", mark.size, (255, 255, 255, 255))
    solid.putalpha(mark.getchannel("A"))
    brand.alpha_composite(solid, ((SIZE - mark.width) // 2, (SIZE - mark.height) // 2))
    brand.save(os.path.join(OUT, name))
    print(name, "mark", mark.size)


def main():
    os.makedirs(OUT, exist_ok=True)
    mark = mark_layer()
    white = rounded(Image.new("RGB", (SIZE, SIZE), (255, 255, 255)))
    white.alpha_composite(mark, ((SIZE - mark.width) // 2, (SIZE - mark.height) // 2))
    white.save(os.path.join(OUT, "app-icon-white-1024.png"))

    build_brand(MARK_RATIO, "app-icon-brand-1024.png")
    build_brand(0.78, "app-icon-brand-small-1024.png")   # ≤32px 用，放大标记避免糊成一团


if __name__ == "__main__":
    main()
