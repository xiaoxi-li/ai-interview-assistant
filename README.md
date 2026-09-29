<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="src/assets/logo-lockup-dark.png">
    <img alt="轻松面试 Easy Interview" src="src/assets/logo-lockup-light.png" width="280">
  </picture>
</p>

<h1 align="center">轻松面试 · Easy Interview</h1>

<p align="center">
  面试实时辅助桌面应用 —— 边听面试官提问，边把「问题、知识库参考与 AI 回答」放在你眼前。
</p>

<p align="center">
  <a href="https://github.com/xiaoxi-li/ai-interview-assistant/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/xiaoxi-li/ai-interview-assistant/actions/workflows/ci.yml/badge.svg"></a>
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%2010%2F11%20%7C%20macOS%2013%2B-2F81F7">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2.x-24C8DB?logo=tauri&logoColor=white">
  <img alt="React" src="https://img.shields.io/badge/React-18-61DAFB?logo=react&logoColor=white">
  <img alt="TypeScript" src="https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-1.77%2B-000000?logo=rust&logoColor=white">
</p>

<p align="center">
  <a href="#界面预览">界面预览</a> ·
  <a href="#核心特性">核心特性</a> ·
  <a href="#快速开始">快速开始</a> ·
  <a href="#一键演示">一键演示</a> ·
  <a href="#编译与打包">编译与打包</a> ·
  <a href="#键盘快捷键">快捷键</a> ·
  <a href="#已知限制">已知限制</a> ·
  <a href="#文档">文档</a>
</p>

---

## 目录

- [界面预览](#界面预览)
- [核心特性](#核心特性)
- [工作原理](#工作原理)
- [环境要求](#环境要求)
- [快速开始](#快速开始)
  - [云端 ASR（阿里云百炼，默认通路）](#云端-asr阿里云百炼默认通路)
  - [本地 ASR 原生推理（可选，当前仅 Windows）](#本地-asr-原生推理可选当前仅-windows)
  - [开箱默认配置](#开箱默认配置)
- [一键演示](#一键演示)
- [编译与打包](#编译与打包)
- [测试与校验](#测试与校验)
- [键盘快捷键](#键盘快捷键)
- [项目结构](#项目结构)
- [已知限制](#已知限制)
- [常见问题](#常见问题)
- [文档](#文档)
- [免责声明](#免责声明)

---

## 界面预览

> 以下截图由 Web 版（`npm run dev`，mock 数据）渲染，用于展示布局与信息层级。

<p align="center">
  <img alt="面试辅助" src="docs/images/interview.png" width="100%">
</p>

<p align="center"><b>面试辅助</b> —— 左列是输入链路（推荐「双通道」：系统音频 = 面试官、麦克风 = 我），右列是输出结果（问题识别 → 知识库召回 → AI 流式回答 + 引用来源）。</p>

| 知识库 | 会话历史 |
|---|---|
| <img alt="知识库" src="docs/images/knowledge.png" width="440"> | <img alt="会话历史" src="docs/images/sessions.png" width="440"> |
| Markdown 编辑 / 分栏 / 预览，中文全文检索 + 语义检索 | 时间线回放、问答与引用，可导出 Markdown / JSON |

| 迷你模式（就地切换、置顶小窗） | 设置 |
|---|---|
| <img alt="迷你模式" src="docs/images/mini-mode.png" width="440"> | <img alt="设置" src="docs/images/settings.png" width="440"> |
| 面试中按 `Ctrl/Cmd + Shift + M` 缩为浮窗，转写与回答继续流式更新 | AI 平台 / 语音识别 / 音频 / 知识库 / 界面 / 隐私，逐项即时反馈 |

---

## 核心特性

- **用自己的知识回答** —— 回答基于本地知识库召回（FTS5 中文分词关键词检索 + sqlite-vec 向量检索，RRF 融合排序），而不是让模型凭空发挥。
- **说话人区分** —— 双通道并行采集：系统音频通道 = 面试官，麦克风通道 = 我；麦克风通路以系统音频作参考做 NLMS 回声消除，避免扬声器里的声音被二次采集。
- **两条 ASR 通路，可随时切换** —— 默认走云端（阿里云百炼实时识别，低延迟、无需下载模型）；需要完全离线时切到本地（`sherpa-onnx` Paraformer 中文），音频不出本机。
- **隐私可控** —— 笔记、索引、会话与配置默认全部存在本机单一 SQLite 文件；API Key 进系统钥匙链不明文落盘；音频上传云端的开关默认开启但可随时关闭（关闭后云端转写会被拦下）。
- **一键演示，零准备跑通** —— 不需要麦克风、不需要下载模型、不需要 API Key，即可在 30 秒内跑完整条链路。
- **轻量常驻** —— 免安装 exe 约 12 MB，空闲工作集约 44 MB，可长时间挂着随时开面。
- **双平台一致** —— Windows 10/11（WebView2）与 macOS 13+ 同一套代码，CI 双平台跑同一套校验。

---

## 工作原理

```text
① 采集   系统音频（面试官） + 麦克风（我，以系统音频作 AEC 参考）
           ↓  16 kHz 单声道 / 20 ms 帧
② 转写   ASR：云端实时 WebSocket，或本地 sherpa-onnx
           ↓  partial（实时）→ final（落库）
③ 识别   问题识别：按句末标点切句 → 规则打分 → 跨段聚合与去重
           ↓  识别到新问题
④ 召回   知识库混合检索：FTS5（BM25） + 向量 → RRF 融合排序
           ↓  Top-K 片段
⑤ 回答   AI 生成（SSE 流式） →「问题 + 引用来源 + 回答」实时呈现
```

技术栈：**Tauri 2（Rust）+ React 18 + TypeScript + Vite 5 + TailwindCSS + SQLite（FTS5 + sqlite-vec）**。

---

## 环境要求

| 依赖 | 版本 | 说明 |
|---|---|---|
| Node.js | ≥ 18（推荐 20 LTS） | 前端构建与测试 |
| Rust | ≥ 1.77（stable） | Tauri 后端 |
| Windows | Windows 10/11 + **WebView2**（系统自带） | 构建 MSI/NSIS 安装包 |
| macOS | 13+ + Xcode Command Line Tools | 构建 .app / .dmg |
| libclang（仅启用本地识别时） | LLVM 18.x（`libclang.dll`） | 只有启用 `--features local-asr-native` 时才需要（bindgen 依赖） |

> Windows 下请确保 `cargo` 在 PATH 中，否则执行：
> ```powershell
> $env:PATH="$env:USERPROFILE\.cargo\bin;$env:PATH"
> ```

---

## 快速开始

```powershell
git clone https://github.com/xiaoxi-li/ai-interview-assistant.git
cd ai-interview-assistant

npm install                 # 首次安装依赖（约 1 分钟）
$env:PATH="$env:USERPROFILE\.cargo\bin;$env:PATH"

npm run tauri dev           # 启动桌面应用（首次 Rust 编译约 3–6 分钟）
```

`tauri dev` 会同时拉起 Vite 开发服务器（`http://localhost:1420`）与桌面窗口：改前端代码热更新，改 Rust 代码自动重编译。

<details>
<summary><b>只想看界面、不装 Rust？可以只跑 Web 版</b></summary>

```powershell
npm run dev        # 打开 http://localhost:1420
```

浏览器环境下后端 IPC 会自动降级为内置 mock 数据（`src/lib/ipc.ts` + `src/mocks/`），用于脱离桌面壳调布局与交互。**Web 版不产生真实转写 / 检索 / 回答**；一键演示的流式过程也需要桌面壳才能看到。

</details>

### 云端 ASR（阿里云百炼，默认通路）

云端通路**不需要下载模型**，但需要 API Key，并且会**把音频上传到服务商**（本地模式不上传）。自开箱默认值收敛后，配置只剩「填一个 Key」：

1. 「设置 → 隐私与安全」的 **「允许上传音频至云端 ASR」默认已勾选**（不勾选时点「开始转写」会被拦下并给出提示；想完全不上传请切到「本地（隐私优先）」通路）。
2. 「设置 → 语音识别」的 **「云端（低延迟）」+「阿里云智能语音」默认已选中**（讯飞 / 腾讯 / OpenAI 兼容显示「待接入」，选中后开始转写会给出可操作提示）。
3. 在「服务商凭证」填入百炼 **API Key**（存系统钥匙链，不落库）—— 这一步是唯一必需的手工配置。
4. （可选）**服务端地址（Base URL）**：默认 `wss://dashscope.aliyuncs.com/api-ws/v1/inference`；如需走代理或地域专属域名可自行填写（**每个服务商各存一份**，点「恢复默认」还原官方地址）。
5. **云端模型**默认已是 `qwen-audio-3.0-asr-flash-streaming`。同族 `qwen-audio-3.1-asr-flash-streaming`、`fun-asr-realtime`、`paraformer-realtime-v2` 亦实测可用；注意 `qwen-audio-3.0-asr-flash`（**无** `-streaming`）是非实时 HTTP 模型，填了会被服务端以 `ModelNotFound` 拒绝。
6. 点 **「连接测试」**：成功显示耗时；失败给出可读原因（未填 Key / 地址非法 / 鉴权失败 / 模型名不适用 / 网络不可达）。测试用的是**当前表单值**（含未保存的修改），不需要先保存。
7. 保存后回到面试辅助页，选「双通道」开始转写：**系统音频通道 = 面试官**、**麦克风通道 = 我**；服务端句级结果中「句未结束」显示为 partial（斜体），「句结束」落库为 final。

<details>
<summary><b>真实凭证冒烟自检（默认 <code>#[ignore]</code>）</b></summary>

```powershell
$env:DASHSCOPE_API_KEY="sk-xxx"; cargo test --lib -- --ignored real_aliyun --nocapture
```

三个用例：**握手鉴权**（建连 + `task-started`）、**音频通路**（按实时率投递音频 → `finish-task` 正常收尾）、**错误凭证**（HTTP 401 → 可操作鉴权提示）。默认用合成音频验证协议通路；如需真实语音识别，追加 `$env:ASR_SAMPLE_WAV="<16k 单声道 WAV 路径>"`；可用 `$env:ASR_MODEL=<模型名>` 覆盖联调所用模型，用于核对某模型是否被实时端点接受。

</details>

### 本地 ASR 原生推理（可选，当前仅 Windows）

默认构建**不启用**原生识别（否则所有构建机都被 libclang 约束）。启用步骤：

```powershell
# 1) 准备 libclang（sherpa-rs-sys 用 bindgen 生成 FFI 绑定）。
#    最轻量做法：只取 PyPI libclang 包里的 libclang.dll，无需安装完整 LLVM。
#    本仓库已在 .libclang/bin/libclang.dll 放好一份（LLVM 18.1.1，该目录已 gitignore）：
$env:LIBCLANG_PATH="<仓库路径>\.libclang\bin"

# 2) 开发运行
npm run tauri dev -- --features local-asr-native

# 3) 出安装包：必须额外用 --config 声明随包携带的原生 DLL，
#    否则安装版缺少 DLL 会在启动阶段直接失败（默认构建不受影响，CI 安全）
npm run tauri build -- --features local-asr-native --config src-tauri/tauri.local-asr.conf.json
```

- 首次启用构建时，`sherpa-rs-sys` 会按 target 下载**约 23 MB** 预编译原生库（SHA256 校验后缓存到 `%LOCALAPPDATA%\sherpa-rs\`），之后构建不再下载。
- 构建产物由 `sherpa-rs-sys` 的 `build.rs` 把 DLL 复制到 `target/<profile>/`（与 exe 同目录）；**安装包**则由 `tauri.local-asr.conf.json` 的 `bundle.resources` 携带。
- 启用后：在「设置 → 模型管理」下载「本地 ASR（Paraformer 中文）」，回到面试辅助页开始转写即可得到真实中文转写。
- 未启用时相关命令会返回**明确的构建指引**错误（不再是一句含糊的「尚未接入」）；就绪状态在**「设置 → 语音识别」（本地模式）**的「本地引擎」处查看。
- 冒烟自检（需已下载模型）：`cargo test --features local-asr-native -- --ignored asr::sherpa`。
- macOS 通路见 [`ai-doc/需求分析.md`](ai-doc/需求分析.md) 待办 TODO-01。

### 开箱默认配置

首次安装（该项无持久化配置）时的默认值收敛为「一条开箱即可用的通路」；**已有配置不会被覆盖**，且默认值只在后端配置对象的 `Default` 里定义（`config/{mod,asr,privacy}.rs`），设置页不另存一份：

| 分区 | 默认值 |
|---|---|
| AI 平台 | **通义千问 Qwen** + 模型 `qwen3.8-flash`（「回答模式」仍默认**非思考**，首字更快） |
| 语音识别 | **云端（低延迟）** + **阿里云智能语音** + 模型 `qwen-audio-3.0-asr-flash-streaming` |
| 隐私与安全 | **允许上传音频至云端 ASR 已勾选** |

选 Qwen + 云端阿里云的动因：两者都走阿里云百炼（同一把 DashScope API Key），首次配置只需填**一个** Key。想完全不联网转写，切到「本地（隐私优先）」并下载本地模型即可。

---

## 一键演示

**主验收路径**：不需要麦克风、不需要下载模型、不需要 API Key。

1. 启动应用：`npm run tauri dev`（或用打好的安装包 / exe）。
2. **首次启动向导**（4 步）：隐私与权限说明 → ASR 通路与模型准备 → AI 平台配置 → 完成。验收时可直接「下一步」跳过。
3. 左侧导航进入 **「面试辅助」**。
4. 左上卡片 **「一键演示 · 完整链路」** → 点 **「开始演示」**。
5. 观察约 30 秒（1x 速度，3 轮问答）。
6. （可选）演示中点标题栏 **「缩小」** 或按 `Ctrl+Shift+M` 进入迷你模式，确认界面就地切换为迷你页、窗口缩为小窗并置顶，且小窗内同样是**逐字流式**输出。

### 验收清单

| # | 预期现象 | 所在面板 |
|---|---|---|
| 1 | 演示自动写入 **4 篇示例笔记**（架构设计 / 性能优化 / 系统设计 / 团队协作），无重复写入 | 知识库 |
| 2 | 面试官与我**双色气泡**交替出现 | 实时转写 |
| 3 | 每条发言先出现**斜体 partial**，随后被 final 覆盖并落库 | 实时转写 |
| 4 | 3 条面试官提问均被识别，显示**置信度徽标 + 命中信号 chips** | 问题识别 |
| 5 | 回答按**打字机效果**逐字流出，标题含模型名（`offline-demo` 或你的模型名） | AI 回答 |
| 6 | 回答下方显示**引用来源**（召回片段的标题 + 摘要 + 融合得分），点击可跳转到知识库并定位该笔记 | AI 回答 |
| 7 | 演示结束（约 30 秒）后自动收尾，卡片重新出现 | 全局 |
| 8 | **「会话历史」**中可看到「演示会话 · 全链路」：时间线回放、问答与引用、**导出 Markdown / JSON** | 会话历史 |
| 9 | 点标题栏**「缩小」**（或 `Ctrl+Shift+M`）：界面**就地切换**为迷你页，窗口同时缩为小尺寸并置顶；识别到新问题后**无需回到完整界面**即自动召回并逐字输出回答；点**「恢复完整界面」**还原界面与窗口几何 | 全局 |

### 演示模式的真实性边界

演示**只替换输入源，不替换业务逻辑**：

| 环节 | 演示中 | 说明 |
|---|---|---|
| 音频 → 文本 | **脚本提供** | 真实转写需接入 ASR 通路 |
| 问题识别 | **真实** | 生产 `QuestionDetector`（按句规则打分 + 跨段聚合 + 去重） |
| 知识库召回 | **真实** | 生产混合检索（FTS5 中文分词 + 向量 + RRF 融合），真实写入 4 篇笔记 |
| 回答生成 | 真实或离线 | 配了 AI Key → 真实 SSE 流式；未配 → `offline_answer` 拼装，并**显式标注「离线演示回答」** |
| 落库 / 事件 / 会话回放 | **真实** | 与正式链路同一套仓储与事件契约 |

因此：**演示跑通 = 除「音频转文本」外的整条链路端到端验证。**

---

## 编译与打包

```powershell
$env:PATH="$env:USERPROFILE\.cargo\bin;$env:PATH"
npm run tauri build
```

产物位置（下表体积为 0.1.0 实际编译产出的**参考值**，随版本变化）：

| 平台 | 路径 | 大小 |
|---|---|---|
| Windows 免安装 exe | `src-tauri/target/release/easy-interview.exe` | ≈ 11.7 MB |
| Windows 安装包（NSIS） | `src-tauri/target/release/bundle/nsis/轻松面试_0.1.0_x64-setup.exe` | ≈ 5.4 MB |
| Windows 安装包（MSI） | `src-tauri/target/release/bundle/msi/轻松面试_0.1.0_x64_zh-CN.msi` | ≈ 6.4 MB |
| macOS | `src-tauri/target/release/bundle/dmg/*.dmg`、`.../macos/*.app` | — |

按需单独打包（更快）：

```powershell
npm run tauri build -- --no-bundle            # 只出免安装 exe
npm run tauri build -- --bundles nsis         # 只出 NSIS 安装包
npm run tauri build -- --bundles msi          # 只出 MSI 安装包
```

> - 前端产物会内嵌进二进制，因此**单个 exe 即可直接运行**，分发时无需附带 `dist/`。
> - MSI 使用 `zh-CN` 语言（已在 `src-tauri/tauri.conf.json` 配置）：产品名为中文时，沿用默认 `en-US` 会导致 WiX `light.exe` 打包失败。
> - ⚠️ 打包前先关掉正在运行的实例，否则 release 产物被占用会导致打包失败。

<details>
<summary><b>安装包手工验签清单</b></summary>

1. 双击安装包完成安装（或直接运行免安装 exe）。
2. 启动后完成首次向导；确认无边框自定义标题栏可拖拽、最小化/最大化/关闭正常。
3. 复跑「一键演示」验收清单 1–9。
4. 「设置」中确认：主题切换（深色 / 浅色 / 跟随系统，点选即时预览、点「保存配置」持久化）、语言切换（中/英）、AI 平台配置、模型管理、隐私设置等页面均可正常打开保存。

</details>

---

## 测试与校验

```powershell
npm run verify            # 前端：类型检查 + 构建 + vitest
cd src-tauri; cargo test  # 后端：单测 + 集成测试
```

`npm run verify` 等价于 `npm run build && npm test`。当前基线：**前端 356 项（36 个文件）、后端 446 项通过**（含 PCM 回放端到端、AEC、双通道、演示链路、命令线程断言）。CI 在 Windows / macOS 双平台跑同一套校验（[`.github/workflows/ci.yml`](.github/workflows/ci.yml)），打 `v*` tag 时由 [`release.yml`](.github/workflows/release.yml) 双平台出包并创建草稿 Release。

> **命令执行模型（NFR-17）**：Tauri 中不带 `(async)` 的 `#[tauri::command]` 会在**界面主线程**同步执行，曾导致四个主页面互切时「点击后停顿再跳转」。现全部命令已异步化（仅 5 个窗口 / 全局快捷键相关命令保留同步），并由 `commands::command_thread_tests` 静态断言防回退：**新增命令请一律使用 `#[tauri::command(async)]`**，重阻塞逻辑（数据库、文件、设备枚举、钥匙链）投递到 `tauri::async_runtime::spawn_blocking`。
> 开发期切页耗时可看控制台 `[perf] 切页 A → B：xx.xms`（`src/lib/perf.ts`，生产构建不输出）。

---

## 键盘快捷键

| 快捷键 | 作用 |
|---|---|
| `Ctrl/Cmd + Shift + A` | 一键提问（立即识别当前问题并生成回答） |
| `Ctrl/Cmd + Shift + P` | 暂停 / 继续采集（暂停期间丢弃音频帧，会话与记录保留） |
| `Ctrl/Cmd + Shift + R` | 重新生成回答 |
| `Ctrl/Cmd + Shift + C` | 复制当前回答 |
| `Ctrl/Cmd + Shift + M` | 进入 / 退出迷你模式 |

快捷键可在面试页快捷条或「设置 → 界面」中调整；全局快捷键（应用失焦也生效）同样在「设置 → 界面」注册。

---

## 项目结构

```text
src/                    前端（React）
  components/interview/ 面试面板：转写 / 问题 / 回答 / 快捷条 / 演示卡 / 迷你页
  pages/                知识库 / 面试辅助 / 会话历史 / 设置 / 首次向导
  stores/               zustand 状态（含 IPC 降级 mock）
  hooks/                事件订阅（转写 / 问题 / 回答）、自动召回与生成、快捷键与主题同步
  mocks/                浏览器调试用的 IPC mock（`npm run dev` 时启用）
src-tauri/src/          后端（Rust）
  asr/                  ASR（通道抽象 / 本地引擎 + VAD 流水线 / 双通道 / 落库；cloud/ = 云端实时 WS）
  audio/                采集（cpal 麦克风 + WASAPI 端点回环）/ VAD / AEC / 设备 / WAV
  interview/            问题识别 / 知识库召回 / 回答生成 / 会话回放
  retrieval/            分块 / 中文分词 / FTS5 / 向量 / RRF 融合
  demo.rs               一键演示脚本与离线链路自检
  tray.rs hotkeys.rs    系统托盘 / 全局快捷键
ai-doc/                 需求分析与技术方案文档
docs/images/            README 配图
```

---

## 已知限制

1. **本地 ASR 原生推理（Windows）已接入并验证，但默认未启用** —— 由 cargo 特性 `local-asr-native` 控制，启用见上文（构建机需 libclang）。未启用时返回**明确的构建指引错误**，不静默降级、不伪造文本；macOS 通路待接入（TODO-01）。
2. **云端 ASR 目前仅接入阿里云百炼** —— 讯飞开放平台 / 腾讯云语音识别 / OpenAI 兼容（自定义）的配置项、凭证与 Base URL 已保留，但协议适配未实现；选中它们开始转写会得到**可操作提示**而非静默失败（TODO-10 / TODO-11）。
3. **Embedding 走本地确定性 Mock** —— 语义召回可用但质量有限；向量通道不可用时会退化为纯关键词检索，不影响使用。选型已定：本地 `BAAI/bge-m3`（ONNX，1024 维，约 2.27 GB），下载源固定为魔搭社区（TODO-07）。
4. **隐身模式开关**目前只出现在面试页快捷条，尚未同步到「设置 → 界面」表单（TODO-09）。
5. **自动更新**已实现「检查 + 提示 + 下载」，安装需由发布流水线对产物签名后生效。
6. **系统音频（面试官）采集存在平台差异** —— Windows 直接走 **WASAPI 端点级回环**，在**播放设备**上采集，**无需启用「立体声混音」、无需安装虚拟声卡**；macOS 仍需安装 BlackHole / Soundflower 等虚拟回环设备（TODO-06）。
7. **采集面板**「开始采集」若未选择具体设备，仅**系统音频**来源可回退到系统默认播放设备；**麦克风**来源需在下拉中选中具体设备（TODO-04）。
8. 选「思考」模式时思考阶段暂无输出；问题识别对含疑问词的陈述句仍可能误报。

---

## 常见问题

<details>
<summary><b>构建与运行</b></summary>

| 现象 | 处理 |
|---|---|
| `npm run tauri dev` 报 `cargo not found` | 把 `%USERPROFILE%\.cargo\bin` 加入 PATH |
| 首次编译很慢 / 卡在 `Compiling` | 正常，Rust 首次全量编译 3–6 分钟 |
| 「设置 → 语音识别」（本地模式）显示「本地引擎：未启用」，或开始转写时报「未启用本地 ASR 原生推理」 | 默认构建不含原生识别；按上文以 `--features local-asr-native` 构建（需 libclang） |
| 报「加载本地 ASR 模型失败」 | 模型文件损坏或下载不完整；在「设置 → 模型管理」删除后重新下载 |

</details>

<details>
<summary><b>云端 ASR 连接测试失败</b></summary>

| 现象 | 处理 |
|---|---|
| 报「需要上传音频」 | 该开关默认已开启，此提示说明曾手动取消：到「设置 → 隐私与安全」重新勾选，或切回「本地（隐私优先）」（本地模式不上传音频） |
| 报「XX 的云端 ASR 待接入」 | 当前仅「阿里云智能语音」已接入；切换服务商或改用本地模式 |
| 报 **HTTP 404** | 地址填成了 **HTTP 接口**。百炼的 `https://dashscope.aliyuncs.com/compatible-mode/v1` 是 OpenAI 兼容的 HTTP 接口（供对话/嵌入/录音文件转写），**实时转写必须用 WebSocket**：`wss://dashscope.aliyuncs.com/api-ws/v1/inference`。点「恢复默认」或手填后**保存配置**再测 |
| 报 **ModelNotFound** | 模型名不能用于实时转写：非实时（HTTP 文件转写）模型（如 `qwen-audio-3.0-asr-flash`，无 `-streaming`）只能走 HTTP 接口。改用实测可用的实时模型：`qwen-audio-3.0-asr-flash-streaming`（默认）、`qwen-audio-3.1-asr-flash-streaming`、`fun-asr-realtime`、`paraformer-realtime-v2` |
| 连接测试失败（其他） | 按提示依次检查：凭证是否已保存、Base URL 是否可达（代理/私有化部署需自行填写）、网络是否可用 |

</details>

<details>
<summary><b>识别不到问题 / 不生成回答</b></summary>

| 现象 | 处理 |
|---|---|
| 转写面板里明明有问题句，但「问题识别」一直为空、AI 回答也不生成 | ① 只有「面试官」通道的转写参与识别：单通道模式下请选「面试官（系统音频）」，双通道模式下确认面试官的话标为「面试官」而不是「我」；② 判定单位是**句**（已按句末标点切句打分），但**纯陈述句、疑问特征过弱或与上一次问题高度重复**的句子不会上报；③ 可先点「手动识别当前问题」用最近 30 秒转写兜底；④ 排查日志 `<数据目录>/logs/app.log.<日期>`：命中会写 `识别到面试问题（字数/得分/来源）` |
| 召回没有结果 | 确认演示已执行（会自动写入 4 篇示例笔记），或先在「知识库」导入 Markdown |
| 演示时没有 AI 回答内容 | 未配 Key 时会用离线回答；若报错请检查「设置 → AI 平台」的 Base URL 与 Key |
| 报「未检测到可用的系统音频来源」 | Windows：确认系统至少有一个可用播放设备，并在「设置 → 音频设备」点「刷新设备」；macOS：安装 BlackHole 等虚拟回环设备后重启应用 |
| 采集面板点「开始采集」报「未指定音频设备」 | 麦克风来源需在下拉中选中具体设备；系统音频来源可直接用系统默认播放设备 |

</details>

<details>
<summary><b>界面与数据</b></summary>

| 现象 | 处理 |
|---|---|
| 选了「浅色」切到别的页面后界面又变回原来的主题 | 主题是**即时预览**：点选立刻生效但**不落库**，需在该分区点「保存配置」才持久化；未保存就离开设置页会自动回滚（避免误改） |
| 想让界面跟随系统深浅色 | 「设置 → 界面与快捷键 → 主题」选「跟随系统」，之后系统切换时界面实时变化，重启仍保持；系统主题不可用时回退深色 |
| 想重置全部数据重来一遍 | 关闭应用后删除数据目录：Windows `%APPDATA%\com.easyinterview.app\`，macOS `~/Library/Application Support/com.easyinterview.app\`（内含 `easy-interview.db` 与 `models/`），下次启动会重新初始化并再次进入首次向导。API Key 存在系统钥匙链中，如需一并清除请在「设置 → AI 平台」中删除 |

</details>

---

## 文档

| 文档 | 内容 |
|---|---|
| [`ai-doc/需求分析.md`](ai-doc/需求分析.md) | 场景与用例、功能需求（KB/IA/ST/SYS）、非功能需求、验收标准、待办清单、需求追溯矩阵 |
| [`ai-doc/技术方案.md`](ai-doc/技术方案.md) | 技术选型、系统架构、核心模块设计、数据模型、关键技术决策与风险 |
| [`docs/images/`](docs/images) | README 配图（由 Web 版 mock 数据渲染，非真实会话内容） |

---

## 免责声明

本项目用于**个人面试准备与练习**。请在参加面试前自行确认并遵守面试平台的服务条款、招聘方的要求以及所在地区的法律法规；是否使用辅助工具以及由此产生的后果由使用者自行承担。
