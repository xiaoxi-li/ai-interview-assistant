# 轻松面试（Easy Interview）

面试实时辅助桌面应用：边听面试官提问，边把问题、知识库参考与 AI 回答放在你眼前。

- **技术栈**：Tauri 2（Rust）+ React 18 + TypeScript + Vite + TailwindCSS + SQLite（FTS5 + sqlite-vec）
- **隐私优先**：默认走本地 ASR 通路（音频不出本机）；云端 ASR 已接入阿里云百炼（P1），需显式同意上传音频
- **本地知识库**：Markdown 导入 → 中文分词全文检索 + 向量检索 → RRF 融合召回

---

## 一、环境要求

| 依赖 | 版本 | 说明 |
|---|---|---|
| Node.js | ≥ 18（推荐 20 LTS） | 前端构建与测试 |
| Rust | ≥ 1.77（stable） | Tauri 后端 |
| Windows | Windows 10/11 + **WebView2**（系统自带） | 构建 MSI/NSIS 安装包 |
| macOS | 12+ + Xcode Command Line Tools | 构建 .app / .dmg |
| libclang（仅可选启用本地识别时） | LLVM 18.x（`libclang.dll`） | 只有启用 `--features local-asr-native` 时才需要（bindgen 依赖），见 2.1 |

> Windows 下请确保 `cargo` 在 PATH 中，否则执行：
> ```powershell
> $env:PATH="$env:USERPROFILE\.cargo\bin;$env:PATH"
> ```

---

## 二、快速开始（开发运行）

```powershell
cd d:\program\ai-interview-assistant

npm install                 # 首次安装依赖（约 1 分钟）
$env:PATH="$env:USERPROFILE\.cargo\bin;$env:PATH"

npm run tauri dev           # 启动桌面应用（首次 Rust 编译约 3–6 分钟）
```

`tauri dev` 会同时拉起 Vite 开发服务器（`http://localhost:1420`）与桌面窗口，改前端代码热更新，改 Rust 代码自动重编译。

> **只想看界面、不装 Rust？** 可以只跑 Web 版（后端 IPC 自动降级为内置 mock 数据）：
> ```powershell
> npm run dev        # 打开 http://localhost:1420
> ```
> Web 版仅用于看布局与交互，不产生真实转写/检索/回答。

### 2.1 启用本地 ASR 原生推理（可选，当前仅 Windows）

默认构建**不启用**原生识别（否则所有构建机都被 libclang 约束）。启用步骤：

```powershell
# 1) 准备 libclang（sherpa-rs-sys 用 bindgen 生成 FFI 绑定）。
#    最轻量做法：只取 PyPI libclang 包里的 libclang.dll，无需安装完整 LLVM。
#    本仓库已在 .libclang/bin/libclang.dll 放好一份（LLVM 18.1.1，该目录已 gitignore）：
$env:LIBCLANG_PATH="d:\program\ai-interview-assistant\.libclang\bin"

# 2) 开发运行
npm run tauri dev -- --features local-asr-native

# 3) 出安装包：必须额外用 --config 声明随包携带的原生 DLL，
#    否则安装版缺少 DLL 会在启动阶段直接失败（默认构建不受影响，CI 安全）
npm run tauri build -- --features local-asr-native --config src-tauri/tauri.local-asr.conf.json
```

- 首次启用构建时，`sherpa-rs-sys` 会按 target 下载**约 23MB** 预编译原生库（SHA256 校验后缓存到 `%LOCALAPPDATA%\sherpa-rs\`），之后构建不再下载。
- 构建产物由 `sherpa-rs-sys` 的 build.rs 把 DLL 复制到 `target/<profile>/`（与 exe 同目录）；**安装包**则由 `tauri.local-asr.conf.json` 的 `bundle.resources` 携带。
- 启用后：在「设置 → 模型管理」下载「本地 ASR（Paraformer 中文）」，再回到面试辅助页开始转写，即可得到真实中文转写。
- 未启用时相关命令会返回**明确的构建指引**错误（不再是一句含糊的「尚未接入」）。
- 冒烟自检（需已下载模型）：`cargo test --features local-asr-native -- --ignored asr::sherpa`（会加载真实模型并解码 1 秒静音）。
- macOS 通路见 `ai-doc/需求分析.md` 5.5 待办 TODO-01。

### 2.2 云端 ASR（阿里云百炼，可选，IA-16）

云端通路**不需要下载模型**，但需要 API Key，并且会**把音频上传到服务商**（本地模式不上传）。

1. 「设置 → 隐私」开启 **「音频上传」**（未开启时点「开始转写」会被拦下并给出提示）。
2. 「设置 → 语音识别」把模式切到 **「云端（低延迟）」**，服务商选 **「阿里云智能语音」**（已接入；讯飞 / 腾讯 / OpenAI 兼容显示「待接入」，选中后开始转写会给出可操作提示）。
3. 在「服务商凭证」填入百炼 **API Key**（存系统钥匙链，不落库）。
4. （可选）**服务端地址（Base URL）**：默认 `wss://dashscope.aliyuncs.com/api-ws/v1/inference`；如需走代理或地域专属域名可自行填写（**每个服务商各存一份**，点「恢复默认」还原官方地址）。
5. **云端模型**默认已是官方推荐的实时模型 `qwen-audio-3.1-asr-flash-streaming`（实测可用；备选 `fun-asr-realtime`、`paraformer-realtime-v2`）。注意 **`qwen-audio-3.1-asr-flash`（无 `-streaming`）是非实时 HTTP 模型**，不能用于实时转写，填了会被服务端以 `ModelNotFound` 拒绝。
6. 点 **「连接测试」**：成功会显示耗时；失败给出可读原因（未填 Key / 地址非法 / 鉴权失败 / 模型名不适用 / 网络不可达）。测试用的是**当前表单值**（含未保存的修改），不需要先点保存。
6. 保存配置后回到面试辅助页，选「双通道」开始转写：**系统音频通道 = 面试官**、**麦克风通道 = 我**，服务端句级结果中「句未结束」显示为 partial（斜体），「句结束」落库为 final。

- 冒烟自检（需要真实凭证，默认 `#[ignore]`）：
  ```powershell
  $env:DASHSCOPE_API_KEY="sk-xxx"; cargo test --lib -- --ignored real_aliyun --nocapture
  ```
  三个用例：**握手鉴权**（建连 + `task-started`）、**音频通路**（按实时率投递音频 → `finish-task` 正常收尾）、**错误凭证**（HTTP 401 → 可操作鉴权提示）。默认用合成音频验证协议通路；如需真实语音识别，追加 `$env:ASR_SAMPLE_WAV="<16k 单声道 WAV 路径>"`（该用例会断言识别出非空文本）；可用 `$env:ASR_MODEL=<模型名>` 覆盖联调所用模型，用于核对某模型是否被实时端点接受。
- **已验证（2026-09-28）**：真实凭证握手与音频通路均通过，无效 Key 报 `鉴权失败（HTTP 401）…`；合成音频不产出文本属预期，真实中文语音文本待用麦克风/WAV 在 GUI 复核。
- 协议与实现要点见 `ai-doc/技术方案.md` 6.3.2；讯飞 / 腾讯 / OpenAI 兼容通路见 `ai-doc/需求分析.md` 5.5 待办 TODO-10 / TODO-11。

---

## 三、验收步骤（一键演示：零准备跑通全链路）

这是**主验收路径**：不需要麦克风、不需要下载模型、不需要 API Key。

### 3.1 操作步骤

1. 启动应用：`npm run tauri dev`（或用第四节打好的安装包 / exe）。
2. **首次启动向导**（4 步）：隐私与权限说明 → ASR 通路与模型准备 → AI 平台配置 → 完成。
   验收时可直接「下一步」跳过（不配 Key 也能跑演示）。
3. 左侧导航进入 **「面试辅助」**（页面分两列：**左列**为演示卡 / 音频采集 / 实时转写，**右列**为快捷操作 / 问题识别 / AI 回答）。
4. 左上卡片 **「一键演示 · 完整链路」** → 点 **「开始演示」**。
5. 观察约 30 秒（1x 速度，3 轮问答）。
6. （可选）演示过程中点标题栏 **「缩小」** 或按 `Ctrl+Shift+M` 进入迷你模式，确认界面就地切换为迷你页、窗口缩为小窗并置顶；点 **「恢复完整界面」** 回到原界面与窗口状态。

### 3.2 验收清单（逐项确认）

| # | 预期现象 | 所在面板 |
|---|---|---|
| 1 | 演示自动写入 **4 篇示例笔记**（架构设计 / 性能优化 / 系统设计 / 团队协作），无重复写入 | 知识库 |
| 2 | 面试官与我**双色气泡**交替出现 | 实时转写 |
| 3 | 每条发言先出现**斜体 partial**（模拟流式中间结果），随后被 final 覆盖并落库 | 实时转写 |
| 4 | 3 条面试官提问均被识别，显示**置信度徽标 + 命中信号 chips**（如「以问号结尾」「含疑问词「怎么」」） | 问题识别 |
| 5 | 每个问题下方自动出现**知识库参考**（标题 + 摘要 + 融合得分），共 3 组 | 问题识别 |
| 6 | 回答按**打字机效果**逐字流出，标题含模型名（`offline-demo` 或你的模型名） | AI 回答 |
| 7 | 回答下方显示**引用来源**，点击可跳转到知识库并定位该笔记 | AI 回答 |
| 8 | 演示结束（约 30 秒）后自动收尾，卡片重新出现 | 全局 |
| 9 | **「会话历史」**中可看到「演示会话 · 全链路」：时间线回放、问答与引用、**导出 Markdown / JSON** | 会话历史 |
| 10 | 点标题栏**「缩小」**（或 `Ctrl+Shift+M`）：整个界面**就地切换**为迷你页（仅面试官问题 + AI 回答），窗口同时缩为小尺寸并置顶；点**「恢复完整界面」**还原完整界面与窗口尺寸/位置/置顶状态，点**「退出应用」**关闭应用 | 全局 |

### 3.3 演示模式的真实性边界（请务必了解）

演示**只替换输入源，不替换业务逻辑**：

| 环节 | 演示中 | 说明 |
|---|---|---|
| 音频 → 文本 | **脚本提供** | 真实麦克风转写需 native ASR 推理接入，见第七节 |
| 问题识别 | **真实** | 生产 `QuestionDetector`（规则打分 + 跨段聚合 + 去重） |
| 知识库召回 | **真实** | 生产混合检索（FTS5 中文分词 + 向量 + RRF 融合），真实写入 4 篇笔记 |
| 回答生成 | 真实或离线 | 配了 AI Key → 真实 SSE 流式；未配 → `offline_answer` 拼装，并**显式标注「离线演示回答」** |
| 落库 / 事件 / 会话回放 | **真实** | 与正式链路同一套仓储与事件契约 |

因此：演示跑通 = 除「音频转文本」外的整条链路端到端验证。

---

## 四、编译与打包（可分发产物）

```powershell
cd d:\program\ai-interview-assistant
$env:PATH="$env:USERPROFILE\.cargo\bin;$env:PATH"

npm run tauri build
```

产物位置（0.1.0 已实际编译产出）：

| 平台 | 路径 | 大小 |
|---|---|---|
| Windows 免安装 exe | `src-tauri/target/release/easy-interview.exe` | 10.8 MB |
| Windows 安装包（NSIS） | `src-tauri/target/release/bundle/nsis/轻松面试_0.1.0_x64-setup.exe` | 5.4 MB |
| Windows 安装包（MSI） | `src-tauri/target/release/bundle/msi/轻松面试_0.1.0_x64_zh-CN.msi` | 6.4 MB |
| macOS | `src-tauri/target/release/bundle/dmg/*.dmg`、`.../macos/*.app` | — |

按需单独打包（更快）：

```powershell
npm run tauri build -- --no-bundle            # 只出免安装 exe
npm run tauri build -- --bundles nsis         # 只出 NSIS 安装包
npm run tauri build -- --bundles msi          # 只出 MSI 安装包
```

> 前端产物会内嵌进二进制，因此**单个 exe 即可直接运行**，分发时无需附带 `dist/`。
> MSI 使用 `zh-CN` 语言（已在 `src-tauri/tauri.conf.json` 配置）：产品名为中文时，
> 若沿用默认 `en-US` 会导致 WiX `light.exe` 打包失败。

### 手工验签清单（安装包）

1. 双击安装包完成安装（或直接运行免安装 exe）。
2. 启动后完成首次向导；确认窗口无边框自定义标题栏可拖拽、最小化/最大化/关闭正常。
3. 复跑第三节验收清单 1–9。
4. 「设置」中确认：主题切换（深色 / 浅色 / **跟随系统**，点选即时预览、点「保存配置」持久化）、语言切换（中/英）、AI 平台配置、模型管理、隐私设置等页面可正常打开保存。

---

## 五、本地校验（提交前自检）

```powershell
npm run verify            # 前端：tsc 构建 + vitest（344 项）
cd src-tauri; cargo test  # 后端：445 项（含 PCM 回放端到端、AEC、双通道、演示链路、命令线程断言）
```

`npm run verify` 等价于 `npm run build && npm test`。CI 会在 Windows / macOS 双平台跑同一套校验（`.github/workflows/`）。

> **命令执行模型（NFR-17，2026-09-29）**：Tauri 中不带 `(async)` 的 `#[tauri::command]` 会在**界面主线程**同步执行，曾导致四个主页面互切时「点击后停顿再跳转」。现全部命令已异步化（仅 5 个窗口 / 全局快捷键相关命令保留同步，见 `ai-doc/技术方案.md` 6.15），并由 `commands::command_thread_tests` 静态断言防回退：**新增命令请一律使用 `#[tauri::command(async)]`**，重阻塞逻辑（数据库、文件、设备枚举、钥匙链）投递到 `tauri::async_runtime::spawn_blocking`。
> 开发期切页耗时可看控制台 `[perf] 切页 A → B：xx.xms`（`src/lib/perf.ts`，生产构建不输出）。

---

## 六、键盘快捷键（面试中快捷操作）

| 快捷键 | 作用 |
|---|---|
| `Ctrl/Cmd + Shift + A` | 一键提问（立即识别当前问题并生成回答） |
| `Ctrl/Cmd + Shift + P` | 暂停 / 继续采集（暂停期间丢弃音频帧，会话与记录保留） |
| `Ctrl/Cmd + Shift + R` | 重新生成回答 |
| `Ctrl/Cmd + Shift + C` | 复制当前回答 |

快捷键可在面试页快捷条或「设置 → 界面」中调整；全局快捷键（应用失焦也生效）在「设置 → 界面」注册。

---

## 七、当前已知限制

1. **本地 ASR 原生推理（IA-15）Windows 通路已接入并验证，但默认未启用**：由 cargo 特性 `local-asr-native` 控制，启用见 2.1（构建机需 libclang）。
   - 未启用时 `SherpaOnnxEngine::transcribe_segment` 返回**明确的构建指引错误**，不静默降级、不伪造文本。
   - 已验证：真实模型可加载并完成解码（冒烟自检见 2.1）；带特性的安装包已随包携带原生 DLL（MSI 11.95MB / NSIS 9.32MB，未启用特性时为 6.40MB / 5.39MB）。
   - macOS 通路待接入，见 `ai-doc/需求分析.md` 5.5 待办 TODO-01。
2. **云端 ASR 仅接入阿里云百炼（IA-16）**：讯飞开放平台 / 腾讯云语音识别 / OpenAI 兼容（自定义）的配置项、凭证与 **Base URL** 已完整保留，但协议适配未实现（`CloudAsrProvider::implemented()` 仅阿里为 `true`）；选中它们开始转写会得到**可操作提示**而非静默失败。详见 `ai-doc/需求分析.md` 待办 TODO-10 / TODO-11。
3. **Embedding 走本地确定性 Mock**：语义召回可用但质量有限；接入本地 ONNX 模型后自动提升（向量通道不可用时会退化为纯关键词检索，不影响使用）。**选型已定**：本地 `BAAI/bge-m3`（ONNX，1024 维，约 2.27GB），下载源固定为魔搭社区，落地清单见 `ai-doc/技术方案.md` 6.5。
4. **隐身模式开关**目前只出现在面试页快捷条，尚未同步到「设置 → 界面」表单。
5. 自动更新已实现「检查 + 提示 + 下载」，安装需由发布流水线对产物签名后生效。
6. **系统音频（面试官）采集的平台差异（IA-14）**：Windows 直接走 **WASAPI 端点级回环**，在**播放设备**上采集，**无需启用「立体声混音」、无需安装虚拟声卡**（采集面板与设置页的「系统音频」下拉即播放设备列表）；macOS 仍需安装 BlackHole / Soundflower 等虚拟回环设备（`cpal` 尚未提供 CoreAudio 回环能力）。
7. 采集面板「开始采集」若未选择具体设备，仅**系统音频**来源可回退到系统默认播放设备；**麦克风**来源需在下拉中选中具体设备，否则报「未指定音频设备」。

---

## 八、常见问题

| 现象 | 处理 |
|---|---|
| `npm run tauri dev` 报 `cargo not found` | 把 `%USERPROFILE%\.cargo\bin` 加入 PATH（见第一节） |
| 首次编译很慢 / 卡在 `Compiling` | 正常，Rust 首次全量编译 3–6 分钟 |
| 演示时没有 AI 回答内容 | 未配 Key 时会用离线回答；若报错请检查「设置 → AI 平台」的 Base URL 与 Key |
| 提示「未启用本地 ASR 原生推理」 | 默认构建不含原生识别；按第二节 2.1 以 `--features local-asr-native` 构建（需 libclang） |
| 报「加载本地 ASR 模型失败」 | 模型文件损坏或下载不完整；在「设置 → 模型管理」删除后重新下载 |
| 提示「未检测到可用的系统音频来源」 | Windows：确认系统至少有一个可用播放设备（插好耳机/音箱），在「设置 → 音频设备」点「刷新设备」；macOS：安装 BlackHole 等虚拟回环设备后重启应用 |
| 采集面板点「开始采集」报「未指定音频设备」 | 麦克风来源需在下拉中选中具体设备；系统音频来源可直接用系统默认播放设备（见第七节第 7 条） |
| 云端模式报「需要上传音频」 | 到「设置 → 隐私」开启「音频上传」；或在「设置 → 语音识别」切回「本地（隐私优先）」（本地模式不上传音频） |
| 云端模式报「XX 的云端 ASR 待接入」 | 当前仅「阿里云智能语音」已接入；请在设置页切换服务商或改用本地模式（见第七节第 2 条） |
| 云端「连接测试」失败 | 按提示依次检查：凭证是否已保存、Base URL 是否可达（代理/私有化部署需自行填写）、网络是否可用；网络问题会在文案中明确说明 |
| 云端「连接测试」报 **HTTP 404** | 地址填成了 **HTTP 接口**。百炼的 `https://dashscope.aliyuncs.com/compatible-mode/v1` 是 OpenAI 兼容的 HTTP 接口（供对话/嵌入/录音文件转写），**实时转写必须用 WebSocket**：`wss://dashscope.aliyuncs.com/api-ws/v1/inference`。点「恢复默认」或手填该地址后**保存配置**再测（详见 `ai-doc/需求分析.md` IA-16） |
| 云端「连接测试」报 **ModelNotFound** | 模型名不能用于实时转写：非实时（HTTP 文件转写）模型（如 `qwen-audio-3.1-asr-flash`，无 `-streaming`）只能走 HTTP 接口。请改用实测可用的实时模型：`qwen-audio-3.1-asr-flash-streaming`（默认）、`fun-asr-realtime`、`paraformer-realtime-v2` |
| 召回没有结果 | 确认演示已执行（会自动写入 4 篇示例笔记），或先在「知识库」导入 Markdown |
| 转写面板里明明有问题句，但「问题识别」一直为空、AI 回答也不生成 | ① 只有「面试官」通道的转写参与识别：单通道模式下请选「面试官（系统音频）」，双通道模式下确认系统音频通路正常（面试官的话应标为「面试官」而不是「我」）；② 判定单位是**句**（已按句末标点切句打分，见 `ai-doc/技术方案.md` 6.13），但**纯陈述句、疑问特征过弱或与上一次问题高度重复**的句子不会上报；③ 可先点「手动识别当前问题」用最近 30 秒转写兜底；④ 排查日志 `<数据目录>/logs/app.log.<日期>`：命中会写 `识别到面试问题（字数/得分/来源）`，未命中只有候选超时丢弃的 debug 记录 |
| 选了「浅色」切到别的页面后，界面又变回原来的主题 | 主题是**即时预览**：点选立刻生效但**不落库**，需在该分区点「保存配置」才持久化；未保存就离开设置页会自动回滚（避免误改）。见 `ai-doc/技术方案.md` 6.14 |
| 想让界面跟随系统深浅色 | 「设置 → 界面与快捷键 → 主题」选「跟随系统」，之后系统切换深浅色时界面实时变化，重启仍保持；系统主题不可用时回退深色 |
| 想重置全部数据重来一遍 | 关闭应用后删除数据目录：Windows `%APPDATA%\com.easyinterview.app\`，macOS `~/Library/Application Support/com.easyinterview.app/`（内含 `easy-interview.db` 与 `models/`），下次启动会重新初始化并再次进入首次向导。注意：API Key 存在系统钥匙链中，如需一并清除请在「设置 → AI 平台」中删除 |

---

## 九、目录结构

```
src/                    前端（React）
  components/interview/ 面试面板：转写 / 问题 / 回答 / 快捷条 / 演示卡 / 迷你页（就地迷你模式）
  pages/                知识库 / 面试辅助（左列演示卡+采集+转写，右列快捷条+问题+回答）/ 会话历史 / 设置 / 首次向导
  stores/               zustand 状态（含 IPC 降级 mock）
  hooks/                事件订阅（转写 / 问题 / 回答）、快捷键与主题同步
src-tauri/src/          后端（Rust）
  asr/                  ASR（通道抽象 / 本地引擎 + VAD 流水线 / 双通道 / 落库；cloud/ = 云端实时 WS，IA-16）
  audio/                采集（cpal 麦克风 + WASAPI 端点回环，IA-14）/ VAD / AEC / 设备 / WAV
  interview/            问题识别 / 知识库召回 / 回答生成 / 会话回放
  retrieval/            分块 / 中文分词 / FTS5 / 向量 / RRF 融合
  demo.rs               一键演示脚本与离线链路自检
  tray.rs hotkeys.rs    系统托盘 / 全局快捷键
ai-doc/需求分析.md       需求分析文档（场景/用例、FR+NFR、验收标准、追溯矩阵）
ai-doc/技术方案.md       完整技术方案（需求表、架构、决策与风险）
```
