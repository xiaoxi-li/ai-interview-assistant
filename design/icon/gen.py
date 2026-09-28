"""调用云端图片生成服务，产出「简」字**艺术字体概念图**（篆书 / 花体等
系统字体没有的风格），下载到 `ai-concepts/` 供评审参考。

注意：这些是生成式概念图，带「AI 生成」水印、字形可能不规范，仅作风格灵感；
正式方案请用 `render.py`（系统字体精确渲染）。

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
OUT_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "ai-concepts")

PROMPTS = [
    (
        "seal-script.png",
        "App icon, single square icon with rounded corners (squircle), flat vector. "
        "Background: smooth diagonal gradient from #5B7CFA to #2B46C4. Centered: the "
        "Chinese character 简 (jian) rendered in ancient Chinese seal script (zhuan shu) "
        "style, uniform thin elegant strokes, symmetric square composition, white "
        "strokes on blue. Minimal, generous padding, no other text, no watermark, "
        "crisp vector edges, modern app icon.",
    ),
    (
        "floral-script.png",
        "App icon, single square icon with rounded corners (squircle), flat vector. "
        "Background: smooth diagonal gradient from #5B7CFA to #2B46C4. Centered: the "
        "Chinese character 简 in an ornate decorative floral calligraphy style, white "
        "strokes with elegant flourishes and thin swirl endings, graceful, artistic. "
        "Minimal, generous padding, no other text, no watermark, crisp vector edges, "
        "modern app icon.",
    ),
]


def generate(prompt: str) -> str | None:
    proc = subprocess.run(
        [sys.executable, BUDDY, "image", prompt],
        capture_output=True, text=True, encoding="utf-8",
    )
    if proc.returncode != 0:
        print("  submit failed:", (proc.stdout or "").strip()[:200])
        return None
    try:
        data = json.loads(proc.stdout)
    except json.JSONDecodeError:
        print("  bad json:", (proc.stdout or "").strip()[:200])
        return None
    urls = data.get("result_url")
    if isinstance(urls, list):
        return urls[0] if urls else None
    if isinstance(urls, str):
        return urls
    print("  no url:", json.dumps(data, ensure_ascii=False)[:200])
    return None


def main() -> None:
    if not os.environ.get("BUDDY_CLOUD_TOKEN"):
        print("BUDDY_CLOUD_TOKEN not set")
        sys.exit(1)
    os.makedirs(OUT_DIR, exist_ok=True)

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
