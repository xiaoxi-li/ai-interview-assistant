"""生成任务栏 / 桌面用图标：去水印 → 抠出标记 → 放大 → 合成圆角方形底。"""

import os

from PIL import Image, ImageDraw

BASE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(BASE, "source", "user", "mark-hd.png")
OUT = os.path.join(BASE, "final", "user")
WM = (850, 990, 1080, 1080)   # 右下角水印，直接填白
SIZE = 1024
RADIUS = 0.2237               # 圆角比例
MARK_RATIO = 0.66             # 标记高度占图标高度（原图约 0.46，放大以突出品牌）
PAD = 6


def mark_layer():
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
    out = Image.new("RGBA", crop.size, (0, 0, 0, 0))
    cp, op = crop.load(), out.load()
    for y in range(crop.height):
        for x in range(crop.width):
            r, g, b = cp[x, y]
            sat = max(r, g, b) - min(r, g, b)
            op[x, y] = (r, g, b, min(255, max(0, (sat - 12) * 6)))
    return out


def main():
    mark = mark_layer()
    scale = SIZE * MARK_RATIO / mark.height
    mark = mark.resize((round(mark.width * scale), round(mark.height * scale)),
                       Image.Resampling.LANCZOS)

    card = Image.new("RGBA", (SIZE, SIZE), (255, 255, 255, 255))
    mask = Image.new("L", (SIZE * 2, SIZE * 2), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, SIZE * 2 - 1, SIZE * 2 - 1],
                                           radius=round(RADIUS * SIZE * 2), fill=255)
    card.putalpha(mask.resize((SIZE, SIZE), Image.Resampling.LANCZOS))
    card.alpha_composite(mark, ((SIZE - mark.width) // 2, (SIZE - mark.height) // 2))
    os.makedirs(OUT, exist_ok=True)
    card.save(os.path.join(OUT, "app-icon-rounded-1024.png"))
    print("app-icon-rounded-1024", card.size, "mark", mark.size)


if __name__ == "__main__":
    main()
