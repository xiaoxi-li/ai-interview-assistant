"""把 final/user/mark-app-1024.png 导出为客户端图标集（src-tauri/icons）。"""

import io
import os
import struct

from PIL import Image

BASE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(BASE, "..", ".."))
SRC = os.path.join(BASE, "final", "user", "app-icon-rounded-1024.png")
OUT = os.path.join(REPO, "src-tauri", "icons")

NAMES = ((32, "32x32.png"), (64, "64x64.png"), (128, "128x128.png"),
         (256, "128x128@2x.png"), (512, "icon.png"), (30, "Square30x30Logo.png"),
         (44, "Square44x44Logo.png"), (71, "Square71x71Logo.png"),
         (89, "Square89x89Logo.png"), (107, "Square107x107Logo.png"),
         (142, "Square142x142Logo.png"), (150, "Square150x150Logo.png"),
         (284, "Square284x284Logo.png"), (310, "Square310x310Logo.png"),
         (50, "StoreLogo.png"))
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]
ICNS = [("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128), ("ic08", 256),
        ("ic09", 512), ("ic10", 1024), ("ic11", 32), ("ic12", 64),
        ("ic13", 256), ("ic14", 512)]


def main():
    src = Image.open(SRC).convert("RGBA")
    scale = lambda n: src.resize((n, n), Image.Resampling.LANCZOS)

    for size, name in NAMES:
        scale(size).save(os.path.join(OUT, name))
    scale(256).save(os.path.join(OUT, "icon.ico"), format="ICO",
                    sizes=[(n, n) for n in ICO_SIZES])

    body = b""
    for code, size in ICNS:
        buf = io.BytesIO()
        scale(size).save(buf, format="PNG")
        data = buf.getvalue()
        body += code.encode("ascii") + struct.pack(">I", len(data) + 8) + data
    with open(os.path.join(OUT, "icon.icns"), "wb") as fh:
        fh.write(b"icns" + struct.pack(">I", len(body) + 8) + body)

    print(f"exported {len(NAMES)} png + icon.ico + icon.icns -> src-tauri/icons")


if __name__ == "__main__":
    main()
