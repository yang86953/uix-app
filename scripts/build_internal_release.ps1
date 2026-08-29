$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$version = '0.0.2'
$artifactName = "uix-$version-internal-win-x64"

$dirtyEntries = @(git -C $repoRoot status --porcelain=v1 --untracked-files=all)
if ($LASTEXITCODE -ne 0) {
    throw 'Unable to inspect the Git worktree.'
}
# 内部发布只接受干净工作树。
if ($dirtyEntries.Count -gt 0) {
    # 脏工作树不得生成内部发布包。
    throw 'The internal release must be built from a clean worktree.'
}

$metadataJson = cargo metadata --format-version 1 --no-deps --locked
if ($LASTEXITCODE -ne 0) {
    throw 'cargo metadata failed.'
}
$metadata = $metadataJson | ConvertFrom-Json
$package = @($metadata.packages | Where-Object { $_.name -eq 'uix' })
if ($package.Count -ne 1 -or $package[0].version -ne $version) {
    throw "Expected exactly one uix package at version $version."
}
# 从 Cargo metadata 读取正式内部 crate 的描述事实。
$packageDescription = [string]$package[0].description
# 发布组合根拒绝缺失或仅含空白的描述，避免生成身份不完整的 .crate。
if ([string]::IsNullOrWhiteSpace($packageDescription)) {
    # 使用稳定错误说明具体缺失的 package 元数据。
    throw 'Expected the uix package to declare a non-empty description.'
}

$targetRoot = [IO.Path]::GetFullPath([string]$metadata.target_directory)
$repoPrefix = $repoRoot + [IO.Path]::DirectorySeparatorChar
if (-not $targetRoot.StartsWith($repoPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing an internal release target outside the repository: $targetRoot"
}

# 演示二进制位于独立工作区（demo/uix-lang-demo），其构建产物位于 demo/target。
$demoTargetRoot = Join-Path $repoRoot 'demo\target'

Push-Location $repoRoot
try {
    # 构建 Windows Vulkan 首选、D3D11 兼容回退的主演示。
    cargo build --release --locked --manifest-path demo\Cargo.toml --bin uix-lang-demo
    if ($LASTEXITCODE -ne 0) {
        throw 'Release Demo build failed.'
    }

    # Cargo 打包时会把私有路径依赖改写为 registry 依赖。
    # 包解析继续固定到仓库内的 proc-macro 源码，内部 UIX crate 不从 crates.io 解析。
    $derivePath = (Resolve-Path -LiteralPath (Join-Path $repoRoot 'uix-derive')).Path.Replace('\', '/')
    $packageArgs = @(
        'package'
        '--locked'
        '--offline'
        '--config'
        "patch.crates-io.uix-derive.path='$derivePath'"
    )
    cargo @packageArgs
    if ($LASTEXITCODE -ne 0) {
        throw 'Internal cargo package build failed.'
    }
}
finally {
    Pop-Location
}

$outputRoot = Join-Path $targetRoot 'internal-release'
New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null
$resolvedOutputRoot = (Resolve-Path -LiteralPath $outputRoot).Path
$stageRoot = Join-Path $resolvedOutputRoot $artifactName
$zipPath = Join-Path $resolvedOutputRoot "$artifactName.zip"

foreach ($existingPath in @($stageRoot, $zipPath)) {
    if (-not (Test-Path -LiteralPath $existingPath)) {
        continue
    }
    $resolvedExisting = (Resolve-Path -LiteralPath $existingPath).Path
    $safePrefix = $resolvedOutputRoot + [IO.Path]::DirectorySeparatorChar
    if (-not $resolvedExisting.StartsWith($safePrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to replace a path outside the internal release directory: $resolvedExisting"
    }
    Remove-Item -Recurse -Force -LiteralPath $resolvedExisting
}

New-Item -ItemType Directory -Path $stageRoot | Out-Null

$payload = [ordered]@{
    'uix-lang-demo.exe' = Join-Path $demoTargetRoot 'release\uix-lang-demo.exe'
    "uix-$version.crate" = Join-Path $targetRoot "package\uix-$version.crate"
    'LICENSE' = Join-Path $repoRoot 'LICENSE'
    'THIRD_PARTY_NOTICES.md' = Join-Path $repoRoot 'THIRD_PARTY_NOTICES.md'
    'CHANGELOG.md' = Join-Path $repoRoot 'CHANGELOG.md'
    'README.md' = Join-Path $repoRoot 'README.md'
    'assets/images/demo.png' = Join-Path $repoRoot 'assets\images\demo.png'
}

foreach ($entry in $payload.GetEnumerator()) {
    if (-not (Test-Path -LiteralPath $entry.Value -PathType Leaf)) {
        throw "Missing release payload: $($entry.Value)"
    }
    $destination = Join-Path $stageRoot $entry.Key
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destination) | Out-Null
    Copy-Item -LiteralPath $entry.Value -Destination $destination
}

$hashLines = foreach ($name in $payload.Keys) {
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $stageRoot $name)).Hash.ToLowerInvariant()
    "$hash  $name"
}
Set-Content -LiteralPath (Join-Path $stageRoot 'SHA256SUMS.txt') -Encoding ascii -Value $hashLines

# 加载 ZIP 模式与压缩级别枚举所在的基础程序集。
Add-Type -AssemblyName System.IO.Compression
# 加载只按显式条目创建 ZIP 所需的文件扩展程序集。
Add-Type -AssemblyName System.IO.Compression.FileSystem
# 固化为 ZIP 规范允许的最早时间，避免当次 staging 时间进入容器摘要。
$archiveTimestamp = [DateTimeOffset]::new(1980, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
# 以 Create 模式打开唯一内部候选包。
$archive = [IO.Compression.ZipFile]::Open($zipPath, [IO.Compression.ZipArchiveMode]::Create)
# 无论条目写入是否失败，都必须关闭 ZIP owner。
try {
    # 按稳定 payload 顺序把每个文件写入候选包。
    foreach ($name in @($payload.Keys) + 'SHA256SUMS.txt') {
        # 取得已经在 staging 目录验证过的源文件。
        $sourcePath = Join-Path $stageRoot $name
        # ZIP 条目统一使用规范正斜杠，避免依赖 Windows 解压器解释反斜杠。
        $entryName = $name.Replace('\', '/')
        # 使用最佳压缩级别创建一个且仅一个尚未打开的规范条目。
        $archiveEntry = $archive.CreateEntry($entryName, [IO.Compression.CompressionLevel]::Optimal)
        # 在条目 stream 打开前写入确定时间，满足 ZipArchive Create 模式生命周期。
        $archiveEntry.LastWriteTime = $archiveTimestamp
        # 以只读方式打开已经在 staging 目录验证过的源文件。
        $sourceStream = [IO.File]::OpenRead($sourcePath)
        # 时间戳固定后再打开唯一条目写入流。
        $entryStream = $archiveEntry.Open()
        # 两个流必须在下一个条目创建前同时关闭。
        try {
            # 流式复制载荷，避免把可执行文件或 crate 整体读入内存。
            $sourceStream.CopyTo($entryStream)
        }
        # 成功或失败都释放源文件与 archive 条目流。
        finally {
            # 先关闭目标流，使当前条目的压缩数据完整写回 archive。
            $entryStream.Dispose()
            # 再关闭只读源文件句柄。
            $sourceStream.Dispose()
        }
    }
}
# ZIP owner 必须在哈希与独立校验前关闭并写完中央目录。
finally {
    # 释放 archive 句柄并完成中央目录。
    $archive.Dispose()
}
# 用独立只读校验器验证精确载荷、路径与逐项哈希。
& (Join-Path $PSScriptRoot 'verify_internal_release.ps1') -ZipPath $zipPath -Version $version
$zipHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $zipPath).Hash.ToLowerInvariant()

Write-Output "Artifact: $zipPath"
Write-Output "SHA256:   $zipHash"
