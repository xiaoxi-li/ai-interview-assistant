# 取 libclang.dll，供 sherpa-rs-sys 的 bindgen 使用（带 local-asr-native 特性构建时才需要）。
#
#   pwsh -File .libclang/fetch.ps1
#   pwsh -File .libclang/fetch.ps1 -Version 19.1.0 -Force
#
# 仓库里已经带了 bin/libclang.dll，正常情况不用跑这个脚本。
# 走 PyPI 的 wheel（约 25MB）而不是 LLVM 官方 GitHub Releases 的整包（1GB 起步且常限速）。

param(
    [string]$Version = '18.1.1',
    [switch]$Force,
    [switch]$KeepWheel
)

$ProgressPreference = 'SilentlyContinue'
$ErrorActionPreference = 'Stop'

$dir = $PSScriptRoot
$bin = Join-Path $dir 'bin'
$dll = Join-Path $bin 'libclang.dll'
$wheel = Join-Path $dir 'libclang.whl'

if ((Test-Path $dll) -and -not $Force) {
    Write-Host "已存在，跳过：$dll"
    Write-Host "要重新下载加 -Force。"
    exit 0
}

New-Item -ItemType Directory -Force -Path $bin | Out-Null

# 1) 找 wheel 地址：优先 PyPI 的 JSON API，不通就退回阿里云镜像的 simple 索引
function Get-WheelUrl {
    param([string]$Version)

    $pattern = "libclang-$([regex]::Escape($Version))-py2\.py3-none-win_amd64\.whl"

    try {
        $meta = Invoke-RestMethod -Uri "https://pypi.org/pypi/libclang/$Version/json" -TimeoutSec 60
        $hit = $meta.urls | Where-Object { $_.filename -match $pattern } | Select-Object -First 1
        if ($hit) { return $hit.url }
    } catch {
        Write-Host "PyPI JSON API 不通（$($_.Exception.Message)），换阿里云镜像。"
    }

    $index = Invoke-WebRequest -Uri "https://mirrors.aliyun.com/pypi/simple/libclang/" -TimeoutSec 60
    $match = [regex]::Match($index.Content, "href=""([^""]*$pattern[^""]*)""")
    if (-not $match.Success) {
        throw "在 PyPI 和阿里云镜像上都没找到 libclang $Version 的 Windows wheel。"
    }
    return $match.Groups[1].Value
}

$url = Get-WheelUrl -Version $Version
Write-Host "下载 $url"
if (Test-Path $wheel) { Remove-Item -LiteralPath $wheel -Force }
Invoke-WebRequest -Uri $url -OutFile $wheel -TimeoutSec 1800
Write-Host ("已下载 {0:N1} MB" -f ((Get-Item $wheel).Length / 1MB))

# 2) 从 wheel 里掏出那一个 DLL（wheel 就是 zip，只是扩展名不是 .zip，Expand-Archive 不认）
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::OpenRead($wheel)
try {
    $entry = $zip.Entries |
        Where-Object { $_.FullName -match 'clang/native/libclang\.dll$' } |
        Select-Object -First 1
    if (-not $entry) { throw "wheel 里没有 clang/native/libclang.dll，可能包结构变了。" }
    [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $dll, $true)
} finally {
    $zip.Dispose()
}

if (-not $KeepWheel) { Remove-Item -LiteralPath $wheel -Force }

$hash = (Get-FileHash -LiteralPath $dll -Algorithm SHA256).Hash
Write-Host ("已写入 {0}（{1:N1} MB）" -f $dll, ((Get-Item $dll).Length / 1MB))
Write-Host "SHA256 $hash"
Write-Host ''
Write-Host '构建前设置：'
Write-Host ('  $env:LIBCLANG_PATH="{0}"' -f $bin)
