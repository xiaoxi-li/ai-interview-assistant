# 轻松面试 · Logo / 应用图标设计

> 设计蓝本：**Easy** → 字母 **E**。样式与色值严格取自客户端现有视觉令牌，
> 因而图标与界面（浅色 / 深色双主题）天然一致。

## 一、定位映射

| 客户端定位 | 图形语言 |
| --- | --- |
| 产品名 Easy（轻松） | 主体是几何字母 **E**，笔画全为胶囊圆角，气质轻、不锋利 |
| 实时转写「边听边答」 | E 的三条横杠同时读作**声波 / 电平条**（中横略短，形成节奏） |
| 低资源、本地优先 | 纯几何双色，无纹理无渐变噪点，16px 仍可辨认 |
| 双主题一致 | 提供品牌渐变版与深色版，取值即 `--primary` 系列 |

## 二、规格

- 画布 1024×1024，圆角比例 **22.37%**（系统级方圆形，四角透明）
- 色板：背景对角渐变 `#5B7CFA → #2B46C4`；主色 `#4F6BED`；浅色点缀 `#7C9AFF`
- 深色版背景 `#1B2440 → #0F1420`，标记渐变 `#B0C5FF → #5B7CFA`
- 笔画粗细 104/1024，横杠圆角半径 = 笔画一半（纯胶囊）
- 标记占比：宽 520/1024 ≈ 51%，高 592/1024 ≈ 58%

## 三、资产清单（`design/icon/`）

```
01-e-wave.png / 02-bubble-e.png   生成式概念图（探索阶段的风格参考）
render.py                         纯 Pillow 渲染脚本（可重复、可改参）
gen.py                            概念图生成脚本（调用云端图片生成服务）
final/
  preview.png                     方案对比图（评审用）
  logo-primary-1024.png           主方案 A：品牌渐变 + 白色 E（推荐）
  logo-primary-{512..32}.png      多尺寸
  logo-dark-1024.png              方案 B：深色主题（深蓝底 + 发光 E）
  logo-flat-1024.png              方案 C：扁平无投影（贴纸风）
  logo-bubble-1024.png            方案 D：对话气泡 + E（听问题→给答案）
  mark-white-512.png              透明底白色标记（托盘 / 单色）
  icon.ico                        多尺寸 ICO
  tauri-icons/                    与 src-tauri/icons 同名同尺寸的完整图标集
```

## 四、重新生成

```powershell
py -3 design/icon/render.py
```

依赖 Pillow（本机已装）。改色 / 改比例只需调整 `render.py` 顶部的品牌令牌与 `BARS` 几何常量。

## 五、应用到客户端

图标集已按 `src-tauri/icons` 现有文件名和尺寸（含 `icon.ico`、`icon.icns`、
Windows Square*Logo、StoreLogo）生成，覆盖即可：

```powershell
Copy-Item design/icon/final/tauri-icons/* src-tauri/icons/ -Force
```

托盘图标取 `app.default_window_icon()`，覆盖后一并生效。
覆盖属于客户端变更：需同步 `ai-doc/技术方案.md` 第 15 章视觉规范，并单独提交。
