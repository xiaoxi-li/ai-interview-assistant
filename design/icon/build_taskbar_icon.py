"""用用户提供的高清标记图生成应用图标。

输出两版底：白底（忠于原稿）、暗色底（凸显彩色标记）。
标记从白底上抠出并去白，避免贴到暗底时边缘残留白圈。
"""

import os

from PIL import Image, ImageDraw, ImageFilter

BASE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(BASE, "source", "user", "taskbar-mark.png")
OUT = os.path.join(BASE, "final", "user")
SIZE = 1024
RADIUS = 0.2237
MARK_RATIO = 0.88
DARK_BG_TOP, DARK_BG_BOTTOM = (27, 36, 64), (15, 20, 32)   # 同客户端深色主题


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


def mark_rgba():
    """裁出标记 → 抠底去白 → 缩放到目标占比，返回 RGBA 图层。"""
    im = Image.open(SRC).convert("RGB")
    px = im.load()
    w, h = im.size
    x0, y0, x1, y1 = w, h, 0, 0
    for y in range(h):
        for x in range(w):
            if min(px[x, y]) < 246:
                x0, y0 = min(x0, x), min(y0, y)
                x1, y1 = max(x1, x), max(y1, y)

    alpha = Image.new("L", (x1 - x0, y1 - y0), 0)
    ap = alpha.load()
    for y in range(y0, y1):
        for x in range(x0, x1):
            r, g, b = px[x, y]
            ap[x - x0, y - y0] = min(255, max(0, (max(r, g, b) - min(r, g, b) - 6) * 18))

    scale = SIZE * MARK_RATIO / alpha.height
    size = (round(alpha.width * scale), round(alpha.height * scale))
    crop = im.crop((x0, y0, x1, y1)).resize(size, Image.Resampling.LANCZOS)
    alpha = alpha.resize(size, Image.Resampling.LANCZOS)

    alpha = alpha.filter(ImageFilter.MinFilter(3))   # 收缩 1px，削掉白底残留的边缘
    out = Image.new("RGBA", size, (0, 0, 0, 0))
    cp, op = crop.load(), out.load()
    ap2 = alpha.load()
    for y in range(size[1]):
        for x in range(size[0]):
            a = ap2[x, y]
            if a >= 110:                              # 半透明边缘一律丢弃，避免暗底上出现白线
                op[x, y] = cp[x, y] + (a,)
    return out


def main():
    mark = mark_rgba()
    pos = ((SIZE - mark.width) // 2, (SIZE - mark.height) // 2)
    os.makedirs(OUT, exist_ok=True)

    dark = rounded(gradient(SIZE, DARK_BG_TOP, DARK_BG_BOTTOM))
    dark.alpha_composite(mark, pos)
    dark.save(os.path.join(OUT, "app-icon-dark-1024.png"))

    white = rounded(Image.new("RGB", (SIZE, SIZE), (255, 255, 255)))
    white.alpha_composite(mark, pos)
    white.save(os.path.join(OUT, "app-icon-taskbar-1024.png"))
    print("mark", mark.size, "-> app-icon-dark-1024 / app-icon-taskbar-1024")


if __name__ == "__main__":
    main()
