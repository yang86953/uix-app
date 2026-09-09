# Windows 原生入口共享 Python 3.11+ 合同，不维护第二套载荷清单。
param(
    [Parameter(Mandatory = $true)][string]$DocsRepo,
    [Parameter(Mandatory = $true)][string]$DocsRevision,
    [string]$Version
)
$ErrorActionPreference = 'Stop'
$arguments = @("$PSScriptRoot/build_internal_release.py", '--platform', 'win-x64', '--docs-repo', $DocsRepo, '--docs-revision', $DocsRevision)
if ($Version) { $arguments += @('--version', $Version) }
& python @arguments
if ($LASTEXITCODE -ne 0) { throw "Internal candidate build failed ($LASTEXITCODE)." }
