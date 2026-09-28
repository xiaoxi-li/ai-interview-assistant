"""把复刻出的应用图标铺到客户端图标位（src-tauri/icons + src/assets）。"""

import io
import os
import struct

from PIL import Image

BASE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(BASE, "..", ".."))
SRC = os.path.join(BASE, "final", "rebuilt")
ICONS = os.path.join(REPO, "src-tauri