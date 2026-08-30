$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# 固定仓库、版本与 Windows 候选包路径。
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$version = '0.0.5'
$artifactName = "uix-$version-internal-win-x64.zip"
$artifactPath = Join-Path $repoRoot "target\internal-release\$artifactName"
$evidencePath = Join-Path $repoRoot "target\internal-release\uix-$version-windows-acceptance.json"

# 运行一个原生命令并把非零退出码提升为稳定失败。
function Invoke-NativeChecked {
    param(
        # 原生命令名称。
        [Parameter(Mandatory = $true)][string]$Command,
        # 原生命令参数，保持调用方给定顺序。
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        # 验收失败时使用的稳定说明。
        [Parameter(Mandatory = $true)][string]$Failure
    )

    # 直接执行命令并让实时输出进入当前终端。
    & $Command @Arguments
    # 任一非零退出码都终止完整验收，禁止继续生成成功证据。
    if ($LASTEXITCODE -ne 0) {
        throw "$Failure (exit code $LASTEXITCODE)."
    }
}

# 运行只需读取一行事实的命令并返回去除首尾空白的文本。
function Get-NativeText {
    param(
        # 原生命令名称。
        [Parameter(Mandatory = $true)][string]$Command,
        # 原生命令参数。
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        # 失败说明。
        [Parameter(Mandatory = $true)][string]$Failure
    )

    # 捕获版本或 Git 身份等小型机器输出。
    $lines = @(& $Command @Arguments 2>&1)
    # 读取失败时不返回残缺事实。
    if ($LASTEXITCODE -ne 0) {
        throw "$Failure (exit code $LASTEXITCODE)."
    }
    # 统一为稳定的单个字符串。
    return (($lines | Out-String).Trim())
}

# 本入口只允许真实 Windows x64 主机执行。
if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) {
    throw 'Windows release acceptance requires a Windows host.'
}
if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne [System.Runtime.InteropServices.Architecture]::X64) {
    throw 'Windows release acceptance requires an x64 host.'
}

# 构建和证据采集依赖的命令必须全部可解析。
foreach ($commandName in @('cargo', 'git', 'rustc')) {
    Get-Command $commandName -ErrorAction Stop | Out-Null
}

# 候选验收只能从完全干净且与远端主分支一致的工作树开始。
$dirtyEntries = @(git -C $repoRoot status --porcelain=v1 --untracked-files=all)
if ($LASTEXITCODE -ne 0) {
    throw 'Unable to inspect the Git worktree.'
}
if ($dirtyEntries.Count -gt 0) {
    throw 'Windows release acceptance requires a clean worktree.'
}
Invoke-NativeChecked 'git' @('-C', $repoRoot, 'fetch', '--prune', 'origin') 'Git fetch failed'
$branch = Get-NativeText 'git' @('-C', $repoRoot, 'branch', '--show-current') 'Unable to read branch'
if ($branch -ne 'main') {
    throw "Windows release acceptance requires branch main, got $branch."
}
$commit = Get-NativeText 'git' @('-C', $repoRoot, 'rev-parse', 'HEAD') 'Unable to read HEAD'
$remoteCommit = Get-NativeText 'git' @('-C', $repoRoot, 'rev-parse', 'origin/main') 'Unable to read origin/main'
if ($commit -ne $remoteCommit) {
    throw 'Windows release acceptance requires HEAD to equal origin/main.'
}

# 保存本次真实主机的软件与图形设备事实。
$os = Get-CimInstance -ClassName Win32_OperatingSystem
$gpuRows = @(Get-CimInstance -ClassName Win32_VideoController)
if ($gpuRows.Count -eq 0) {
    throw 'Windows release acceptance requires at least one reported video controller.'
}
$rustcVersion = Get-NativeText 'rustc' @('--version') 'Unable to read rustc version'
$cargoVersion = Get-NativeText 'cargo' @('--version') 'Unable to read cargo version'

Push-Location $repoRoot
try {
    # Vulkan 共享场景、资源与 surface 生命周期必须在真实 Windows 驱动上通过。
    Invoke-NativeChecked 'cargo' @(
        'test', '--locked', '--no-default-features',
        '--features', 'vulkan-parity-test',
        '--test', 'vulkan_gpu_parity', '--', '--nocapture'
    ) 'Windows Vulkan parity failed'

    # D3D11 兼容回退必须在同一真实 Windows 主机上通过共享规范验收。
    Invoke-NativeChecked 'cargo' @(
        'test', '--locked', '--no-default-features',
        '--features', 'd3d11-parity-test',
        '--test', 'd3d11_gpu_parity', '--', '--nocapture'
    ) 'Windows D3D11 parity failed'

    # 主演示按默认 registry 自动选择 Vulkan，并对真实最终 surface 执行像素与搬移验收。
    $readbackLines = @(& cargo run --release --locked --manifest-path demo\Cargo.toml `
        --features test-harness --bin uix-lang-demo -- --test-graphics-readback 2>&1)
    $readbackExitCode = $LASTEXITCODE
    # 把被捕获的完整输出回放到当前终端，便于人工与自动日志同时取证。
    $readbackLines | ForEach-Object { Write-Output $_ }
    if ($readbackExitCode -ne 0) {
        throw "Windows automatic surface readback failed (exit code $readbackExitCode)."
    }
    $readbackText = ($readbackLines | Out-String)
    if (-not $readbackText.Contains('UIX_GRAPHICS_READBACK_OK')) {
        throw 'Windows automatic surface readback did not emit the success marker.'
    }

    # 第一次生成并内置校验规范 Windows ZIP。
    & (Join-Path $PSScriptRoot 'build_internal_release.ps1')
    if (-not (Test-Path -LiteralPath $artifactPath -PathType Leaf)) {
        throw "Windows release artifact is missing: $artifactPath"
    }
    $firstHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $artifactPath).Hash.ToLowerInvariant()

    # 使用相同提交、工具链和依赖输入再次生成候选包。
    & (Join-Path $PSScriptRoot 'build_internal_release.ps1')
    if (-not (Test-Path -LiteralPath $artifactPath -PathType Leaf)) {
        throw "Second Windows release artifact is missing: $artifactPath"
    }
    $secondHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $artifactPath).Hash.ToLowerInvariant()
    # 两次容器摘要必须逐字节一致。
    if ($firstHash -ne $secondHash) {
        throw "Windows release archive is not reproducible: $firstHash != $secondHash"
    }

    # 在最终保留的第二份 ZIP 上再次执行独立只读校验器。
    & (Join-Path $PSScriptRoot 'verify_internal_release.ps1') `
        -ZipPath $artifactPath `
        -Version $version

    # 只把最小、非敏感且可复核的环境事实写入目标目录。
    $evidence = [ordered]@{
        schema_version = 1
        product_version = $version
        commit = $commit
        generated_at_utc = [DateTime]::UtcNow.ToString('o')
        operating_system = [ordered]@{
            caption = [string]$os.Caption
            version = [string]$os.Version
            build_number = [string]$os.BuildNumber
            architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        }
        toolchain = [ordered]@{
            rustc = $rustcVersion
            cargo = $cargoVersion
        }
        graphics = [ordered]@{
            automatic_surface_readback = 'passed'
            vulkan_parity = 'passed'
            d3d11_parity = 'passed'
            adapters = @($gpuRows | ForEach-Object {
                [ordered]@{
                    name = [string]$_.Name
                    driver_version = [string]$_.DriverVersion
                    video_processor = [string]$_.VideoProcessor
                }
            })
        }
        artifact = [ordered]@{
            name = $artifactName
            size_bytes = (Get-Item -LiteralPath $artifactPath).Length
            sha256 = $secondHash
            reproducible = $true
            verifier = 'passed'
        }
    }
    # 证据文件不进入候选 ZIP，也不修改仓库工作树。
    $evidence | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $evidencePath -Encoding utf8

    # 输出 runner 与人工记录都可直接提取的最终字段。
    Write-Output "UIX_WINDOWS_ACCEPTANCE_OK commit=$commit sha256=$secondHash"
    Write-Output "Artifact: $artifactPath"
    Write-Output "Evidence: $evidencePath"
}
finally {
    # 无论成功或失败都恢复调用方位置。
    Pop-Location
}
