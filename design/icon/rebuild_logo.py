"""按品牌设计稿 1:1 复刻 logo。

只做两件事：把设计稿中的像素**上采样**（LANCZOS）并对边缘做**阈值锐化**，
使放大后的轮廓干净清晰。几何形状、比例、位置、渐变方向全部取自设计稿，
不做任何重新设计。

用法：py -3 rebuild_logo.py
"""

import os

from PIL import Image, ImageDraw, ImageFilter

BASE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(BASE, "source", "crops")
OUT = os.path.join(BASE, "final", "rebuilt")
BIG = 1120          # 上采样后的工作尺寸
K = 2.2             # 边缘锐化系数（过大在低分辨率源上会产生阶梯）
RADIUS_RATIO = 0.2237


def lut(k):
    return [max(0, min(255, int((v / 255 - 0.5) * k * 255 + 127.5))) for v in range(256)]


def up(mask, w, h, k=K):
    """先轻度模糊让低分辨率边缘连续，再放大并做温和的阈值锐化。"""
    soft = mask.filter(ImageFilter.GaussianBlur(0.8))
    return soft.resize((w, h), Image.Resampling.LANCZOS).point(lut(k))


def card_mask(size, ratio=RADIUS_RATIO):
    n = size * 2
    m = Image.new("L", (n, n), 0)
    ImageDraw.Draw(m).rounded_rectangle([0, 0, n - 1, n - 1],
                                        radius=int(ratio * n), fill=255)
    return m.resize((size, size), Image.Resampling.LANCZOS)


def lerp(a, b, t):
    return tuple(int(round(a[i] + (b[i] - a[i]) * t)) for i in range(3))


def gradient(size, c0, c1):
    """竖向渐变再按对角线混合，方向取自设计稿（左上亮、右下深）。"""
    t = Image.linear_gradient("L").resize((size, size), Image.Resampling.BILINEAR)
    chans = [t.point([lerp(c0, c1, v / 255)[i] for v in range(256)]) for i in range(3)]
    return Image.merge("RGB", chans)


def build_app_icon():
    """应用图标：蓝底圆角卡片 + 白色横杠，几何与底色来自设计稿。"""
    src = Image.open(os.path.join(SRC, "app_icon.png")).convert("RGB")
    w, h = src.size
    mask = Image.new("L", (w, h), 0)
    mp = mask.load()
    for y in range(h):
        for x in range(w):
            r, g, b = src.getpixel((x, y))
            mp[x, y] = 255 if min(r, g, b) > 170 else 0

    alpha = up(mask, BIG, BIG)
    top = src.getpixel((w // 2, max(1, h // 10)))
    bottom = src.getpixel((w // 2, h - max(2, h // 10)))
    card = gradient(BIG, top, bottom).convert("RGBA")
    card.putalpha(card_mask(BIG))

    white = Image.new("RGBA", (BIG, BIG), (255, 255, 255, 255))
    white.putalpha(alpha)
    out = Image.alpha_composite(card, white)
    os.makedirs(OUT, exist_ok=True)
    out.save(os.path.join(OUT, "app-icon-1024.png"))
    print("app-icon", out.size, "bg", top, "->", bottom)


def build_mark():
    """彩色标记：透明底，颜色与形状取自设计稿。"""
    src = Image.open(os.path.join(SRC, "hero_mark.png")).convert("RGB")
    w, h = src.size
    ink = Image.new("L", (w, h), 0)
    ip = ink.load()
    for y in range(h):
        for x in range(w):
            r, g, b = src.getpixel((x, y))
            d = max(r, g, b) - min(r, g, b)   # 白底近乎无饱和度，彩色墨迹饱和度高
            ip[x, y] = min(255, max(0, int((d - 12) * 2.2)))

    scale = 560 / max(w, h)
    nw, nh = int(w * scale), int(h * scale)
    color = src.resize((nw, nh), Image.Resampling.LANCZOS).convert("RGBA")
    color.putalpha(up(ink, nw, nh))
    px = color.load()          # 去白底：按 alpha 反推本色，消除边缘白边
    for y in range(nh):
        for x in range(nw):
            r, g, b, a = px[x, y]
            if a < 8:
                px[x, y] = (r, g, b, 0)
            else:
                f = a / 255.0
                px[x, y] = tuple(
                    min(255, max(0, int((c - (1 - f) * 255) / f)))
                    for c in (r, g, b)) + (a,)
    color.save(os.path.join(OUT, "mark-1024.png"))
    print("mark", color.size)


if __name__ == "__main__":
    build_app_icon()
    build_mark()
