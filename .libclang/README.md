# .libclang

`sherpa-rs-sys` 用 `bindgen` 生成 FFI 绑定，`bindgen` 运行时需要 `libclang.dll`。这个目录就是放它的地方。

## 什么时候需要

只有带 `local-asr-native` 特性构建时才需要（本地 ASR 原生推理）：

```powershell
$env:LIBCLANG_PATH="<仓库路径>\.libclang\bin"
npm run tauri dev -- --features local-asr-native
```

默认构建用不到，CI 也不用（`cargo test` 和日常构建都不碰这个目录）。macOS 同理用不到，Xcode Command Line Tools 自带 libclang。

## 仓库里已经带了什么

| 文件 | 说明 |
|---|---|
| `bin/libclang.dll` | 80.1 MB，直接可用 |
| `LICENSE-libclang.txt` | LLVM 的许可证（Apache-2.0 with LLVM Exceptions） |
| `fetch.ps1` | 重新获取 DLL 的脚本，正常情况下不用跑 |

来源是 PyPI 上的 `libclang` 包（18.1.1）里 `clang/native/libclang.dll` 那一个文件，不需要装完整 LLVM，也不需要 Python。

```
SHA256  B13EC96D9A036D6C6C36DD09E00415EAD7ACC7FCFD40E721BF33EA86B3490F9D
```

`bin/libclang.dll` 入库是为了 clone 完能直接编，代价是仓库多一个 80.1 MB 的文件（实测 git 对象库里压缩后约 27.5 MB，克隆时拉的差不多就是这个量）。如果你不打算开本地 ASR，这个体积可以忽略不计。

顺带说明，GitHub 对单个超过 50 MB 的文件会在推送时给一条警告（上限 100 MB，不会失败）。真嫌大的话可以把这份 DLL 换成 25.2 MB 的 wheel 加一条解压命令，代价是编译前多跑一步。

## 重新获取

DLL 丢了或者要换版本：

```powershell
pwsh -File .libclang/fetch.ps1              # 按 PyPI 上的 18.1.1 重下
pwsh -File .libclang/fetch.ps1 -Version 19.1.0 -Force
```

脚本从 PyPI 拉 wheel（约 25 MB）再解出 DLL，不走 LLVM 官方 GitHub Releases——那个包 1 GB 起步而且经常限速。

## 不再需要这个目录时

`git rm -r --cached .libclang` 之后把它加回 `.gitignore` 即可，不过已提交的历史里还会留着这份 blob，仓库体积要真正降下来得改写历史。
