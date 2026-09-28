"""调用多模态图片生成服务生成图标候选方案，并把结果下载到本目录。

用法：
    set BUDDY_CLOUD_TOKEN=...
    py -3 gen.py
"""

import json
import os
import subprocess
import sys
import time
import urllib.request

BUDDY = (
    r"c:\Users\LQM\.vscode\extensions\tencent-cloud.coding-copilot-4.12.38765564"
    r"\out\extension\builtin\buddy-multimodal-generation\scripts\buddy-cloud.py"
)
OUT_DIR = os.path.dirname(os.path.abspath(__file__))

PROMPTS = [
    (
        "03-dark-e.png",
        "App icon design, one square app icon with rounded corners (squircle), flat vector, "
        "minimal. Background: deep dark navy gradient from #0F1420 to #171E2E. Centered: a "
        "glowing geometric capital letter E made of three thick horizontal rounded bars, "
        "gradient fill from #7C9AFF to #4F6BED, with a soft blue neon glow behind it. Dark "
        "theme desktop software icon, elegant, high contrast, no text, no watermark, crisp "
        "vector edges, Windows 11 and macOS style.",
    ),
    (
        "04-cards-e.png",
        "App icon design, one square app icon with rounded corners (squircle), flat vector, "
        "minimal. Background: soft light lavender white gradient #F5F6FA to #E4E9FF with a "
        "thin royal blue inner border glow. Centered: a bold geometric capital letter E built "
        "from three stacked rounded horizontal bars that read like three stacked note cards of "
        "a knowledge base, bars in brand blue gradient #4F6BED to #2B46C4, the bottom bar in "
        "lighter blue #7C9AFF. Clean, calm, professional productivity software icon, no text, "
        "no watermark, crisp vector edges, Windows 11 and macOS style.",
    ),
]


def generate(prompt: str) -> str | None:
    proc = subprocess.run(
        [sys.executable, BUDDY, "image", prompt],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if proc.returncode != 0:
        print("  submit failed:", (proc.stdout or "").strip()[:300])
        return None
    try:
        data = json.loads(proc.stdout)
    except json.JSONDecodeError:
        print("  bad json:", (proc.stdout or "").strip()[:300])
        return None
    urls = data.get("result_url")
    if isinstance(urls, list):
        return urls[0] if urls else None
    if isinstance(urls, str):
        return urls
    print("  no url:", json.dumps(data, ensure_ascii=False)[:300])
    return None


def main() -> None:
    if not os.environ.get("BUDDY_CLOUD_TOKEN"):
        print("BUDDY_CLOUD_TOKEN not set")
        sys.exit(1)

    for name, prompt in PROMPTS:
        target = os.path.join(OUT_DIR, name)
        if os.path.exists(target):
            print(name, "already exists, skip")
            continue
        url = None
        for attempt in range(1, 4):
            print(f"{name}: attempt {attempt}")
            url = generate(prompt)
            if url:
                break
            time.sleep(20)
        if not url:
            print(name, "FAILED")
            continue
        urllib.request.urlretrieve(url, target)
        print(name, "OK", os.path.getsize(target), "bytes")


if __name__ == "__main__":
    main()
