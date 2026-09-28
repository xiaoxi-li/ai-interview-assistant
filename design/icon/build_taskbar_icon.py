"""用用户提供的高清标记图生成应用图标：标记居中 + 白底圆角方形。

该图标记本就占高约 90%，无需放大；只做两件事：把标记水平/垂直居中，套 22.37% 圆角。
"""

import os

from PIL import Image, ImageDraw

BASE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(BASE, "source", "user", "taskbar-mark.png")
OUT = os.path.join(BASE, "final", "user")
SIZE = 1024
RADIUS = 0.2237
MARK_RATIO = 0.88        # 标记高度占图标比例（原图 0.898，略收留一点呼吸）


def main():
    im = Image.open(SRC).convert("RGB")
    px = im.load()
    w, h = im.size
    x0, y0, x1, y1 = w, h, 0, 0
    for y in range(h):
        for x in range(w):
            if min(px[x, y]) < 246:
                x0, y0 = min(x0, x), min(y0, y)
                x1, y1 = max(x1, x), max(y1, y)
    mark = im.crop((x0, y0, x1, y1))

    scale = SIZE * MARK_RATIO / mark.height
    mark = mark.resize((round(mark.width * scale), round(mark.height * scale)),
                       Image.Resampling.LANCZOS).convert("RGBA")

    card = Image.new("RGBA", (SIZE, SIZE), (255, 255, 255, 255))
    mask = Image.new("L", (SIZE * 2, SIZE * 2), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, SIZE * 2 - 1, SIZE * 2 - 1],
                                           radius=round(RADIUS * SIZE * 2), fill=255)
    card.putalpha(mask.resize((SIZE, SIZE), Image.Resampling.LANCZOS))
    card.alpha_composite(mark, ((SIZE - mark.width) // 2, (SIZE - mark.height) // 2))

    os.makedirs(OUT, exist_ok=True)
    card.save(os.path.join(OUT, "app-icon-taskbar-1024.png"))
    print("app-icon-taskbar-1024", card.size, "mark", mark.size)


if __name__ == "__main__":
    main()
