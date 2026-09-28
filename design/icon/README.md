# 轻松面试 · 品牌标识与应用图标

> 以设计方品牌稿为准，**成品素材原样使用：不重绘、不改色、不改几何、不加描边或字重补偿**。

## 一、品牌要素（取自设计稿）

| 项 | 内容 |
| --- | --- |
| 品牌名 | **Easy** · 轻松面试 |
| Slogan | 让面试，更轻松 |
| 主图形 | 三条**渐变波浪横杠**（字母 E 的意象；流畅线条象征语音转写与信息流动） |
| 主色 | `#3B82F6` 科技 · 信赖 |
| 辅助色 | `#22D3A6` 活力 · 成长 |
| 浅色底 | `#E6F0FF` 清晰 · 专注 |
| 文字色 | `#0F172A` 专业 · 清晰 |
| 字体 | 中文：思源黑体 / 阿里巴巴普惠体；英文：Inter / Poppins |
| 造型风格 | 全圆润、渐变色彩、简洁现代 |

> 界面设计令牌仍是 `#4F6BED` 系（见《技术方案》15.1–15.3），**本次不改界面主色**；
> 品牌 logo 使用上表色板，新增品牌视觉元素时优先取 logo 色。

## 二、成品素材（直接使用，勿改）

```
final/brand/                 设计方导出的透明底成品
  mark-1024.png              主标记（透明底）1024
  mark-256.png               主标记（透明底）256   ← 界面用
  icon-app-1024.png          应用图标（圆角卡片）
  icon-app-256.png           应用图标（圆角卡片）
  horizontal.png             横版组合 1560×560（标记 + Easy + slogan）
  vertical.png               竖版组合 720×792
final/tauri-icons/           整套客户端图标（15 个 PNG + icon.ico + icon.icns）
source/brand-sheet.png       品牌设计稿原图 1080×720（要素与配色出处）
```

## 三、落地位置

| 客户端位置 | 使用素材 |
| --- | --- |
| 任务栏 / 桌面 / 安装包 / 窗口 / 托盘图标 | `final/tauri-icons/*` → `src-tauri/icons/*` |
| 自定义标题栏（`App.tsx`） | `src/assets/logo-mark.png` |
| 首次启动向导头部（`OnboardingPage.tsx`） | `src/assets/logo-mark.png` |
| 迷你模式标题栏（`MiniView.tsx`） | `src/assets/logo-mark.png` |

托盘图标取 `app.default_window_icon()`，覆盖 `src-tauri/icons` 后自动跟随。
`src/assets/` 另存 `logo-horizontal.png` / `logo-vertical.png` 备用（关于页、启动页等横竖版场景）。

## 四、脚本与留档

- `extract_logo.py` —— 按颜色特征（品牌蓝青 / 纯白底）从设计稿中定位并裁出各 logo 区块，输出到 `source/crops/`，供对照与备查。
- `archive/` —— 早期自绘方案留档，**不作为交付物**：`easy/`（字母字标）、`hanzi/`（汉字「简」九套系统字体方案）、`render-hanzi.py`、`preview-hanzi.png`。探索期脚本 `scan_fonts.py` / `gen.py` 亦在此列。

## 五、覆盖客户端图标

```powershell
Copy-Item design/icon/final/tauri-icons/* src-tauri/icons/ -Force
```

覆盖前请关闭运行中的 `easy-interview.exe`（否则文件被占用）。
图标替换属客户端变更：须同步 `ai-doc/技术方案.md` 15.6 并单独提交。
