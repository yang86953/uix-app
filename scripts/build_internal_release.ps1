$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$version = '0.0.1'
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

$targetRoot = [IO.Path]::GetFullPath([string]$metadata.target_directory)
$repoPrefix = $repoRoot + [IO.Path]::DirectorySeparatorChar
if (-not $targetRoot.StartsWith($repoPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing an internal release target outside the repository: $targetRoot"
}

# 演示二进制已迁入独立工作区（demo/gui-demo），其构建产物位于 demo/target。
$demoTargetRoot = Join-Path $repoRoot 'demo\target'

Push-Location $repoRoot
try {
    cargo build --release --locked --manifest-path demo\Cargo.toml --bin uix-demo
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
    'uix-demo.exe' = Join-Path $demoTargetRoot 'release\uix-demo.exe'
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
        # 使用最佳压缩级别创建一个且仅一个规范条目。
        [IO.Compression.ZipFileExtensions]::CreateEntryFromFile(
            # 传入当前唯一 ZIP owner。
            $archive,
            # 传入 staging 中的可信源文件。
            $sourcePath,
            # 传入经过规范化的 archive 路径。
            $entryName,
            # 保持既有最佳压缩策略。
            [IO.Compression.CompressionLevel]::Optimal
        ) | Out-Null
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
