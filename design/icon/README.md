# 轻松面试 · Logo / 应用图标设计

> 设计蓝本：**Easy** —— 巨大的几何 **E** 居左，剩下的 **asy** 接在**下横杠末端**，
> 与小字同高的一条水平带收尾，合起来正是产品名。样式与色值严格取自客户端现有
> 视觉令牌，因而图标与界面（浅色 / 深色双主题）天然一致。

## 一、定位映射

| 客户端定位 | 图形语言 |
| --- | --- |
| 产品名 Easy（轻松） | 大 **E** 作主体，**asy** 小字接在下横杠末端，整体即「Easy」 |
| 「轻松」的气质 | 笔画全为胶囊圆角，不锋利；大小字对比形成从容、不紧绷的节奏 |
| 实时转写「边听边答」 | E 的三条横杠同时读作**声波 / 电平条**（中横略短，形成节奏） |
| 低资源、本地优先 | 纯几何双色，无纹理无噪点，16px 仍可辨认 |
| 双主题一致 | 提供品牌渐变版与深色版，取值即 `--primary` 系列 |

## 二、规格

- 画布 1024×1024，圆角比例 **22.37%**（系统级方圆形，四角透明）
- 色板：背景对角渐变 `#5B7CFA → #2B46C4`；主色 `#4F6BED`；深色版背景 `#1B2440 → #0F1420`
- 大 E：缩放 **0.82**（高 485、宽 426，占画布 47% / 42%），笔画 104×0.82，横杠圆角 = 笔画一半（纯胶囊）
- 小 asy：`Segoe UI Semibold` **240px**，接在**下横杠末端**（间距 18px）；其字母主体 x-height（≈123）与下横杠（85）**等高居中对齐**，视觉上连成同一条水平带，「y」的降部自然垂到 E 底边之下
- 大小关系：E 高 485 / asy 主体 123 ≈ 4:1，既保住「E 是主体」，又让 asy 有足够分量读成一个词
- 小尺寸策略（响应式字标）：**≤ 64px 只保留大 E**，标记放大到 1.22 倍——asy 在这个尺寸会糊成一团

## 三、资产清单（`design/icon/`）

```
01-e-wave.png / 02-bubble-e.png   生成式概念图（探索阶段的风格参考）
render.py                         纯 Pillow 渲染脚本（可重复、可改参）
gen.py                            概念图生成脚本（调用云端图片生成服务）
final/
  preview.png                     方案对比图（评审用）
  logo-primary-1024.png           主方案 A：大 E + 右侧小 asy（推荐）
  logo-primary-{512..32}.png      多尺寸
  logo-nested-1024.png            方案 B：满幅 E，小 asy 嵌进中横与下横之间的开口
  logo-dark-1024.png              方案 C：深色主题（深蓝底 + 发光蓝字标）
  logo-flat-1024.png              方案 D：扁平无投影
  mark-white-512.png              透明底白色大 E（托盘 / 单色）
  icon.ico                        多尺寸 ICO
  tauri-icons/                    与 src-tauri/icons 同名同尺寸的完整图标集
```

## 四、重新生成

```powershell
py -3 design/icon/render.py
```

依赖 Pillow。改色 / 改比例 / 换字体只需调整 `render.py` 顶部的品牌令牌、
`BARS` 几何常量与 `draw_word_lockup()` 的默认参数。

## 五、应用到客户端

图标集已按 `src-tauri/icons` 现有文件名和尺寸（含 `icon.ico`、`icon.icns`、
Windows Square*Logo、StoreLogo）生成，覆盖即可：

```powershell
Copy-Item design/icon/final/tauri-icons/* src-tauri/icons/ -Force
```

托盘图标取 `app.default_window_icon()`，覆盖后一并生效。
覆盖属于客户端变更：需同步 `ai-doc/技术方案.md` 第 15 章视觉规范，并单独提交。
