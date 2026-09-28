"""轻松面试（Easy Interview）Logo 生成脚本 —— 纯几何绘制，颜色与品牌令牌严格一致。

设计要点：
- 主体「E」= Easy，三横杠同时读作声波/电平条（对应「实时转写」）
- 圆润胶囊化笔画呼应「轻松」；品牌渐变方圆形底
- 无外部素材、无水印，可重复渲染出所有平台尺寸

用法：py -3 render.py
"""

import io
import os
import struct

from PIL import Image, ImageDraw, ImageFilter

BASE = os.path.dirname(os.path.abspath(__file__))
FINAL = os.path.join(BASE, "final")
TAURI_ICONS = os.path.join(FINAL, "tauri-icons")

S = 1024          # 主尺寸
SS = 4            # 超采样倍数（抗锯齿）

# ---- 品牌令牌（与 src/index.css / tailwind.config.js 一致） ----
BRAND_PRIMARY = (79, 107, 237)    # #4F6BED
BRAND_LIGHT = (91, 124, 250)      # #5B7CFA
BRAND_DEEP = (43, 70, 196)        # #2B46C4
BRAND_PALE = (124, 154, 255)      # #7C9AFF
DARK_BG_1 = (27, 36, 64)          # 深色主题背景上端
DARK_BG_2 = (15, 20, 32)          # #0F1420 深色主题背景下端

# ---- 「E」几何（1024 画布坐标） ----
BAR_H = 104                       # 笔画粗细
RADIUS = BAR_H // 2               # 胶囊圆角
GAP = 140
MARK_TOP = 216
STEM_X = (252, 356)
ARM_LONG = (252, 772)
ARM_MID = (252, 680)

Y0 = MARK_TOP
Y1 = Y0 + BAR_H + GAP
Y2 = Y1 + BAR_H + GAP
Y3 = Y2 + BAR_H
MARK_BOTTOM = Y3

BARS = [
    (STEM_X[0], Y0, STEM_X[1], Y3),                       # 竖干
    (ARM_LONG[0], Y0, ARM_LONG[1], Y0 + BAR_H),           # 上横
    (ARM_MID[0], Y1, ARM_MID[1], Y1 + BAR_H),             # 中横（略短，读作声波）
    (ARM_LONG[0], Y2, ARM_LONG[1], Y2 + BAR_H),           # 下横
]

# 嵌套版：中横再缩短，给下开口腾出「asy」的位置
BARS_NESTED = [
    BARS[0],
    BARS[1],
    (ARM_MID[0], Y1, 620, Y1 + BAR_H),
    BARS[3],
]

# 「asy」小字字体候选（Windows）
FONT_CANDIDATES = [
    r"C:\Windows\Fonts\seguisb.ttf",   # Segoe UI Semibold
    r"C:\Windows\Fonts\segoeui.ttf",
    r"C:\Windows\Fonts\Verdana.ttf",
    r"C:\Windows\Fonts\arial.ttf",
    r"C:\Windows\Fonts\msyh.ttc",
]


def load_font(px):
    from PIL import ImageFont

    for path in FONT_CANDIDATES:
        if os.path.exists(path):
            try:
                return ImageFont.truetype(path, px)
            except OSError:
                continue
    return ImageFont.load_default()


def lerp(c0, c1, t):
    return tuple(int(round(c0[i] + (c1[i] - c0[i]) * t)) for i in range(3))


def colorize(t, c0, c1):
    """把 L 通道的 0..255 映射到 c0..c1 的颜色。"""
    chans = []
    for i in range(3):
        chans.append(t.point([lerp(c0, c1, v / 255)[i] for v in range(256)]))
    return Image.merge("RGB", chans)


def vertical_gradient(size, c0, c1):
    t = Image.linear_gradient("L").resize(size, Image.Resampling.BILINEAR)
    return colorize(t, c0, c1)


def diagonal_gradient(size, c0, c1, x_weight=0.42):
    gy = Image.linear_gradient("L").resize(size, Image.Resampling.BILINEAR)
    gx = Image.linear_gradient("L").rotate(90, expand=True).resize(
        size, Image.Resampling.BILINEAR
    )
    return colorize(Image.blend(gy, gx, x_weight), c0, c1)


def squircle_mask(size=S, radius_ratio=0.2237):
    n = size * SS
    mask = Image.new("L", (n, n), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [0, 0, n - 1, n - 1], radius=int(radius_ratio * n), fill=255
    )
    return mask.resize((size, size), Image.Resampling.LANCZOS)


def scaled_bars(scale, size=S, center=None, bars=None):
    cx, cy = center or (S / 2, S / 2)
    k = scale * (size / S)
    out = []
    for x0, y0, x1, y1 in (bars or BARS):
        out.append(
            (
                cx * (size / S) + (x0 - S / 2) * k,
                cy * (size / S) + (y0 - S / 2) * k,
                cx * (size / S) + (x1 - S / 2) * k,
                cy * (size / S) + (y1 - S / 2) * k,
            )
        )
    return out


def draw_mark(size=S, scale=1.0, c0=(255, 255, 255), c1=(233, 238, 255),
              shadow=None, shadow_offset=12, shadow_blur=22, center=None, bars=None):
    """绘制「E」标记，返回 RGBA 图层（尺寸 = size）。"""
    marks = scaled_bars(scale, size, center, bars)
    radius = RADIUS * scale * (size / S)

    high = Image.new("L", (size * SS, size * SS), 0)
    d = ImageDraw.Draw(high)
    for x0, y0, x1, y1 in marks:
        d.rounded_rectangle(
            [x0 * SS, y0 * SS, x1 * SS, y1 * SS], radius=radius * SS, fill=255
        )
    alpha = high.resize((size, size), Image.Resampling.LANCZOS)

    colored = vertical_gradient((size, size), c0, c1).convert("RGBA")
    colored.putalpha(alpha)

    if shadow is None:
        return colored

    layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    sh = Image.new("L", (size, size), 0)
    sh.paste(alpha, (0, shadow_offset))
    sh = sh.filter(ImageFilter.GaussianBlur(shadow_blur))
    sh = sh.point(lambda v: int(v * shadow / 255))
    shadow_layer = Image.new("RGBA", (size, size), (6, 14, 50, 255))
    shadow_layer.putalpha(sh)
    layer = Image.alpha_composite(layer, shadow_layer)
    return Image.alpha_composite(layer, colored)


def sheen(size=S, top_alpha=64, fade=0.45):
    """左上到中部的白色微高光，制造轻微立体感。"""
    t = Image.linear_gradient("L").resize((size, size), Image.Resampling.BILINEAR)
    alpha = t.point([max(0, int(top_alpha * (1 - (v / 255) / fade))) for v in range(256)])
    layer = Image.new("RGBA", (size, size), (255, 255, 255, 255))
    layer.putalpha(alpha)
    return layer


def radial_glow(size, center, radius, strength):
    small = 256
    gl = Image.new("L", (small, small), 0)
    cx, cy = center[0] / size * small, center[1] / size * small
    r = radius / size * small
    ImageDraw.Draw(gl).ellipse([cx - r, cy - r, cx + r, cy + r], fill=strength)
    gl = gl.filter(ImageFilter.GaussianBlur(small * 0.18))
    return gl.resize((size, size), Image.Resampling.BILINEAR)


def draw_word_lockup(e_scale=0.90, font_px=205, gap=18,
                     c0=(255, 255, 255), c1=(232, 238, 255), c_asy=None,
                     shadow=None, shadow_offset=14, shadow_blur=26):
    """主方案：大「E」居左，小字「asy」接在下横杠末端。

    asy 的字母主体（x-height 区）与下横杠**等高居中对齐**，故视觉上两者
    连成同一条水平带；"y" 的降部自然垂到 E 底边之下，整体按合并后的
    包围盒居中，避免重心歪斜。
    """
    size = S
    font = load_font(font_px)
    bb = font.getbbox("asy", anchor="ls")   # 以基线左端为原点
    asy_w = bb[2] - bb[0]
    xh = -bb[1]                             # x-height（a/s/y 均无升部）
    desc = max(0.0, bb[3])                  # y 的降部
    bar_h = BAR_H * e_scale
    drop = max(0.0, xh / 2 - bar_h / 2 + desc)
    e_w = (ARM_LONG[1] - STEM_X[0]) * e_scale
    e_h = (Y3 - Y0) * e_scale
    ox = (size - (e_w + gap + asy_w)) / 2
    oy = (size - (e_h + drop)) / 2

    layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    layer = Image.alpha_composite(layer, draw_mark(
        size, e_scale, c0, c1, center=(ox + e_w / 2, oy + e_h / 2),
        shadow=shadow, shadow_offset=shadow_offset, shadow_blur=shadow_blur))
    baseline = oy + e_h - bar_h / 2 + xh / 2
    txt = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    ImageDraw.Draw(txt).text((ox + e_w + gap - bb[0], baseline), "asy",
                             font=font, anchor="ls", fill=(c_asy or c0) + (255,))
    return Image.alpha_composite(layer, txt)


def draw_word_nested(font_px=126, c0=(255, 255, 255), c1=(232, 238, 255), c_asy=None,
                     shadow=None, shadow_offset=14, shadow_blur=26):
    """方案 B：满幅大「E」，小字「asy」嵌进中横与下横之间的开口。"""
    size = S
    font = load_font(font_px)
    bb = font.getbbox("asy")
    asy_w, asy_h = bb[2] - bb[0], bb[3] - bb[1]
    layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    layer = Image.alpha_composite(layer, draw_mark(
        size, 1.0, c0, c1, bars=BARS_NESTED,
        shadow=shadow, shadow_offset=shadow_offset, shadow_blur=shadow_blur))
    top, bottom = Y1 + BAR_H, Y2
    baseline = top + (bottom - top - asy_h) / 2 - bb[1]
    ox = STEM_X[1] + (ARM_LONG[1] - STEM_X[1] - asy_w) / 2
    txt = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    ImageDraw.Draw(txt).text((ox - bb[0], baseline), "asy",
                             font=font, fill=(c_asy or c0) + (255,))
    return Image.alpha_composite(layer, txt)


def compose(bg_theme, mark_scale=1.0, mark_theme="light", glow=False, sheen_on=True,
            shadow=None, mark_layer=None):
    size = S
    if bg_theme == "brand":
        bg = diagonal_gradient((size, size), BRAND_LIGHT, BRAND_DEEP)
    elif bg_theme == "dark":
        bg = diagonal_gradient((size, size), DARK_BG_1, DARK_BG_2, x_weight=0.35)
    elif bg_theme == "flat":
        bg = diagonal_gradient((size, size), BRAND_LIGHT, BRAND_DEEP)
    else:
        raise ValueError(bg_theme)

    canvas = bg.convert("RGBA")

    if glow:
        g = Image.new("RGBA", (size, size), BRAND_PRIMARY + (255,))
        g.putalpha(radial_glow(size, (size / 2, size / 2), size * 0.40, 92))
        canvas = Image.alpha_composite(canvas, g)

    if sheen_on:
        canvas = Image.alpha_composite(canvas, sheen(size))

    if mark_layer is not None:
        mark = mark_layer
    elif mark_theme == "light":
        mark = draw_mark(size, mark_scale, (255, 255, 255), (232, 238, 255), shadow)
    else:
        mark = draw_mark(size, mark_scale, (176, 197, 255), BRAND_LIGHT, shadow)
    canvas = Image.alpha_composite(canvas, mark)

    canvas.putalpha(squircle_mask(size))
    return canvas


def downsize(img, size):
    return img.resize((size, size), Image.Resampling.LANCZOS)


def save(img, path, size=None):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    out = downsize(img, size) if size else img
    out.save(path)
    print("wrote", os.path.relpath(path, BASE), out.size)


def save_ico(candidates, path):
    """从 256 主图导出多尺寸 ICO。"""
    master = candidates[256]
    os.makedirs(os.path.dirname(path), exist_ok=True)
    master.save(
        path,
        format="ICO",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    print("wrote", os.path.relpath(path, BASE))


def save_icns(candidates, path):
    """手工打包 ICNS（现代 PNG 类型）。"""
    types = {
        "icp4": 16, "icp5": 32, "icp6": 64, "ic07": 128, "ic08": 256,
        "ic09": 512, "ic10": 1024, "ic11": 32, "ic12": 64, "ic13": 256, "ic14": 512,
    }
    body = b""
    for code, size in types.items():
        buf = io.BytesIO()
        candidates[size].save(buf, format="PNG")
        data = buf.getvalue()
        body += code.encode("ascii") + struct.pack(">I", len(data) + 8) + data
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as fh:
        fh.write(b"icns" + struct.pack(">I", len(body) + 8) + body)
    print("wrote", os.path.relpath(path, BASE), len(body) + 8, "bytes")


def build_preview(primary, nested, dark, flat, mark_white):
    """生成一张方案对比图，便于评审。"""
    from PIL import ImageFont

    W, H = 1600, 940
    sheet = Image.new("RGBA", (W, H), (20, 22, 28, 255))

    def font(size):
        for path in (r"C:\Windows\Fonts\msyh.ttc", r"C:\Windows\Fonts\segoeui.ttf"):
            try:
                return ImageFont.truetype(path, size)
            except OSError:
                continue
        return ImageFont.load_default()

    title = font(38)
    label = font(24)

    d = ImageDraw.Draw(sheet)
    d.text((80, 48), "轻松面试 Easy Interview · Logo 方案", font=title, fill=(240, 243, 255))
    d.text((80, 100), "色板：#5B7CFA → #2B46C4（品牌主色 #4F6BED）｜圆角 22.4%｜大 E + 小 asy",
           font=label, fill=(150, 158, 180))

    items = [
        ("A 大 E + 小 asy（推荐）", primary),
        ("B E 内含小 asy", nested),
        ("C 深色主题", dark),
        ("D 扁平无投影", flat),
    ]
    big = 256
    gap = 56
    total = len(items) * big + (len(items) - 1) * gap
    x = (W - total) // 2
    for text, img in items:
        icon = img.resize((big, big), Image.Resampling.LANCZOS)
        sheet.alpha_composite(icon, (x, 170))
        tw = d.textlength(text, font=label)
        d.text((x + (big - tw) / 2, 170 + big + 18), text, font=label,
               fill=(206, 214, 236))
        x += big + gap

    d.text((80, 560), "尺寸阶梯（主方案 128 / 64 / 48 / 32 / 16）", font=label,
           fill=(150, 158, 180))
    x = 80
    for size in (128, 64, 48, 32, 16):
        icon = primary.resize((size, size), Image.Resampling.LANCZOS)
        sheet.alpha_composite(icon, (x, 700 - size // 2))
        x += size + 40

    d.text((900, 560), "透明底白色标记（托盘 / 单色场景）", font=label,
           fill=(150, 158, 180))
    strip = Image.new("RGBA", (300, 200), (43, 70, 196, 255))
    white_mark = mark_white.resize((200, 200), Image.Resampling.LANCZOS)
    strip.alpha_composite(white_mark, (50, 0))
    sheet.alpha_composite(strip, (900, 620))

    save(sheet.convert("RGB"), os.path.join(FINAL, "preview.png"))


def main():
    os.makedirs(FINAL, exist_ok=True)
    os.makedirs(TAURI_ICONS, exist_ok=True)

    # 1) 主方案 A：大 E 居左 + 右侧小字 asy（垂直居中）
    primary = compose("brand", mark_layer=draw_word_lockup(shadow=50))
    # 2) 方案 B：满幅大 E，小字 asy 嵌进中横与下横之间的开口
    nested = compose("brand", mark_layer=draw_word_nested(shadow=50))
    # 3) 深色主题版本
    dark = compose("dark", glow=True, mark_layer=draw_word_lockup(
        c0=(176, 197, 255), c1=BRAND_LIGHT, c_asy=(168, 190, 255), shadow=40))
    # 4) 扁平版：无投影无高光
    flat = compose("brand", sheen_on=False,
                   mark_layer=draw_word_lockup(shadow=None))
    # 5) 小尺寸优化版：只保留大 E（asy 缩小后会糊），标记放大
    small_src = compose("brand", mark_scale=1.22, mark_theme="light",
                        sheen_on=False, shadow=None)

    for name, img in (("logo-primary-1024.png", primary),
                      ("logo-nested-1024.png", nested),
                      ("logo-dark-1024.png", dark),
                      ("logo-flat-1024.png", flat)):
        save(img, os.path.join(FINAL, name))
    for size in (512, 256, 128, 64, 32):
        save(primary, os.path.join(FINAL, f"logo-primary-{size}.png"), size)

    # 5) 透明底白色标记（托盘 / 单色场景）
    mark = Image.new("RGBA", (512, 512), (0, 0, 0, 0))
    mark = Image.alpha_composite(mark, draw_mark(512, 1.34, (255, 255, 255), (255, 255, 255)))
    save(mark, os.path.join(FINAL, "mark-white-512.png"))

    # 6) Tauri 图标集（尺寸与现有 src-tauri/icons 完全一致）
    keyed = {s: downsize(primary, s) for s in (16, 24, 32, 48, 64, 128, 256, 512, 1024)}
    small_keyed = {s: downsize(small_src, s) for s in (16, 24, 32, 48, 64, 128, 256)}
    for s in (16, 24, 32, 48, 64):
        keyed[s] = small_keyed[s]

    for name, size in (("32x32.png", 32), ("64x64.png", 64), ("128x128.png", 128),
                       ("128x128@2x.png", 256), ("icon.png", 512),
                       ("Square30x30Logo.png", 30), ("Square44x44Logo.png", 44),
                       ("Square71x71Logo.png", 71), ("Square89x89Logo.png", 89),
                       ("Square107x107Logo.png", 107), ("Square142x142Logo.png", 142),
                       ("Square150x150Logo.png", 150), ("Square284x284Logo.png", 284),
                       ("Square310x310Logo.png", 310), ("StoreLogo.png", 50)):
        base = small_keyed if size <= 64 else keyed
        save(base.get(size) or downsize(primary, size),
             os.path.join(TAURI_ICONS, name))

    ico_master = Image.new("RGBA", (256, 256), (0, 0, 0, 0))
    ico_master = Image.alpha_composite(ico_master, small_keyed[256])
    save_ico({256: ico_master}, os.path.join(TAURI_ICONS, "icon.ico"))
    save_ico({256: ico_master}, os.path.join(FINAL, "icon.ico"))
    save_icns(keyed, os.path.join(TAURI_ICONS, "icon.icns"))

    # 7) 方案对比图
    build_preview(primary, nested, dark, flat, mark)


if __name__ == "__main__":
    main()
