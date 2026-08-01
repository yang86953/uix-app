[CmdletBinding()]
param(
    [switch]$AllowDirtyForVerification
)

$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$version = '0.0.1'
$artifactName = "uix-$version-internal-win-x64"

$dirtyEntries = @(git -C $repoRoot status --porcelain=v1 --untracked-files=all)
if ($LASTEXITCODE -ne 0) {
    throw 'Unable to inspect the Git worktree.'
}
if ($dirtyEntries.Count -gt 0 -and -not $AllowDirtyForVerification) {
    throw 'The internal release must be built from a clean worktree. Use -AllowDirtyForVerification only to test the packaging path.'
}
if ($dirtyEntries.Count -gt 0) {
    $artifactName += '-UNVERIFIED'
    Write-Warning 'Building an UNVERIFIED packaging-path artifact from a dirty worktree.'
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

Push-Location $repoRoot
try {
    cargo build --release --locked --bin uix-demo
    if ($LASTEXITCODE -ne 0) {
        throw 'Release Demo build failed.'
    }

    $packageArgs = @('package', '--locked', '--offline')
    if ($AllowDirtyForVerification) {
        $packageArgs += '--allow-dirty'
    }
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
    'uix-demo.exe' = Join-Path $targetRoot 'release\uix-demo.exe'
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

Compress-Archive -Path (Join-Path $stageRoot '*') -DestinationPath $zipPath -CompressionLevel Optimal
$zipHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $zipPath).Hash.ToLowerInvariant()

Write-Output "Artifact: $zipPath"
Write-Output "SHA256:   $zipHash"
