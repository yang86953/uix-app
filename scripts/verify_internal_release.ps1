# Windows 原生验证器委托共同独立校验核心，版本必须显式给定。
param(
    [Parameter(Mandatory = $true)][string]$ZipPath,
    [Parameter(Mandatory = $true)][string]$Version
)
$ErrorActionPreference = 'Stop'
& python "$PSScriptRoot/verify_internal_release.py" --archive $ZipPath --version $Version
if ($LASTEXITCODE -ne 0) { throw "Internal candidate verification failed ($LASTEXITCODE)." }
