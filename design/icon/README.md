# 轻松面试 · 品牌标识与应用图标

> 以设计方品牌稿与用户提供的高清 logo 为准，**原样使用：不重绘、不改色、不改几何**。

## 一、品牌要素

| 项 | 内容 |
| --- | --- |
| 品牌名 | **Easy** · 轻松面试 |
| Slogan | 让面试，更轻松 |
| 主图形 | 三条**向右上倾斜约 15°** 的圆角横杠（字母 E 意象；线条流动象征语音转写与信息流转） |
| 主色 | `#3B82F6` / 辅助色 `#22D3A6` / 文字色 `#0F172A` |
| 造型风格 | 全圆润、渐变色彩、简洁现代 |

> 界面设计令牌仍是 `#4F6BED` 系（见《技术方案》15.1–15.3），品牌 logo 使用上表色板。

## 二、现行素材

```
source/user/mark-hd.png          用户提供的应用图标源（1080²，含水印原图）
source/user/lockup-hd.png        用户提供的横版组合源（1080²，含水印原图）
final/user/mark-app-1024.png     去水印后的应用图标（白底方形）
final/user/lockup-light.png      横版标识 · 浅色主题（深色文字）
final/user/lockup-dark.png       横版标识 · 深色主题（白色文字）
final/rebuilt/app-icon-1024.png  按设计稿复刻的应用图标（备用）
final/rebuilt/mark-1024.png      按设计稿复刻的透明底标记（备用）
source/brand-sheet.png           品牌设计稿原图 1080×720（要素与配色出处）
source/crops/                    设计稿中定位出的标记（app_icon / hero_mark），复刻脚本的输入
```

## 三、落地位置

| 客户端位置 | 使用素材 |
| --- | --- |
| 任务栏 / 桌面 / 安装包 / 窗口 / 托盘图标 | `final/user/mark-app-1024.png` → `src-tauri/icons/*` |
| 自定义标题栏、向导页头部、迷你模式标题栏 | `src/assets/logo-lockup-{light,dark}.png`，以 Tailwind `dark:` 变体随主题切换 |

托盘图标取 `app.default_window_icon()`，覆盖 `src-tauri/icons` 后自动跟随。

## 四、脚本（复现流程）

```powershell
py -3 design/icon/process_user_logo.py   # 去水印 + 裁切 + 拆标记/文字 + 双主题着色（主流程）
py -3 design/icon/rebuild_logo.py        # 从设计稿像素 1:1 复刻（上采样 + 边缘锐化 + 去白底）
py -3 design/icon/extract_logo.py        # 从设计稿按颜色特征定位各 logo 区块
py -3 design/icon/export_icons.py        # 导出 15 尺寸 + icon.ico + icon.icns
```

## 五、处理约定（踩过的坑）

- **去水印**：水印固定在右下角，该区域是纯背景，直接抹除填白，不触碰其他像素。
- **拆分标记与文字按位置而非亮度**：标记本身含深蓝，用亮度阈值会把深蓝误判成文字；改为自动查找中间那条空白列分界。
- **抠底用饱和度**：`alpha = clamp((sat-12)*6)`，用 `255-min` 会残留一圈白边。
- **低分辨率位图放大**：先 GaussianBlur(0.8) 让边缘连续，再 LANCZOS 上采样 + LUT 阈值锐化（K=2.2），K 过大出阶梯、过小发虚。
