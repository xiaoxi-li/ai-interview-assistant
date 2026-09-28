"""把 final/user/app-icon-brand-1024.png 导出为客户端图标集（src-tauri/icons）。

ICO 手工组装：每个尺寸都从 1024 源**直接缩放**，避免「先缩到 256 再缩到 16」的二次损失。
"""

import io
import os
import struct

from PIL import Image

BASE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(BASE, "..", ".."))
SRC = os.path.join(BASE, "final", "user", "app-icon-taskbar-1024.png")
SMALL_SRC = SRC          # 标记本已占 88%，小尺寸无需另一版
OUT = os.path.join(REPO, "src-tauri", "icons")
SMALL_UPTO = 32          # 该尺寸及以下改用「标记更大」的专用版，避免小尺寸糊成一团

NAMES = ((32, "32x32.png"), (64, "64x64.png"), (128, "128x128.png"),
         (256, "128x128@2x.png"), (512, "icon.png"), (30, "Square30x30Logo.png"),
         (44, "Square44x44Logo.png"), (71, "Square71x71Logo.png"),
         (89, "Square89x89Logo.png"), (107, "Square107x107Logo.png"),
         (142, "Square142x142Logo.png"), (150, "Square150x150Logo.png"),
         (284, "Square284x284Logo.png"), (310, "Square310x310Logo.png"),
         (50, "StoreLogo.png"))
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
ICNS = [("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128), ("ic08", 256),
        ("ic09", 512), ("ic10", 1024), ("ic11", 32), ("ic12", 64),
        ("ic13", 256), ("ic14", 512)]


def png_bytes(img):
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return buf.getvalue()


def write_ico(pick, path, sizes):
    blobs = [(n, png_bytes(pick(n).resize((n, n), Image.Resampling.LANCZOS)))
             for n in sizes]
    header = struct.pack("<HHH", 0, 1, len(blobs))
    offset = 6 + 16 * len(blobs)
    entries, data = b"", b""
    for size, blob in blobs:
        dim = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
        data += blob
    with open(path, "wb") as fh:
        fh.write(header + entries + data)


def main():
    src = Image.open(SRC).convert("RGBA")
    small = Image.open(SMALL_SRC).convert("RGBA")

    def pick(n):
        return small if n <= SMALL_UPTO else src

    for size, name in NAMES:
        pick(size).resize((size, size), Image.Resampling.LANCZOS).save(
            os.path.join(OUT, name))
    write_ico(pick, os.path.join(OUT, "icon.ico"), ICO_SIZES)

    body = b""
    for code, size in ICNS:
        blob = png_bytes(src.resize((size, size), Image.Resampling.LANCZOS))
        body += code.encode("ascii") + struct.pack(">I", len(blob) + 8) + blob
    with open(os.path.join(OUT, "icon.icns"), "wb") as fh:
        fh.write(b"icns" + struct.pack(">I", len(body) + 8) + body)

    print(f"exported {len(NAMES)} png + icon.ico({len(ICO_SIZES)} sizes) + icon.icns")


if __name__ == "__main__":
    main()
