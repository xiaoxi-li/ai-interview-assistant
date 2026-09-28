"""扫描系统字体，找出真正含有目标汉字（默认「简」）的字体。

用于挑选可渲染的中文艺术字体（隶书 / 行楷 / 新魏 / 琥珀 / 彩云 / 姚体 / 舒体 …）。

用法：py -3 scan_fonts.py [字符]
"""

import glob
import os
import sys

from PIL import Image, ImageDraw, ImageFont

CHAR = sys.argv[1] if len(sys.argv) > 1 else "简"
NODEF = "\ue05f"  # 私有区码位：字体通常无此字形，用于比对 .notdef
FONT_DIR = r"C:\Windows\Fonts"


def ink_ratio(font, ch, size=64):
    """渲染字符，返回墨迹像素占比；同时返回是否与 .notdef 相同。"""
    img = Image.new("L", (size, size), 0)
    ImageDraw.Draw(img).text((size / 2, size / 2), ch, font=font, fill=255,
                             anchor="mm")
    px = sum(1 for p in img.getdata() if p > 8)
    return px / (size * size)


def main():
    hits = []
    for path in sorted(glob.glob(os.path.join(FONT_DIR, "*.*"))):
        if os.path.splitext(path)[1].lower() not in (".ttf", ".ttc", ".otf"):
            continue
        for index in (0, 1, 2):
            try:
                font = ImageFont.truetype(path, 64, index=index)
            except OSError:
                if index == 0:
                    break
                continue
            except Exception:
                break
            try:
                r_char = ink_ratio(font, CHAR)
                r_nodef = ink_ratio(font, NODEF)
            except Exception:
                break
            if r_char <= 0:
                break
            if abs(r_char - r_nodef) < 1e-9:  # 与缺字形完全一致 → 无此字
                break
            name = font.getname()
            hits.append((os.path.basename(path), name[0], name[1], r_char))
            break

    hits.sort(key=lambda h: -h[3])
    print(f"含「{CHAR}」的字体 {len(hits)} 个：")
    for f, family, style, r in hits:
        print(f"  {r * 100:5.2f}%  {f:<20} {family} / {style}")


if __name__ == "__main__":
    main()
