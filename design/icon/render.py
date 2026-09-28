"""轻松面试 · Logo 渲染器 —— 汉字「简」艺术字体方案。

设计意图：「简」= 简单 / 简约 / 化繁为简，正对客户端「本地优先、低资源、
低复杂度、让面试变轻松」的定位。本脚本用系统自带的中文字体渲染多套风格
方案，供评审挑选；选定后再导出与 `src-tauri/icons` 同名的完整图标集。

用法：
    py -3 render.py                 # 渲染全部方案 + 对比图
    py -3 render.py --apply hupo    # 把某方案导出为完整图标集
"""

import io
import os
import struct
import sys

from PIL import Image, ImageDraw, ImageFilter, ImageFont

BASE = os.path.dirname(os.path.abspath(__file__))
FINAL = os.path.join(BASE, "final")
HANZI_DIR = os.path.join(FINAL, "hanzi")
TAURI_ICONS = os.path.join(FINAL, "tauri-icons")
LABEL_FONT = r"C:\Windows\Fonts\msyh.ttc"

S = 1024          # 主尺寸
SS = 4            # 方圆形遮罩的超采样倍数
HANZI = "简"      # 主体汉字
GLYPH_RATIO = 0.62  # 字面占画布比例

# ---- 品牌令牌（与 src/index.css / tailwind.config.js 一致） ----
BRAND_PRIMARY = (79, 107, 237)    # #4F6BED
BRAND_LIGHT = (91, 124, 250)      # #5B7CFA
BRAND_DEEP = (43, 70, 196)        # #2B46C4
BRAND_PALE = (124, 154, 255)      # #7C9AFF
DARK_BG_1 = (27, 36, 64)
DARK_BG_2 = (15, 20, 32)

# ---- 候选字体（均为系统自带，已确认含「简」字） ----
# (key, 名称, 字体文件, 风格说明, 字重补偿)
# 字重补偿 = 在 1600px 渲染尺度上给笔画外扩的像素：书法体笔画偏细，图标尺寸下会
# 失重、被背景吃掉，统一补偿后各方案的视觉重量才可比。
STYLES = [
    ("hupo", "华文琥珀", r"C:\Windows\Fonts\STHUPO.TTF", "圆润厚重 · 装饰感最强", 0),
    ("liti", "华文隶书", r"C:\Windows\Fonts\STLITI.TTF", "古朴典雅 · 蚕头燕尾", 16),
    ("xingkai", "华文行楷", r"C:\Windows\Fonts\STXINGKA.TTF", "行云流水 · 手写气韵", 13),
    ("xinwei", "华文新魏", r"C:\Windows\Fonts\STXINWEI.TTF", "魏碑刀刻 · 方正有力", 15),
    ("yaoti", "方正姚体", r"C:\Windows\Fonts\FZYTK.TTF", "美术字 · 横细竖粗", 10),
    ("shuti", "方正舒体", r"C:\Windows\Fonts\FZSTK.TTF", "舒同体 · 飘逸洒脱", 17),
    ("kaiti", "华文楷体", r"C:\Windows\Fonts\STKAITI.TTF", "端正工整 · 传统书风", 15),
    ("zhongsong", "华文中宋", r"C:\Windows\Fonts\STZHONGS.TTF", "宋体骨架 · 典雅稳重", 13),
    ("caiyun", "华文彩云", r"C:\Windows\Fonts\STCAIYUN.TTF", "空心装饰 · 轻盈跳脱", 10),
]

# 把自备字体（篆书 / 花体等系统没有的书法字体）丢进该目录即会自动成为候选方案
EXTRA_DIR = os.path.join(BASE, "fonts")


def all_styles():
    styles = list(STYLES)
    if os.path.isdir(EXTRA_DIR):
        for name in sorted(os.listdir(EXTRA_DIR)):
            if os.path.splitext(name)[1].lower() in (".ttf", ".otf", ".ttc"):
                key = os.path.splitext(name)[0].lower()
                styles.append((key, key, os.path.join(EXTRA_DIR, name), "自备字体", 12))
    return styles


# --------------------------------------------------------------------------
# 基础绘制
# --------------------------------------------------------------------------

def lerp(c0, c1, t):
    return tuple(int(round(c0[i] + (c1[i] - c0[i]) * t)) for i in range(3))


def colorize(t, c0, c1):
    chans = [t.point([lerp(c0, c1, v / 255)[i] for v in range(256)]) for i in range(3)]
    return Image.merge("RGB", chans)


def vertical_gradient(size, c0, c1):
    t = Image.linear_gradient("L").resize(size, Image.Resampling.BILINEAR)
    return colorize(t, c0, c1)


def diagonal_gradient(size, c0, c1, x_weight=0.42):
    gy = Image.linear_gradient("L").resize(size, Image.Resampling.BILINEAR)
    gx = Image.linear_gradient("L").rotate(90, expand=True).resize(
        size, Image.Resampling.BILINEAR)
    return colorize(Image.blend(gy, gx, x_weight), c0, c1)


def squircle_mask(size=S, radius_ratio=0.2237):
    n = size * SS
    mask = Image.new("L", (n, n), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [0, 0, n - 1, n - 1], radius=int(radius_ratio * n), fill=255)
    return mask.resize((size, size), Image.Resampling.LANCZOS)


def sheen(size=S, top_alpha=64, fade=0.45):
    t = Image.linear_gradient("L").resize((size, size), Image.Resampling.BILINEAR)
    alpha = t.point([max(0, int(top_alpha * (1 - (v / 255) / fade))) for v in range(256)])
    layer = Image.new("RGBA", (size, size), (255, 255, 255, 255))
    layer.putalpha(alpha)
    return layer


def radial_glow(size, radius, strength):
    small = 256
    gl = Image.new("L", (small, small), 0)
    c = small / 2
    r = radius / size * small
    ImageDraw.Draw(gl).ellipse([c - r, c - r, c + r, c + r], fill=strength)
    gl = gl.filter(ImageFilter.GaussianBlur(small * 0.18))
    return gl.resize((size, size), Image.Resampling.BILINEAR)


# --------------------------------------------------------------------------
# 汉字字形
# --------------------------------------------------------------------------

def render_glyph(font_path, box=S, ratio=GLYPH_RATIO, stroke=0,
                 c0=(255, 255, 255), c1=(232, 238, 255)):
    """渲染「简」并按墨迹框归一化，返回居中于 box×box 的 RGBA 图层。

    不同字体的字面大小差异很大，统一按墨迹外接框缩放，保证各方案的
    视觉体量一致，只有风格不同；`stroke` 用于给细笔画字体补字重。
    """
    big = 1600
    font = ImageFont.truetype(font_path, big)
    probe = Image.new("L", (big * 2, big * 2), 0)
    ImageDraw.Draw(probe).text((big, big), HANZI, font=font, fill=255,
                               anchor="mm", stroke_width=stroke, stroke_fill=255)
    bbox = probe.getbbox()
    if not bbox:
        raise RuntimeError(f"字体未渲染出字形：{font_path}")
    crop = probe.crop(bbox)
    w, h = crop.size
    scale = box * ratio / max(w, h)
    size = (max(1, int(round(w * scale))), max(1, int(round(h * scale))))
    alpha = crop.resize(size, Image.Resampling.LANCZOS)

    grad = vertical_gradient(size, c0, c1).convert("RGBA")
    grad.putalpha(alpha)
    layer = Image.new("RGBA", (box, box), (0, 0, 0, 0))
    layer.alpha_composite(grad, ((box - size[0]) // 2, (box - size[1]) // 2))
    return layer


def with_shadow(glyph, strength=55, offset=12, blur=26):
    alpha = glyph.getchannel("A")
    sh = Image.new("L", glyph.size, 0)
    sh.paste(alpha, (0, offset))
    sh = sh.filter(ImageFilter.GaussianBlur(blur))
    sh = sh.point(lambda v: int(v * strength / 255))
    out = Image.new("RGBA", glyph.size, (6, 14, 50, 255))
    out.putalpha(sh)
    out = Image.alpha_composite(out, glyph)
    return out


# --------------------------------------------------------------------------
# 合成
# --------------------------------------------------------------------------

def compose(glyph, theme="brand", glow=False, sheen_on=True, shadow=55):
    if theme == "brand":
        bg = diagonal_gradient((S, S), BRAND_LIGHT, BRAND_DEEP)
    elif theme == "dark":
        bg = diagonal_gradient((S, S), DARK_BG_1, DARK_BG_2, x_weight=0.35)
    else:
        raise ValueError(theme)

    canvas = bg.convert("RGBA")
    if glow:
        g = Image.new("RGBA", (S, S), BRAND_PRIMARY + (255,))
        g.putalpha(radial_glow(S, S * 0.42, 96))
        canvas = Image.alpha_composite(canvas, g)
    if sheen_on:
        canvas = Image.alpha_composite(canvas, sheen(S))

    mark = with_shadow(glyph, shadow) if shadow else glyph
    canvas = Image.alpha_composite(canvas, mark)
    canvas.putalpha(squircle_mask(S))
    return canvas


# --------------------------------------------------------------------------
# 输出
# --------------------------------------------------------------------------

def downsize(img, size):
    return img.resize((size, size), Image.Resampling.LANCZOS)


def save(img, path, size=None):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    out = downsize(img, size) if size else img
    out.save(path)
    print("wrote", os.path.relpath(path, BASE), out.size)


def save_ico(master, path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    master.save(path, format="ICO",
                sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64),
                       (128, 128), (256, 256)])


def save_icns(candidates, path):
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
    print("wrote", os.path.relpath(path, BASE))


def build_preview(items):
    cols, cell_w, cell_h, margin = 3, 300, 366, 60
    rows = (len(items) + cols - 1) // cols
    W = margin * 2 + cols * cell_w
    H = 150 + rows * cell_h + margin
    sheet = Image.new("RGBA", (W, H), (20, 22, 28, 255))

    def font(px):
        try:
            return ImageFont.truetype(LABEL_FONT, px)
        except OSError:
            return ImageFont.load_default()

    title, name_f, note_f = font(38), font(28), font(22)
    d = ImageDraw.Draw(sheet)
    d.text((margin, 48), "轻松面试 · 汉字「简」Logo 方案", font=title, fill=(240, 243, 255))
    d.text((margin, 100), "色板 #5B7CFA → #2B46C4（主色 #4F6BED）｜方圆形圆角 22.4%｜字面占 62%",
           font=note_f, fill=(150, 158, 180))

    icon = 240
    for i, (key, label, note, img) in enumerate(items):
        cx = margin + (i % cols) * cell_w
        cy = 150 + (i // cols) * cell_h
        sheet.alpha_composite(downsize(img, icon), (cx + (cell_w - icon) // 2, cy))
        for text, f, y, color in (
            (label, name_f, cy + icon + 18, (230, 235, 250)),
            (note, note_f, cy + icon + 56, (140, 148, 172)),
            (f"[{key}]", note_f, cy + icon + 84, (100, 110, 140)),
        ):
            tw = d.textlength(text, font=f)
            d.text((cx + (cell_w - tw) / 2, y), text, font=f, fill=color)

    save(sheet.convert("RGB"), os.path.join(FINAL, "preview.png"))


def export_icon_set(img):
    """导出与 src-tauri/icons 同名的完整图标集。"""
    keyed = {s: downsize(img, s) for s in (16, 24, 32, 48, 64, 128, 256, 512, 1024)}
    for name, size in (("32x32.png", 32), ("64x64.png", 64), ("128x128.png", 128),
                       ("128x128@2x.png", 256), ("icon.png", 512),
                       ("Square30x30Logo.png", 30), ("Square44x44Logo.png", 44),
                       ("Square71x71Logo.png", 71), ("Square89x89Logo.png", 89),
                       ("Square107x107Logo.png", 107), ("Square142x142Logo.png", 142),
                       ("Square150x150Logo.png", 150), ("Square284x284Logo.png", 284),
                       ("Square310x310Logo.png", 310), ("StoreLogo.png", 50)):
        save(keyed.get(size) or downsize(img, size),
             os.path.join(TAURI_ICONS, name))
    save_ico(keyed[256], os.path.join(TAURI_ICONS, "icon.ico"))
    save_icns(keyed, os.path.join(TAURI_ICONS, "icon.icns"))


def main():
    if "--apply" in sys.argv:
        key = sys.argv[sys.argv.index("--apply") + 1]
        match = [s for s in all_styles() if s[0] == key]
        if not match:
            print("unknown style:", key)
            sys.exit(1)
        _, label, font_path, _, stroke = match[0]
        img = compose(render_glyph(font_path, stroke=stroke))
        export_icon_set(img)
        save(img, os.path.join(FINAL, "logo-selected.png"))
        print("applied:", key, label)
        return

    items = []
    for key, label, font_path, note, stroke in all_styles():
        glyph = render_glyph(font_path, stroke=stroke)
        img = compose(glyph)
        save(img, os.path.join(HANZI_DIR, f"{key}-1024.png"))
        save(glyph, os.path.join(HANZI_DIR, f"{key}-mark.png"))
        items.append((key, label, note, img))

    build_preview(items)
    print("\n挑选后执行：py -3 render.py --apply <key>")


if __name__ == "__main__":
    main()
