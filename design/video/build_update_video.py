#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""生成「Easy AI 面试助手 · 本次更新」介绍视频（1920x1080 / 30fps / 人声旁白 + 字幕 + 原创音乐）。

用法：
    python build_update_video.py --preview 3,12,20,34,45,60   # 只导出关键帧 PNG
    python build_update_video.py --render                     # 渲染整片
    python build_update_video.py --audio-only                 # 只合成旁白与音乐

旁白通过 Windows 自带中文语音（System.Speech，Microsoft Huihui）离线合成，
逐句生成后按时间轴铺轨；字幕直接使用同一份文案。音乐为 numpy 实时合成的
132 BPM 电子风格配乐，人声出现时自动做闪避（ducking）。

内容取自仓库 git 提交（2026-11-21 / 11-22 两批）：
    IA-11 隐身模式 + 窗口防捕获（重点）
    IA-14 进程级回环、采集设备重新检测与默认回退
    IA-16.9 云端 ASR 三家接入（讯飞 / 腾讯云 / OpenAI 兼容）
    KB-07 本地 ONNX 嵌入（BGE-M3）
    打包：本地 ASR + 本地嵌入合并配置
"""

import argparse
import math
import os
import subprocess
import wave

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

# ---------------------------------------------------------------- 基础参数
W, H = 1920, 1080
FPS = 30
TOTAL = 65.0
XF = 0.52  # 场景转场（品牌色光带扫过）时长

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
OUT_MP4 = os.path.join(HERE, "Easy更新介绍.mp4")
MIX_WAV = os.path.join(HERE, "_mix.wav")
TTS_DIR = os.path.join(HERE, "_tts")
PREVIEW_DIR = os.path.join(HERE, "_preview")
LOGO_PATH = os.path.join(ROOT, "design", "icon", "final", "user", "lockup-light.png")
FFMPEG = os.path.join(ROOT, "node_modules", "ffmpeg-static", "ffmpeg.exe")

FONT_B = "C:/Windows/Fonts/msyhbd.ttc"
FONT_R = "C:/Windows/Fonts/msyh.ttc"
FONT_L = "C:/Windows/Fonts/msyhl.ttc"

NAVY = (13, 27, 54)
BLUE = (31, 92, 240)
BLUE_MID = (60, 133, 255)
CYAN = (44, 197, 232)
GRAY = (102, 120, 148)
GRAY_L = (154, 170, 194)
WHITE = (255, 255, 255)

# ---------------------------------------------------------------- 旁白 / 字幕
# (最早开始时间, 文案)。实际起点会按上一句实际时长顺延，避免重叠。
NARRATION = [
    (0.65, "这次更新，我们只做一件大事：让面试窗口，彻底隐身。"),
    (7.15, "先看重点：会议中防录屏。"),
    (10.15, "开启隐身模式，你的桌面照常用。"),
    (13.75, "但共享出去的画面里，这个窗口已经完全消失。"),
    (18.95, "截图、录屏，对方都看不到。"),
    (22.15, "系统级防护，微软和苹果系统都支持。"),
    (30.45, "隐身之后，还有三个顺手的变化。"),
    (34.15, "迷你页贴边，托盘常驻，提问暂停都有快捷键。"),
    (40.15, "其他几处，也一起办了。"),
    (43.05, "音频只采指定应用。"),
    (45.75, "云端识别四家齐了。"),
    (48.45, "知识库本地语义检索。"),
    (51.15, "本地组件合并打包。"),
    (53.85, "设备重新检测，不用重启。"),
    (58.05, "更新已经就绪，打开应用更新到最新版，下一次面试，安心开完这一场。"),
]
SUBTITLES = []  # [(start, end, text)]，由 prepare_narration 填充

VOICE = "Microsoft Huihui"
TTS_RATE = 4


# ---------------------------------------------------------------- 工具函数
def clamp(x, a=0.0, b=1.0):
    return a if x < a else (b if x > b else x)


def seg(t, start, dur):
    if dur <= 0:
        return 1.0 if t >= start else 0.0
    return clamp((t - start) / dur)


def ease_out(t, p=3):
    t = clamp(t)
    return 1 - (1 - t) ** p


def ease_in_out(t):
    t = clamp(t)
    return t * t * (3 - 2 * t)


def lerp(a, b, t):
    return a + (b - a) * t


_font_cache = {}


def font(size, weight="b"):
    size = max(1, int(round(size)))
    key = (size, weight)
    if key not in _font_cache:
        path = {"b": FONT_B, "r": FONT_R, "l": FONT_L}[weight]
        _font_cache[key] = ImageFont.truetype(path, size, index=0)
    return _font_cache[key]


_MEASURE = ImageDraw.Draw(Image.new("L", (8, 8)))


def text_w(s, size, weight="b"):
    return _MEASURE.textlength(s, font=font(size, weight))


_SHADOW_CACHE = {}
_SPRITE_CACHE = {}


def shadow_sprite(w, h, r, blur=26, color=(24, 58, 132), alpha=52, dy=10):
    key = (int(w), int(h), int(r), blur, color, alpha, dy)
    if key in _SHADOW_CACHE:
        return _SHADOW_CACHE[key]
    pad = blur * 2
    s = Image.new("RGBA", (int(w) + pad * 2, int(h) + pad * 2), (0, 0, 0, 0))
    d = ImageDraw.Draw(s)
    d.rounded_rectangle([pad, pad + dy, pad + w, pad + h + dy], radius=r, fill=color + (alpha,))
    s = s.filter(ImageFilter.GaussianBlur(blur))
    _SHADOW_CACHE[key] = s
    return s


def paste(img, sprite, x, y, opacity=1.0, shadow=None):
    """把 RGBA sprite 贴到 RGB 画布。shadow = (阴影 sprite, dy)。"""
    if opacity <= 0.004:
        return
    if shadow is not None:
        sh, sdy = shadow
        pad = (sh.width - sprite.width) // 2
        m = sh.split()[3]
        if opacity < 0.999:
            m = m.point(lambda v: int(v * opacity))
        img.paste(sh, (int(x - pad), int(y - pad + sdy)), m)
    if opacity < 0.999:
        a = sprite.split()[3].point(lambda v: int(v * opacity))
        layer = sprite.copy()
        layer.putalpha(a)
        img.paste(layer, (int(x), int(y)), layer)
    else:
        img.paste(sprite, (int(x), int(y)), sprite)


def text_alpha(img, xy, s, size, fill, weight="b", opacity=1.0, pad=10):
    """带透明度的文字，避免直接改颜色导致的发灰。"""
    if opacity <= 0.01:
        return
    f = font(size, weight)
    bbox = _MEASURE.textbbox((0, 0), s, font=f)
    w, h = int(bbox[2] - bbox[0]), int(bbox[3] - bbox[1])
    layer = Image.new("RGBA", (w + pad * 2, h + pad * 2), (0, 0, 0, 0))
    ImageDraw.Draw(layer).text((pad - bbox[0], pad - bbox[1]), s, font=f, fill=fill + (255,))
    paste(img, layer, xy[0] - pad, xy[1] - pad, opacity)


def hgrad(size, c1, c2):
    w, h = size
    t = np.linspace(0.0, 1.0, max(2, w))[None, :, None]
    arr = np.array(c1, float)[None, None, :] * (1 - t) + np.array(c2, float)[None, None, :] * t
    arr = np.repeat(arr, h, axis=0)
    return Image.fromarray(arr.astype(np.uint8), "RGB")


def gradient_text(text, size, c1, c2, weight="b", pad=14):
    key = ("gtext", text, size, c1, c2, weight)
    if key in _SPRITE_CACHE:
        return _SPRITE_CACHE[key]
    f = font(size, weight)
    bbox = _MEASURE.textbbox((0, 0), text, font=f)
    w, h = int(bbox[2] - bbox[0]), int(bbox[3] - bbox[1])
    mask = Image.new("L", (w + pad * 2, h + pad * 2), 0)
    ImageDraw.Draw(mask).text((pad - bbox[0], pad - bbox[1]), text, font=f, fill=255)
    grad = hgrad(mask.size, c1, c2)
    out = Image.new("RGBA", mask.size, (0, 0, 0, 0))
    out.paste(grad, (0, 0), mask)
    _SPRITE_CACHE[key] = out
    return out


# ---------------------------------------------------------------- 绘制笔（支持超采样）
class Pen:
    def __init__(self, img, draw, k=1.0):
        self.img, self.d, self.k = img, draw, k

    def _b(self, box):
        k = self.k
        return [box[0] * k, box[1] * k, box[2] * k, box[3] * k]

    def _w(self, w):
        return max(1, int(round(w * self.k)))

    def _f(self, size, weight):
        return font(size * self.k, weight)

    def rect(self, box, r=0, fill=None, outline=None, width=1):
        x0, y0, x1, y1 = box
        if x1 < x0:
            x0, x1 = x1, x0
        if y1 < y0:
            y0, y1 = y1, y0
        b = self._b([x0, y0, x1, y1])
        # PIL 的圆角实现要求 radius*2 + 2 <= 短边，否则会抛错
        rmax = max(0.0, (min(b[2] - b[0], b[3] - b[1]) - 2) / 2)
        rad = min(r * self.k, rmax)
        self.d.rounded_rectangle(b, radius=rad, fill=fill,
                                 outline=outline, width=self._w(width))

    def ellipse(self, box, fill=None, outline=None, width=1):
        self.d.ellipse(self._b(box), fill=fill, outline=outline, width=self._w(width))

    def line(self, pts, fill=None, width=2, joint="curve"):
        self.d.line([(x * self.k, y * self.k) for x, y in pts], fill=fill,
                    width=self._w(width), joint=joint)

    def poly(self, pts, fill=None, outline=None):
        self.d.polygon([(x * self.k, y * self.k) for x, y in pts], fill=fill, outline=outline)

    def arc(self, box, start, end, fill, width=3):
        self.d.arc(self._b(box), start, end, fill=fill, width=self._w(width))

    def text(self, xy, s, size, fill, weight="b", anchor="la"):
        self.d.text((xy[0] * self.k, xy[1] * self.k), s, font=self._f(size, weight),
                    fill=fill, anchor=anchor)

    def tlen(self, s, size, weight="b"):
        return self.d.textlength(s, font=self._f(size, weight)) / self.k


def hi_res(w, h, fn, ss=2, downscale=True):
    big = Image.new("RGBA", (int(w * ss), int(h * ss)), (0, 0, 0, 0))
    p = Pen(big, ImageDraw.Draw(big), ss)
    fn(p)
    if not downscale:
        return big
    return big.resize((int(w), int(h)), Image.LANCZOS)


# ---------------------------------------------------------------- 背景
def build_background():
    top = (232, 243, 255)
    bot = (252, 253, 255)
    t = np.linspace(0.0, 1.0, H)[:, None, None]
    arr = np.array(top, float)[None, None, :] * (1 - t) + np.array(bot, float)[None, None, :] * t
    arr = np.repeat(arr, W, axis=1)
    bg = Image.fromarray(arr.astype(np.uint8), "RGB")

    glow = Image.new("RGB", (W, H), (255, 255, 255))
    d = ImageDraw.Draw(glow)
    d.ellipse([1180, -420, 2260, 620], fill=(196, 222, 255))
    d.ellipse([-360, 700, 620, 1500], fill=(206, 240, 252))
    glow = glow.filter(ImageFilter.GaussianBlur(150))
    bg = Image.blend(bg, glow, 0.55)

    grid = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    gd = ImageDraw.Draw(grid)
    for x in range(0, W, 80):
        gd.line([(x, 0), (x, H)], fill=(120, 160, 215, 5), width=1)
    for y in range(0, H, 80):
        gd.line([(0, y), (W, y)], fill=(120, 160, 215, 5), width=1)
    return Image.alpha_composite(bg.convert("RGBA"), grid).convert("RGB")


BG = None
BLOB_A = None
BLOB_B = None


def float_blob(size, color, alpha):
    pad = int(size * 0.45)
    s = Image.new("RGBA", (size + pad * 2, size + pad * 2), (0, 0, 0, 0))
    ImageDraw.Draw(s).ellipse([pad, pad, pad + size - 1, pad + size - 1], fill=color + (alpha,))
    return s.filter(ImageFilter.GaussianBlur(size // 4))


def draw_bg_motion(img, t):
    paste(img, BLOB_A, 1180 + 70 * math.sin(t * 0.22), -220 + 50 * math.cos(t * 0.18), 0.5)
    paste(img, BLOB_B, -220 + 60 * math.cos(t * 0.16 + 1.1), 600 + 60 * math.sin(t * 0.2 + 0.4), 0.45)


# ---------------------------------------------------------------- 品牌
_LOGO = None


def logo_sprite(height):
    global _LOGO
    if _LOGO is None:
        _LOGO = Image.open(LOGO_PATH).convert("RGBA")
    key = ("logo", height)
    if key not in _SPRITE_CACHE:
        w = int(round(_LOGO.width * height / _LOGO.height))
        _SPRITE_CACHE[key] = _LOGO.resize((w, int(height)), Image.LANCZOS)
    return _SPRITE_CACHE[key]


def brand_mark(height):
    key = ("mark", height)
    if key in _SPRITE_CACHE:
        return _SPRITE_CACHE[key]
    src = Image.open(LOGO_PATH).convert("RGBA")
    crop = src.crop((0, 0, int(src.width * 0.365), src.height))
    w = int(round(crop.width * height / crop.height))
    out = crop.resize((w, int(height)), Image.LANCZOS)
    _SPRITE_CACHE[key] = out
    return out


# ---------------------------------------------------------------- 图标
def draw_icon(p, name, cx, cy, s, color):
    h = s / 2.0
    if name == "mini":
        p.rect([cx - h, cy - h * 0.62, cx + h, cy + h * 0.62], 6, outline=color, width=3)
        p.rect([cx + h * 0.16, cy + h * 0.14, cx + h, cy + h * 0.62], 3, fill=color)
        p.line([(cx - h * 0.62, cy - h * 0.22), (cx + h * 0.0, cy - h * 0.22)], color, 3)
    elif name == "tray":
        p.line([(cx - h, cy - h * 0.12), (cx - h, cy + h * 0.52),
                (cx + h, cy + h * 0.52), (cx + h, cy - h * 0.12)], color, 3)
        p.line([(cx, cy - h * 0.78), (cx, cy + h * 0.12)], color, 3)
        p.line([(cx - h * 0.32, cy - h * 0.2), (cx, cy + h * 0.14),
                (cx + h * 0.32, cy - h * 0.2)], color, 3)
    elif name == "keyboard":
        p.rect([cx - h, cy - h * 0.6, cx + h, cy + h * 0.6], 6, outline=color, width=3)
        for i in (-0.5, 0, 0.5):
            p.ellipse([cx + h * i - 3, cy - h * 0.26 - 3, cx + h * i + 3, cy - h * 0.26 + 3], fill=color)
        p.line([(cx - h * 0.48, cy + h * 0.24), (cx + h * 0.48, cy + h * 0.24)], color, 3)
    elif name == "wave":
        for i, hh in enumerate((0.3, 0.62, 0.92, 0.5, 0.24)):
            x = cx - h * 0.72 + i * (h * 0.36)
            p.line([(x, cy - h * hh), (x, cy + h * hh)], color, 3)
    elif name == "cloud":
        p.arc([cx - h, cy - h * 0.5, cx + h * 0.1, cy + h * 0.8], 110, 360, color, 3)
        p.arc([cx - h * 0.55, cy - h, cx + h * 0.5, cy + h * 0.5], 170, 350, color, 3)
        p.arc([cx + h * 0.1, cy - h * 0.2, cx + h, cy + h * 0.8], 200, 350, color, 3)
        p.line([(cx - h * 0.7, cy + h * 0.72), (cx + h * 0.95, cy + h * 0.72)], color, 3)
    elif name == "search":
        p.ellipse([cx - h * 0.92, cy - h * 0.92, cx + h * 0.28, cy + h * 0.28], outline=color, width=3)
        p.line([(cx + h * 0.16, cy + h * 0.16), (cx + h * 0.9, cy + h * 0.9)], color, 3)
    elif name == "box":
        p.line([(cx, cy - h), (cx + h * 0.88, cy - h * 0.52), (cx + h * 0.88, cy + h * 0.46),
                (cx, cy + h), (cx - h * 0.88, cy + h * 0.46), (cx - h * 0.88, cy - h * 0.52),
                (cx, cy - h)], color, 3)
        p.line([(cx - h * 0.88, cy - h * 0.52), (cx, cy - h * 0.04), (cx + h * 0.88, cy - h * 0.52)],
               color, 3)
        p.line([(cx, cy - h * 0.04), (cx, cy + h)], color, 3)
    elif name == "refresh":
        p.arc([cx - h, cy - h, cx + h, cy + h], 40, 320, color, 3)
        p.line([(cx + h * 0.1, cy - h * 0.98), (cx + h * 0.78, cy - h * 0.52),
                (cx + h * 0.66, cy + h * 0.05)], color, 3)
    elif name == "shield":
        pts = [(cx, cy - h), (cx + h * 0.82, cy - h * 0.66), (cx + h * 0.82, cy + h * 0.06),
               (cx, cy + h), (cx - h * 0.82, cy + h * 0.06), (cx - h * 0.82, cy - h * 0.66)]
        p.poly(pts, fill=color)
        p.line([(cx - h * 0.34, cy - h * 0.04), (cx - h * 0.06, cy + h * 0.28),
                (cx + h * 0.4, cy - h * 0.4)], (255, 255, 255), 4)
    elif name == "check":
        p.line([(cx - h * 0.58, cy + h * 0.02), (cx - h * 0.12, cy + h * 0.48),
                (cx + h * 0.64, cy - h * 0.46)], color, 4)


def pill(label, size=24, padx=24, weight="r", fg=BLUE, bg=(232, 242, 255), border=(200, 222, 255)):
    tw = text_w(label, size, weight)
    w = int(tw + padx * 2)
    h = int(size * 1.7)
    out = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(out)
    d.rounded_rectangle([0, 0, w - 1, h - 1], radius=h // 2, fill=bg + (255,), outline=border + (255,), width=2)
    d.text((w / 2, h / 2 - 1), label, font=font(size, weight), fill=fg, anchor="mm")
    return out


def pill_cached(label, size=24, weight="r", fg=BLUE, bg=(232, 242, 255), border=(200, 222, 255)):
    key = ("pill", label, size, weight, fg, bg, border)
    if key not in _SPRITE_CACHE:
        _SPRITE_CACHE[key] = pill(label, size, weight=weight, fg=fg, bg=bg, border=border)
    return _SPRITE_CACHE[key]


def scan_band(w):
    """横向柔光带，用来说明「画面里扫不到东西」。"""
    key = ("scan", int(w))
    if key in _SPRITE_CACHE:
        return _SPRITE_CACHE[key]
    h = 110
    ys = np.arange(h)[:, None].astype(float)
    xs = np.linspace(0, 1, int(w))[None, :]
    a = np.exp(-((ys - h / 2) / (h / 4.6)) ** 2) * np.exp(-((xs - 0.5) / 0.45) ** 4)
    arr = np.zeros((h, int(w), 4), np.uint8)
    arr[..., 0], arr[..., 1], arr[..., 2] = 108, 164, 255
    arr[..., 3] = np.clip(a * 74, 0, 255).astype(np.uint8)
    img = Image.fromarray(arr, "RGBA")
    _SPRITE_CACHE[key] = img
    return img


def chip(label, size=23, icon=None, icon_color=BLUE, fg=NAVY, bg=(255, 255, 255),
         border=(200, 222, 255), pad_l=20, pad_r=24, gap=12):
    """胶囊标签（宽度按文字自动计算，可带图标）。"""
    key = ("chip", label, size, icon, icon_color, fg, bg, border, pad_l, pad_r, gap)
    if key in _SPRITE_CACHE:
        return _SPRITE_CACHE[key]
    tw = text_w(label, size, "b")
    h = int(size + 40)
    icon_s = size + 6
    w = int(pad_l + (icon_s + gap if icon else 0) + tw + pad_r)

    def build(p):
        p.rect([0, 0, w, h], h / 2, fill=bg, outline=border, width=2)
        xx = pad_l
        if icon:
            draw_icon(p, icon, xx + icon_s / 2, h / 2, icon_s, icon_color)
            xx += icon_s + gap
        p.text((xx, h / 2), label, size, fg, "b", "lm")

    _SPRITE_CACHE[key] = hi_res(w, h, build, ss=2)
    return _SPRITE_CACHE[key]


# ---------------------------------------------------------------- 模拟应用卡片
BRAND_MARK = None


def mock_app_card(p, x, y, w, h, hidden=False):
    if hidden:
        p.rect([x, y, x + w, y + h], 20, fill=(255, 255, 255), outline=(176, 202, 240), width=3)
        for i in range(0, int(w) - 20, 24):
            p.line([(x + 14 + i, y), (x + 30 + i, y)], (150, 184, 232), 3)
            p.line([(x + 14 + i, y + h), (x + 30 + i, y + h)], (150, 184, 232), 3)
        for i in range(0, int(h) - 20, 24):
            p.line([(x, y + 14 + i), (x, y + 30 + i)], (150, 184, 232), 3)
            p.line([(x + w, y + 14 + i), (x + w, y + 30 + i)], (150, 184, 232), 3)
        # 中间：盾牌 + 文案
        cx, cy = x + w / 2, y + h / 2
        p.ellipse([cx - 46, cy - 76, cx + 46, cy + 16], fill=(234, 243, 255))
        draw_icon(p, "shield", cx, cy - 34, 44, BLUE)
        p.text((cx, cy + 42), "0 个多余窗口", h * 0.13, NAVY, "b", "mm")
        p.text((cx, cy + 42 + h * 0.15), "共享 / 录屏里看不到", h * 0.085, GRAY, "r", "mm")
        return

    p.rect([x, y, x + w, y + h], 20, fill=(15, 22, 36), outline=(46, 62, 92), width=2)
    p.rect([x, y, x + w, y + h * 0.165], 20, fill=(21, 30, 48))
    p.rect([x, y + h * 0.165 - 16, x + w, y + h * 0.165], 0, fill=(21, 30, 48))
    # 标记 + 标题
    mk = brand_mark(h * 0.062)
    if p.k != 1:
        mk = mk.resize((max(1, int(mk.width * p.k)), max(1, int(mk.height * p.k))), Image.LANCZOS)
    p.img.paste(mk, (int((x + 14) * p.k), int((y + h * 0.055) * p.k)), mk)
    p.text((x + 16 + mk.width / p.k, y + h * 0.055), "Easy", h * 0.072, (228, 238, 250), "b")
    for i in range(3):
        p.ellipse([x + w - 22 - i * 11, y + h * 0.082, x + w - 18 - i * 11, y + h * 0.082 + 4],
                  fill=(68, 86, 116))
    # 问题
    qy = y + h * 0.235
    p.rect([x + 14, qy, x + w - 14, qy + h * 0.14], 8, fill=(26, 36, 56))
    p.text((x + 26, qy + h * 0.036), "面试官：介绍下你的项目", h * 0.062, (172, 190, 214), "r")
    # 回答
    ay = y + h * 0.41
    p.rect([x + 14, ay, x + w - 14, y + h - 14], 10, fill=(19, 27, 44))
    p.text((x + 26, ay + h * 0.04), "AI 回答", h * 0.056, (92, 152, 255), "b")
    for i, r in enumerate((0.9, 0.74, 0.92, 0.5)):
        by = ay + h * 0.145 + i * h * 0.105
        p.rect([x + 26, by, x + 26 + (w - 52) * r, by + h * 0.032], 4, fill=(48, 62, 90))


def mock_app_card_sprite(w, h, hidden=False):
    key = ("appcard", int(w), int(h), hidden)
    if key not in _SPRITE_CACHE:
        _SPRITE_CACHE[key] = hi_res(w, h, lambda p: mock_app_card(p, 0, 0, w, h, hidden), ss=2)
    return _SPRITE_CACHE[key]


# ---------------------------------------------------------------- 会议桌面示意
def mock_desktop(p, w, h, hide_app):
    p.rect([0, 0, w, h], 18, fill=(233, 242, 254))
    p.rect([0, 0, w, h * 0.52], 18, fill=(226, 238, 253))
    p.ellipse([w * 0.56, -h * 0.24, w * 0.99, h * 0.28], fill=(204, 225, 252))
    p.ellipse([-w * 0.12, h * 0.6, w * 0.36, h * 1.14], fill=(212, 240, 251))

    mx, my, mw, mh = w * 0.05, h * 0.1, w * 0.6, h * 0.54
    p.rect([mx, my, mx + mw, my + mh], 12, fill=(24, 31, 45))
    p.rect([mx + 14, my + 12, mx + mw - 14, my + 26], 6, fill=(38, 48, 66))
    tw = (mw - 44) / 2
    th = (mh - 92) / 2
    for i, nm in enumerate(("面试官", "我")):
        tx = mx + 14 + i * (tw + 16)
        ty = my + 38
        p.rect([tx, ty, tx + tw, ty + th], 8, fill=(35, 45, 62))
        cx, cy = tx + tw / 2, ty + th * 0.44
        p.ellipse([cx - 16, cy - 18, cx + 16, cy + 14], fill=(80, 98, 128))
        p.ellipse([cx - 7, cy - 30, cx + 7, cy - 16], fill=(104, 124, 156))
        p.text((tx + 10, ty + th - 24), nm, 13, (196, 210, 230), "r")
    cby = my + mh - 30
    p.rect([mx + mw / 2 - 84, cby, mx + mw / 2 + 84, cby + 20], 10, fill=(44, 55, 76))
    for i in (-1, 0):
        p.ellipse([mx + mw / 2 - 26 + i * 30, cby + 4, mx + mw / 2 - 12 + i * 30, cby + 18],
                  fill=(148, 166, 194))
    p.ellipse([mx + mw / 2 + 16, cby + 4, mx + mw / 2 + 30, cby + 18], fill=(232, 82, 82))

    ax, ay = w * 0.54, h * 0.36
    aw, ah = w * 0.42, h * 0.52
    if not hide_app:
        p.rect([ax - 4, ay - 4, ax + aw + 8, ay + ah + 14], 24, fill=(150, 176, 216, 70))
    mock_app_card(p, ax, ay, aw, ah, hidden=hide_app)

    tag_x = w - 148
    p.rect([tag_x, h - 44, w - 14, h - 14], 15, fill=(255, 255, 255, 235))
    p.ellipse([tag_x + 12, h - 34, tag_x + 20, h - 26], fill=(232, 82, 82))
    p.text((tag_x + 28, h - 39), "正在共享", 14, (86, 102, 128), "r")


def mock_desktop_sprite(w, h, hide_app):
    key = ("desk", int(w), int(h), hide_app)
    if key not in _SPRITE_CACHE:
        _SPRITE_CACHE[key] = hi_res(w, h, lambda p: mock_desktop(p, w, h, hide_app), ss=2)
    return _SPRITE_CACHE[key]


def demo_panel(w, h, hide):
    key = ("panel", int(w), int(h), hide)
    if key in _SPRITE_CACHE:
        return _SPRITE_CACHE[key]

    def build(p):
        p.rect([0, 0, w, h], 26, fill=(255, 255, 255))

    panel = hi_res(w, h, build, ss=2, downscale=False)
    desk = hi_res(w - 30, h - 30, lambda p: mock_desktop(p, w - 30, h - 30, hide),
                  ss=2, downscale=False)
    panel.paste(desk, (30, 30), desk)
    Pen(panel, ImageDraw.Draw(panel), 2).rect([0, 0, w, h], 26, outline=(212, 229, 250), width=2)
    panel = panel.resize((int(w), int(h)), Image.LANCZOS)

    _SPRITE_CACHE[key] = panel
    return panel


# ---------------------------------------------------------------- 场景 1：开场
def scene_intro(t):
    img = BG.copy()
    draw_bg_motion(img, t)
    d = ImageDraw.Draw(img)

    a = ease_out(seg(t, 0.0, 0.7))
    lg = logo_sprite(102)
    paste(img, lg, 176, 148 - 16 * (1 - a), a)

    a2 = ease_out(seg(t, 0.3, 0.6))
    x = 182
    for ch in "本次更新":
        text_alpha(img, (x, 306), ch, 25, GRAY, "r", a2)
        x += text_w(ch, 25, "r") + 8

    a3 = ease_out(seg(t, 0.5, 0.75))
    text_alpha(img, (176, 348 - 24 * (1 - a3)), "会议里，让窗口", 88, NAVY, "b", a3)

    a4 = ease_out(seg(t, 0.8, 0.85))
    gt = gradient_text("彻底隐身", 110, (26, 86, 240), (52, 198, 232), "b")
    paste(img, gt, 176 - 14, 458 + 26 * (1 - a4) - 14, a4)

    a5 = ease_out(seg(t, 1.3, 0.8))
    if a5 > 0:
        uw = 470 * a5
        d.line([(198, 626), (198 + uw * 0.55, 620), (198 + uw, 608)],
               fill=(58, 200, 232), width=9, joint="curve")

    a6 = ease_out(seg(t, 1.55, 0.7))
    text_alpha(img, (182, 664), "1 个重点功能，5 处体验改进", 31, GRAY, "r", a6)

    labels = ["防录屏", "进程级回环", "云端 ASR 四家", "本地语义检索"]
    cx = 180
    for i, s in enumerate(labels):
        ai = ease_out(seg(t, 1.95 + i * 0.15, 0.5))
        sp = pill_cached(s, 23)
        paste(img, sp, cx, 748 + 14 * (1 - ai), ai)
        cx += sp.width + 14

    a9 = ease_out(seg(t, 2.8, 0.7))
    if a9 > 0:
        d.line([(182, 826), (182 + 120, 826)], fill=(198, 216, 240), width=2)
        text_alpha(img, (318, 812), "更新日期 2026.11.22", 20, (168, 186, 210), "r", a9)

    a7 = ease_out(seg(t, 0.85, 0.9))
    cw, ch = 600, 448
    cxm, cym = 1150, 316 + 34 * (1 - a7)
    card = mock_app_card_sprite(cw, ch)
    paste(img, card, cxm, cym, a7, shadow=(shadow_sprite(cw, ch, 20, blur=32, alpha=78, dy=14), -26))

    a8 = ease_out(seg(t, 2.1, 0.75))
    if a8 > 0:
        badge = chip("录屏中不可见", 23, "shield", BLUE)
        paste(img, badge, 1030, 724 + 16 * (1 - a8), a8,
              shadow=(shadow_sprite(badge.width, badge.height, badge.height / 2,
                                    blur=18, alpha=60, dy=6), -12))

    return img


# ---------------------------------------------------------------- 场景 2：会议中防录屏
def scene_hero(t):
    img = BG.copy()
    draw_bg_motion(img, t)
    d = ImageDraw.Draw(img)

    a0 = ease_out(seg(t, 0.0, 0.6))
    tag = pill_cached("重点更新", 23, "b", WHITE, BLUE, BLUE)
    paste(img, tag, 160, 96 + 12 * (1 - a0), a0)

    a1 = ease_out(seg(t, 0.12, 0.7))
    text_alpha(img, (158, 146 - 22 * (1 - a1)), "会议中防录屏", 76, NAVY, "b", a1)

    a2 = ease_out(seg(t, 0.32, 0.7))
    text_alpha(img, (164, 250), "系统级防捕获：共享画面、录屏、截图里都没有这个窗口", 29, GRAY, "r", a2)

    pw, ph = 770, 452
    ly = 320
    for i, (x, hide, label, accent) in enumerate((
            (160, False, "你的桌面", (120, 138, 164)),
            (990, True, "对方看到的共享画面", BLUE))):
        ai = ease_out(seg(t, 0.55 + i * 0.25, 0.85))
        if ai <= 0:
            continue
        xx = x + 26 * (1 - ai)
        yy = ly + 5 * math.sin(t * 1.1 + i * 1.7)
        card = demo_panel(pw, ph, hide)
        paste(img, card, xx, yy, ai,
              shadow=(shadow_sprite(pw, ph, 26, blur=28, alpha=50, dy=12), -18))
        d.ellipse([xx - 22, yy - 33, xx - 10, yy - 21], fill=accent)
        text_alpha(img, (xx + 2, yy - 46), label, 24, accent, "b", ai)

    # 右面板的「扫描」光带：强调画面里已经没有多余窗口
    a_scan = ease_out(seg(t, 2.2, 0.5))
    if a_scan > 0:
        cyc = (t * 0.42) % 1.0
        band = scan_band(pw - 60)
        by = 320 + 40 + cyc * (ph - 140)
        paste(img, band, 990 + 30, by, a_scan * 0.85)

    a3 = ease_out(seg(t, 1.3, 0.6))
    if a3 > 0:
        arrow = hi_res(112, 112, lambda p: (
            p.rect([0, 0, 112, 112], 56, fill=(255, 255, 255), outline=(206, 226, 255), width=2),
            p.line([(40, 56), (74, 56)], BLUE, 5),
            p.line([(60, 42), (74, 56), (60, 70)], BLUE, 5),
        ), ss=2)
        pulse = 1 + 0.05 * math.sin(t * 2.4)
        if abs(pulse - 1) > 0.005:
            nw, nh = int(arrow.width * pulse), int(arrow.height * pulse)
            arrow = arrow.resize((nw, nh), Image.LANCZOS)
        paste(img, arrow, 950 - arrow.width / 2, 550 - arrow.height / 2 + 10 * (1 - a3), a3,
              shadow=(shadow_sprite(arrow.width, arrow.height, arrow.height / 2,
                                    blur=16, alpha=46, dy=6), -8))

    a4 = ease_out(seg(t, 1.85, 0.7))
    if a4 > 0:
        tip = hi_res(310, 58, lambda p: (
            p.rect([0, 0, 310, 58], 29, fill=(31, 92, 240)),
            p.text((155, 29), "已排除出录制画面", 24, WHITE, "b", "mm"),
        ), ss=2)
        paste(img, tip, 1220, 800 + 16 * (1 - a4) + 4 * math.sin(t * 1.5), a4)

    chips = ("屏幕共享不可见", "截图 / 录屏留白", "设置里一键开关")
    cx = 160
    for i, s in enumerate(chips):
        ai = ease_out(seg(t, 2.5 + i * 0.26, 0.55))
        sp = chip(s, 24, "check", CYAN)
        paste(img, sp, cx + 18 * (1 - ai), 818, ai)
        cx += sp.width + 22

    a5 = ease_out(seg(t, 3.5, 0.7))
    text_alpha(img, (162, 886),
               "Windows 10 2004+ · WDA_EXCLUDEFROMCAPTURE　|　macOS · set_content_protected",
               20, GRAY_L, "r", a5)

    return img


# ---------------------------------------------------------------- 场景 3：隐身配套
def scene_companion(t):
    img = BG.copy()
    draw_bg_motion(img, t)
    d = ImageDraw.Draw(img)

    a0 = ease_out(seg(t, 0.0, 0.6))
    tag = pill_cached("隐身配套", 23, "b", WHITE, BLUE, BLUE)
    paste(img, tag, 160, 148 + 12 * (1 - a0), a0)

    a1 = ease_out(seg(t, 0.12, 0.7))
    text_alpha(img, (158, 198 - 22 * (1 - a1)), "隐身之后，用得更顺", 70, NAVY, "b", a1)

    items = [
        ("mini", "迷你页待命", "窗口缩成小卡片贴边停靠，一眼扫到答案，不挡题", "Ctrl + Shift + M"),
        ("tray", "托盘常驻", "不占任务栏，关掉主界面也还在，开会前再唤起", "系统托盘"),
        ("keyboard", "全局快捷键", "在会议软件里照样能一键提问、暂停，不用切窗口", "Ctrl + Shift + A / P"),
    ]
    pen = Pen(img, d, 1.0)
    for i, (ic, title, desc, hint) in enumerate(items):
        ai = ease_out(seg(t, 0.45 + i * 0.42, 0.6))
        if ai <= 0:
            continue
        y = 368 + i * 178 + 22 * (1 - ai)
        x = 168 + 34 * (1 - ai)
        d.ellipse([x, y, x + 72, y + 72], fill=(234, 243, 255))
        draw_icon(pen, ic, x + 36, y + 36, 32, BLUE)
        text_alpha(img, (x + 94, y + 2), title, 35, NAVY, "b", ai)
        text_alpha(img, (x + 94, y + 50), desc, 22, GRAY, "r", ai)
        hp = pill_cached(hint, 19, "r", BLUE, (238, 245, 255), (210, 230, 255))
        paste(img, hp, x + 94, y + 88, ai)

    a2 = ease_out(seg(t, 1.0, 0.9))
    if a2 > 0:
        cw, ch = 460, 372
        cx, cy = 1258, 296 + 30 * (1 - a2)
        card = mock_app_card_sprite(cw, ch)
        paste(img, card, cx, cy, a2, shadow=(shadow_sprite(cw, ch, 20, blur=30, alpha=74, dy=14), -24))

        a3 = ease_out(seg(t, 1.9, 0.6))
        if a3 > 0:
            bub = chip("Ctrl + Shift + M", 23)
            paste(img, bub, 1180, 694 + 16 * (1 - a3), a3,
                  shadow=(shadow_sprite(bub.width, bub.height, bub.height / 2,
                                       blur=16, alpha=52, dy=6), -10))

        text_alpha(img, (1262, 784), "迷你页 · 常驻置顶", 22, GRAY, "r", a2)

    return img


# ---------------------------------------------------------------- 场景 4：其他更新
def scene_others(t):
    img = BG.copy()
    draw_bg_motion(img, t)
    d = ImageDraw.Draw(img)

    a0 = ease_out(seg(t, 0.0, 0.6))
    tag = pill_cached("其他更新", 23, "b", BLUE, (230, 241, 255), (198, 220, 255))
    paste(img, tag, 160, 92 + 12 * (1 - a0), a0)

    a1 = ease_out(seg(t, 0.1, 0.65))
    text_alpha(img, (158, 140 - 20 * (1 - a1)), "顺手把这些也办了", 62, NAVY, "b", a1)

    rows = [
        ("wave", "只听该听的声音",
         "系统音频采集范围可选「只采集指定应用」，会议软件单独锁，聊天提示音不再串进转写", "音频采集"),
        ("cloud", "云端识别四家齐了",
         "讯飞、腾讯云、OpenAI 兼容新接入，与阿里云一起在设置里任意切换", "语音识别"),
        ("search", "知识库本地语义检索",
         "内置 BGE-M3 向量模型，检索不出本机；模型缺失时自动回退关键词检索", "知识库"),
        ("box", "一个安装包全带上",
         "本地 ASR 与本地嵌入合并打包，断网也能跑起来", "离线打包"),
        ("refresh", "设备「重新检测」",
         "换耳机、插麦克风不用重启应用；设备拔出后自动回退到系统默认", "音频设备"),
    ]
    top, row_h = 250, 136
    pen = Pen(img, d, 1.0)
    for i, (ic, title, desc, tagtxt) in enumerate(rows):
        ai = ease_out(seg(t, 0.3 + i * 0.4, 0.6))
        if ai <= 0:
            continue
        y = top + i * row_h
        x = 176 + 40 * (1 - ai)
        lw = 1560 * ease_out(seg(t, 0.3 + i * 0.4, 0.7))
        if i > 0:
            d.line([(x, y - 16), (x + lw, y - 16)], fill=(223, 232, 246), width=2)
        d.ellipse([x, y + 12, x + 68, y + 80], fill=(234, 243, 255))
        draw_icon(pen, ic, x + 34, y + 46, 30, BLUE)
        text_alpha(img, (x + 90, y + 8), title, 32, NAVY, "b", ai)
        text_alpha(img, (x + 90, y + 54), desc, 21, GRAY, "r", ai)
        tg = pill_cached(tagtxt, 19, "r", (108, 132, 168), (243, 247, 253), (228, 236, 248))
        paste(img, tg, 1700 - tg.width, y + 30, ai)

    return img


# ---------------------------------------------------------------- 场景 5：结尾
def scene_outro(t):
    img = BG.copy()
    draw_bg_motion(img, t)
    d = ImageDraw.Draw(img)

    a0 = ease_out(seg(t, 0.0, 0.8))
    lg = logo_sprite(118)
    paste(img, lg, (W - lg.width) / 2, 250 + 20 * (1 - a0), a0)

    a1 = ease_out(seg(t, 0.35, 0.75))
    w1 = text_w("更新已就绪", 84, "b")
    text_alpha(img, ((W - w1) / 2, 468 + 22 * (1 - a1)), "更新已就绪", 84, NAVY, "b", a1)

    a2 = ease_out(seg(t, 0.75, 0.7))
    sub = "打开 Easy 更新到最新版，下一次面试就能用上"
    text_alpha(img, ((W - text_w(sub, 33, "r")) / 2, 594), sub, 33, GRAY, "r", a2)

    a3 = ease_out(seg(t, 1.25, 0.75))
    if a3 > 0:
        badge = chip("开着隐身模式，安心开完这一场", 22, "shield", BLUE,
                     bg=(232, 242, 255), border=(232, 242, 255))
        paste(img, badge, (W - badge.width) / 2, 692 + 14 * (1 - a3), a3)

    return img


SCENES = [
    (0.0, 7.0, scene_intro),
    (7.0, 30.0, scene_hero),
    (30.0, 39.5, scene_companion),
    (39.5, 57.0, scene_others),
    (57.0, 65.0, scene_outro),
]


def draw_global(img, t):
    d = ImageDraw.Draw(img)
    x0, x1, y = 160, 1760, 1054
    d.line([(x0, y), (x1, y)], fill=(210, 226, 246), width=2)
    p = clamp(t / TOTAL)
    d.line([(x0, y), (x0 + (x1 - x0) * p, y)], fill=(124, 174, 255), width=2)
    for s, _, _ in SCENES[1:]:
        xc = x0 + (x1 - x0) * (s / TOTAL)
        d.ellipse([xc - 3, y - 3, xc + 3, y + 3], fill=(210, 226, 246))


def scene_at(t):
    idx = 0
    for i, (s, e, _) in enumerate(SCENES):
        if s <= t < e:
            idx = i
    return idx


def wipe_sprite():
    """转场时的品牌色光带：遮住新场景刚起步时的空档。"""
    key = ("wipe",)
    if key in _SPRITE_CACHE:
        return _SPRITE_CACHE[key]
    w = 430
    xs = np.arange(w)
    core = 184  # 实心部分半宽
    fade = 2
    alpha = np.zeros(w)
    alpha[(xs > w / 2 - core) & (xs < w / 2 + core)] = 255
    for i in range(fade):
        v = 255 * (i / fade) ** 1.6
        alpha[int(w / 2 - core) - 1 - i] = v
        alpha[int(w / 2 + core) + i] = v
    t = np.clip((xs - (w / 2 - core)) / (2 * core), 0, 1)[:, None]
    c1 = np.array([46, 104, 245], float)
    c2 = np.array([62, 200, 235], float)
    rgb = c1[None, :] * (1 - t) + c2[None, :] * t
    arr = np.zeros((H, w, 4), np.uint8)
    arr[..., :3] = np.repeat(rgb[None, :, :], H, axis=0).astype(np.uint8)
    arr[..., 3] = np.repeat(np.clip(alpha, 0, 255).astype(np.uint8)[None, :], H, axis=0)
    img = Image.fromarray(arr, "RGBA")
    _SPRITE_CACHE[key] = img
    return img


def wipe(prev, cur, p):
    """从左往右的柔边溶膜转场。"""
    soft = 170
    edge = p * (W + soft * 2) - soft
    col = np.clip((edge - np.arange(W)) / soft, 0.0, 1.0)
    mask = Image.fromarray(np.repeat((col * 255).astype(np.uint8)[None, :], H, axis=0), "L")
    out = Image.composite(cur, prev, mask)
    if 0.02 < p < 0.98:
        band = wipe_sprite()
        paste(out, band, edge - band.width / 2, 0, 1.0)
    return out


def render_frame(t):
    idx = scene_at(t)
    start, end, fn = SCENES[idx]
    img = fn(t - start)

    # 场景进入时用柔边溶膜覆盖上一场景终态，避免白场空档
    if idx > 0 and (t - start) < XF:
        ps, pe, pfn = SCENES[idx - 1]
        prev = pfn(pe - ps)
        img = wipe(prev, img, ease_in_out((t - start) / XF))

    draw_global(img, t)
    draw_subtitle(img, t)
    return img


# ---------------------------------------------------------------- 字幕
def subtitle_sprite(text):
    key = ("subtitle", text)
    if key in _SPRITE_CACHE:
        return _SPRITE_CACHE[key]
    size = 37
    max_w = 1520
    lines, cur = [], ""
    for ch in text:
        if cur and text_w(cur + ch, size, "b") > max_w:
            lines.append(cur)
            cur = ch
        else:
            cur += ch
    if cur:
        lines.append(cur)
    lh = 52
    w = int(max(text_w(s, size, "b") for s in lines) + 84)
    h = int(lh * len(lines) + 36)

    def build(p):
        p.rect([0, 0, w, h], 20, fill=(14, 30, 58, 214))
        p.rect([0, 0, w, h], 20, outline=(122, 164, 226, 130), width=2)
        for j, line in enumerate(lines):
            p.text((w / 2, 18 + lh * j + lh / 2), line, size, WHITE, "b", "mm")

    _SPRITE_CACHE[key] = hi_res(w, h, build, ss=2)
    return _SPRITE_CACHE[key]


def draw_subtitle(img, t):
    for i, (s, e, text) in enumerate(SUBTITLES):
        if s - 0.1 <= t <= e:
            a = min(ease_out(clamp((t - s + 0.1) / 0.2)), ease_out(clamp((e - t) / 0.22)))
            if a <= 0:
                return
            sp = subtitle_sprite(text)
            paste(img, sp, (W - sp.width) / 2, 908, a)
            return


# ---------------------------------------------------------------- 中文旁白（系统 SAPI）
def read_mono_wav(path):
    with wave.open(path, "rb") as f:
        raw = f.readframes(f.getnframes())
        sr, ch, sw = f.getframerate(), f.getnchannels(), f.getsampwidth()
    a = np.frombuffer(raw, dtype=np.int16).astype(np.float32) / 32768.0
    if ch == 2:
        a = a.reshape(-1, 2).mean(axis=1)
    return a, sr


def resample(a, src, dst):
    if src == dst:
        return a
    n = max(1, int(round(len(a) * dst / src)))
    x0 = np.arange(len(a))
    return np.interp(np.linspace(0, max(0, len(a) - 1), n), x0, a).astype(np.float32)


def sapi_synth(text, path):
    quoted = text.replace("'", "''")
    ps = (
        "Add-Type -AssemblyName System.Speech\n"
        "$s = New-Object System.Speech.Synthesis.SpeechSynthesizer\n"
        f"$s.SelectVoice('{VOICE}')\n"
        f"$s.Rate = {TTS_RATE}\n"
        "$s.Volume = 100\n"
        f"$s.SetOutputToWaveFile('{path}')\n"
        f"$s.Speak('{quoted}')\n"
        "$s.Dispose()\n"
    )
    r = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", ps],
                       capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f"旁白合成失败：{r.stderr.strip()}")


def prepare_narration(sr=44100):
    """逐句合成中文旁白，返回 (单声道音轨, 字幕时间轴)。"""
    global SUBTITLES
    os.makedirs(TTS_DIR, exist_ok=True)
    n = int(TOTAL * sr)
    track = np.zeros(n, np.float32)
    subs = []
    prev_end = 0.0
    for i, (want, text) in enumerate(NARRATION):
        path = os.path.join(TTS_DIR, f"{i:02d}.wav")
        if not os.path.exists(path) or os.path.getsize(path) < 1000:
            sapi_synth(text, path)
        a, src_sr = read_mono_wav(path)
        a = resample(a, src_sr, sr)
        nz = np.where(np.abs(a) > 0.012)[0]
        if len(nz) > sr // 4:
            a = a[max(0, nz[0] - 60): min(len(a), nz[-1] + 120)]
        dur = len(a) / sr
        start = max(want, prev_end + 0.16)
        start = min(start, TOTAL - 0.25 - dur)
        i0 = int(start * sr)
        seg = a[:n - i0]
        fade = min(int(0.035 * sr), max(1, len(seg) // 6))
        seg[:fade] *= np.linspace(0, 1, fade, dtype=np.float32)
        seg[-fade:] *= np.linspace(1, 0, fade, dtype=np.float32)
        track[i0:i0 + len(seg)] += seg * 1.12
        end = start + dur
        subs.append((start, min(TOTAL - 0.05, end + 0.12), text))
        prev_end = end
        print(f"  narration {i + 1:02d}/{len(NARRATION)}: {start:5.2f}s - {end:5.2f}s  {text}")
    SUBTITLES = subs
    return track, subs


# ---------------------------------------------------------------- 音乐
def build_music(dur, sr=44100):
    """132 BPM 电子风格配乐：四拍底鼓 + 军鼓 + 踩镲 + 八分贝斯 + 十六分琶音。"""
    rng = np.random.default_rng(20261122)
    n = int(dur * sr)
    L = np.zeros(n, np.float32)
    R = np.zeros(n, np.float32)
    beat = 60.0 / 132.0
    bar = beat * 4

    def hz(midi):
        return 440.0 * 2 ** ((midi - 69) / 12.0)

    def add(t0, sig, gain=1.0, pan=0.0):
        i0 = int(t0 * sr)
        if i0 >= n:
            return
        s = sig[:n - i0]
        ga = math.cos((pan + 1) * math.pi / 4)
        gb = math.sin((pan + 1) * math.pi / 4)
        L[i0:i0 + len(s)] += s * gain * ga
        R[i0:i0 + len(s)] += s * gain * gb

    def kick():
        m = int(0.26 * sr)
        tt = np.arange(m) / sr
        f = 158 * np.exp(-tt * 31) + 47
        sig = np.sin(2 * np.pi * np.cumsum(f) / sr) * np.exp(-tt * 13)
        sig += rng.normal(0, 1, m) * np.exp(-tt * 280) * 0.22
        return sig.astype(np.float32)

    def snare():
        m = int(0.18 * sr)
        tt = np.arange(m) / sr
        noise = rng.normal(0, 1, m)
        noise = noise - np.concatenate(([0], noise[:-1]))
        sig = noise * np.exp(-tt * 24) + np.sin(2 * np.pi * 185 * tt) * np.exp(-tt * 18) * 0.35
        return sig.astype(np.float32)

    def hat(open_hat=False):
        m = int((0.11 if open_hat else 0.042) * sr)
        tt = np.arange(m) / sr
        noise = rng.normal(0, 1, m)
        noise = noise - np.concatenate(([0], noise[:-1]))
        return (noise * np.exp(-tt * (28 if open_hat else 86))).astype(np.float32)

    def bass(freq, d):
        m = int(d * sr)
        tt = np.arange(m) / sr
        sig = np.sin(2 * np.pi * freq * tt) + 0.35 * np.sin(2 * np.pi * freq * 2 * tt)
        return (sig * np.minimum(1, tt / 0.008) * np.exp(-tt * 3.0)).astype(np.float32)

    def pluck(freq, d):
        m = int(d * sr)
        tt = np.arange(m) / sr
        sig = (np.sin(2 * np.pi * freq * tt)
               + 0.4 * np.sin(2 * np.pi * freq * 2.01 * tt)
               + 0.18 * np.sin(2 * np.pi * freq * 3.02 * tt))
        return (sig * np.exp(-tt * 9) * np.minimum(1, tt / 0.004)).astype(np.float32)

    def stab(freqs, d):
        m = int(d * sr)
        tt = np.arange(m) / sr
        sig = np.zeros(m)
        for f in freqs:
            sig += (np.sin(2 * np.pi * f * tt)
                    + 0.45 * np.sin(2 * np.pi * f * 2.003 * tt)
                    + 0.2 * np.sin(2 * np.pi * f * 3.01 * tt))
        return (sig * np.exp(-tt * 7) * np.minimum(1, tt / 0.006) / len(freqs)).astype(np.float32)

    def sweep():
        m = int(0.42 * sr)
        tt = np.arange(m) / sr
        noise = np.convolve(rng.normal(0, 1, m), np.ones(64) / 64, mode="same")
        return (noise * np.sin(np.pi * tt / tt[-1]) * 0.5).astype(np.float32)

    prog = [([57, 60, 64], 33), ([53, 57, 60], 29),
            ([60, 64, 67], 36), ([55, 59, 62], 31)]  # Am - F - C - G
    ks, ss = kick(), snare()
    hs, ho = hat(), hat(True)
    tpos, ci = 0.0, 0
    while tpos < dur:
        chord, root = prog[ci % 4]
        m = int(min(bar, dur - tpos) * sr)
        tt = np.arange(m) / sr
        pad = np.zeros(m)
        for mm in chord:
            for h, amp in ((1, 1.0), (2, 0.3), (3, 0.12)):
                pad += amp * np.sin(2 * np.pi * hz(mm) * h * tt + rng.uniform(0, 6.28))
        pad *= np.minimum(1, tt / 0.25) * np.minimum(1, np.maximum(0, (bar - tt) / 0.3))
        pad = (pad / len(chord) * 0.055).astype(np.float32)
        add(tpos, pad, 1.0, -0.22)
        add(tpos, pad, 1.0, 0.22)

        for b in (1.5, 3.5):
            st = stab([hz(x) for x in chord], 0.42)
            add(tpos + b * beat, st, 0.5, -0.3 if b < 2 else 0.3)

        for k in range(8):
            f = hz(root - 12 + (7 if k in (3, 7) else 0))
            add(tpos + k * beat * 0.5, bass(f, beat * 0.45), 0.28 if k % 2 == 0 else 0.2)

        arp = [chord[0] + 12, chord[1] + 12, chord[2] + 12, chord[1] + 12,
               chord[2] + 12, chord[0] + 24, chord[1] + 12, chord[2] + 12]
        for k in range(16):
            mm = arp[k % 8] + (12 if k >= 8 else 0)
            add(tpos + k * beat * 0.25, pluck(hz(mm), 0.32), 0.055, -0.35 if k % 2 else 0.35)

        for b in range(4):
            add(tpos + b * beat, ks, 0.52)
            if b in (1, 3):
                add(tpos + b * beat, ss, 0.3)
            add(tpos + b * beat + beat * 0.5, hs, 0.075, 0.25)
            add(tpos + b * beat + beat * 0.75, hs, 0.05, -0.25)
        add(tpos + bar - beat * 0.5, ho, 0.09, 0.1)
        tpos += bar
        ci += 1

    for st in (7.0, 30.0, 39.5, 57.0):
        add(st - 0.38, sweep(), 0.55)

    # 底鼓侧链泵感
    phase = (np.arange(n) / sr) % beat
    pump = (1.0 - 0.32 * np.exp(-(phase / 0.13) ** 2)).astype(np.float32)
    L *= pump
    R *= pump

    ir_n = int(0.6 * sr)
    ir = rng.normal(0, 1, ir_n) * np.exp(-np.arange(ir_n) / (0.18 * sr))
    ir /= np.sqrt(np.sum(ir ** 2)) + 1e-9
    for buf in (L, R):
        buf += np.convolve(buf, ir, mode="full")[:n] * 0.14

    tt = np.arange(n) / sr
    fade = np.minimum(1, tt / 1.2) * np.minimum(1, np.maximum(0, (dur - tt) / 2.2))
    L *= fade
    R *= fade
    peak = max(np.abs(L).max(), np.abs(R).max()) + 1e-9
    L = np.tanh(L / peak * 1.25) * 0.52
    R = np.tanh(R / peak * 1.25) * 0.52
    return L, R


def build_audio(sr=44100):
    """合成旁白 + 激情配乐，旁白出现时压低音乐，输出混音 WAV。"""
    narr, subs = prepare_narration(sr)
    L, R = build_music(TOTAL, sr)
    n = len(L)

    mask = np.zeros(n, np.float32)
    for s, e, _ in subs:
        i0 = int(max(0, s - 0.16) * sr)
        i1 = int(min(TOTAL, e + 0.22) * sr)
        mask[i0:i1] = 1.0
    k = int(0.12 * sr)
    k += k % 2 == 0
    smooth = np.convolve(mask, np.ones(k) / k, mode="same")
    g = (1.0 - 0.62 * smooth).astype(np.float32)
    L *= g
    R *= g

    voice = np.tanh(narr[:n] * 2.5) * 0.88
    L += voice
    R += voice * 0.98
    peak = max(np.abs(L).max(), np.abs(R).max()) + 1e-9
    if peak > 0.94:
        L = L / peak * 0.94
        R = R / peak * 0.94

    pcm = np.clip(np.stack([L, R], axis=1) * 32767, -32768, 32767).astype(np.int16)
    with wave.open(MIX_WAV, "wb") as f:
        f.setnchannels(2)
        f.setsampwidth(2)
        f.setframerate(sr)
        f.writeframes(pcm.tobytes())
    print("audio ->", MIX_WAV)
    return MIX_WAV


# ---------------------------------------------------------------- 主流程
def main():
    global BG, BLOB_A, BLOB_B
    ap = argparse.ArgumentParser()
    ap.add_argument("--preview", type=str, default="")
    ap.add_argument("--render", action="store_true")
    ap.add_argument("--audio-only", action="store_true")
    ap.add_argument("--no-audio", action="store_true")
    args = ap.parse_args()

    BG = build_background()
    BLOB_A = float_blob(760, (188, 216, 255), 120)
    BLOB_B = float_blob(680, (196, 238, 250), 110)

    if not args.no_audio:
        print("合成旁白与配乐…")
        build_audio()

    if args.audio_only:
        return

    if args.preview:
        os.makedirs(PREVIEW_DIR, exist_ok=True)
        for s in args.preview.split(","):
            t = float(s)
            img = render_frame(t)
            path = os.path.join(PREVIEW_DIR, f"t{t:05.1f}.png")
            img.save(path)
            print("preview ->", path)
        return

    if not args.render:
        print("用法：--preview 3,12 | --render | --audio-only")
        return

    if not os.path.exists(FFMPEG):
        raise SystemExit("找不到 ffmpeg-static，请先 npm install --no-save ffmpeg-static")

    wav = None if args.no_audio else MIX_WAV
    frames = int(TOTAL * FPS)
    cmd = [FFMPEG, "-y", "-hide_banner", "-loglevel", "error",
           "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}", "-r", str(FPS), "-i", "-"]
    if wav:
        cmd += ["-i", wav]
    cmd += ["-c:v", "libx264", "-preset", "medium", "-crf", "19",
            "-pix_fmt", "yuv420p", "-profile:v", "high", "-movflags", "+faststart"]
    if wav:
        cmd += ["-c:a", "aac", "-b:a", "192k", "-shortest"]
    cmd += [OUT_MP4]

    proc = subprocess.Popen(cmd, stdin=subprocess.PIPE)
    for f in range(frames):
        img = render_frame(f / FPS)
        proc.stdin.write(img.convert("RGB").tobytes())
        if f % 200 == 0:
            print(f"  frame {f}/{frames} ({f / FPS:5.1f}s)", flush=True)
    proc.stdin.close()
    proc.wait()
    print("done ->", OUT_MP4, os.path.getsize(OUT_MP4) // 1024, "KB")


if __name__ == "__main__":
    main()
