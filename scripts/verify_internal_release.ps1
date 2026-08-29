# 声明内部候选包校验器的命令行契约。
param(
    # ZIP 路径必须由调用方显式提供。
    [Parameter(Mandatory = $true)]
    # 接收待验证的内部候选包路径。
    [string]$ZipPath,
    # 版本默认与当前首发候选保持一致。
    [string]$Version = '0.0.2'
)

# 任何校验失败都立即终止并返回非零进程状态。
$ErrorActionPreference = 'Stop'

# 校验 ZIP 条目只使用安全且规范的相对路径。
function Assert-SafeArchiveEntryName {
    # 声明单个条目名称参数。
    param(
        # 条目名称必须由调用方显式传入。
        [Parameter(Mandatory = $true)]
        # 接收 ZIP 中保存的原始名称。
        [string]$Name
    )

    # 空名称不能描述可交付文件。
    if ([string]::IsNullOrWhiteSpace($Name)) {
        # 拒绝无法定位的空条目。
        throw 'Internal release contains an empty archive entry name.'
    }
    # ZIP 规范路径必须使用正斜杠。
    if ($Name.Contains('\')) {
        # 拒绝依赖 Windows 解压器私有解释的反斜杠路径。
        throw "Internal release entry is not canonical: $Name"
    }
    # 根路径、UNC 与盘符都不得越过解包根目录。
    if ($Name.StartsWith('/') -or $Name.Contains(':') -or [IO.Path]::IsPathRooted($Name)) {
        # 拒绝任何绝对 archive 路径。
        throw "Internal release entry is rooted: $Name"
    }
    # 按规范分隔符拆分路径段。
    $segments = @($Name.Split('/'))
    # 空段、当前目录与父目录都可能改变解包目标。
    if (@($segments | Where-Object { $_ -eq '' -or $_ -eq '.' -or $_ -eq '..' }).Count -gt 0) {
        # 拒绝路径穿越与非规范分隔。
        throw "Internal release entry contains an unsafe path segment: $Name"
    }
}

# 将版本约束为构建器支持的稳定数字形式。
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
    # 非法版本不能参与预期条目拼接。
    throw "Internal release version is invalid: $Version"
}
# 解析并确认输入确实是现有 ZIP 文件。
$resolvedZip = (Resolve-Path -LiteralPath $ZipPath -ErrorAction Stop).Path
# 非文件输入不能进入 archive reader。
if (-not (Test-Path -LiteralPath $resolvedZip -PathType Leaf)) {
    # 拒绝目录或消失的输入。
    throw "Internal release ZIP is not a file: $resolvedZip"
}
# 后缀必须明确表明 ZIP 容器。
if (-not $resolvedZip.EndsWith('.zip', [StringComparison]::OrdinalIgnoreCase)) {
    # 拒绝误传其它制品。
    throw "Internal release artifact is not a ZIP: $resolvedZip"
}

# 声明必须被哈希清单逐项覆盖的七个 payload。
$expectedPayload = @(
    # uix-lang release Demo 是首个可执行载荷。
    'uix-lang-demo.exe'
    # 内部 crate 使用精确版本命名。
    "uix-$Version.crate"
    # 专有许可必须随包交付。
    'LICENSE'
    # 第三方声明必须随包交付。
    'THIRD_PARTY_NOTICES.md'
    # 当前版本变更记录必须随包交付。
    'CHANGELOG.md'
    # 使用入口必须随包交付。
    'README.md'
    # Demo 的运行时图片使用规范 archive 路径。
    'assets/images/demo.png'
)
# 清单自身是第八个且唯一不自哈希的条目。
$expectedEntries = @($expectedPayload) + 'SHA256SUMS.txt'
# 固化构建器写入的规范条目时间，读取时忽略 ZIP 不保存的时区信息。
$expectedEntryTimestamp = [DateTime]::new(1980, 1, 1, 0, 0, 0, [DateTimeKind]::Unspecified)

# 加载 ZIP reader 契约所在的基础程序集。
Add-Type -AssemblyName System.IO.Compression
# 加载只读 ZIP 文件扩展 API 所需的程序集。
Add-Type -AssemblyName System.IO.Compression.FileSystem
# 打开待验证 ZIP，并由当前脚本唯一拥有 reader 生命周期。
$archive = [IO.Compression.ZipFile]::OpenRead($resolvedZip)
# 无论校验结果如何都必须释放文件句柄。
try {
    # 固化条目快照，避免在多次检查间重新枚举 reader。
    $entries = @($archive.Entries)
    # 使用 Windows 消费语义拒绝仅大小写不同的重复条目。
    $seenNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    # 在集合比较前先验证每个原始 archive 路径。
    foreach ($entry in $entries) {
        # 保存当前条目的原始规范名称。
        $entryName = $entry.FullName
        # 拒绝路径穿越、绝对路径与反斜杠。
        Assert-SafeArchiveEntryName -Name $entryName
        # 同名或仅大小写不同的重复条目会让解包结果不确定。
        if (-not $seenNames.Add($entryName)) {
            # 拒绝重复所有权的 archive 路径。
            throw "Internal release contains a duplicate entry: $entryName"
        }
        # 条目时间必须与构建契约一致，避免 staging 文件时间污染容器摘要。
        if ($entry.LastWriteTime.DateTime -ne $expectedEntryTimestamp) {
            # 拒绝使相同载荷产生不同 ZIP 字节的时间戳漂移。
            throw "Internal release entry timestamp is not canonical: $entryName"
        }
    }
    # 条目总数必须与冻结包结构完全相同。
    if ($entries.Count -ne $expectedEntries.Count) {
        # 同时拒绝多余目录条目、额外文件与缺失文件。
        throw "Internal release entry count mismatch: expected $($expectedEntries.Count), got $($entries.Count)."
    }
    # 逐项验证精确大小写与路径。
    foreach ($expectedEntry in $expectedEntries) {
        # 预期条目必须在原始名称集合中精确存在一次。
        if (-not (@($entries.FullName) -ccontains $expectedEntry)) {
            # 拒绝缺失或大小写漂移的交付文件。
            throw "Internal release is missing the exact entry: $expectedEntry"
        }
    }

    # 取得唯一且已通过集合校验的哈希清单。
    $manifestEntry = $archive.GetEntry('SHA256SUMS.txt')
    # 清单大小设置保守上限，避免异常载荷占用无界内存。
    if ($manifestEntry.Length -gt 65536) {
        # 拒绝不符合七项摘要规模的清单。
        throw "Internal release hash manifest is too large: $($manifestEntry.Length) bytes."
    }
    # 为清单原始字节创建有界内存 owner。
    $manifestBuffer = [IO.MemoryStream]::new()
    # 打开清单条目的只读 stream。
    $manifestStream = $manifestEntry.Open()
    # 无论读取是否成功都必须释放 entry stream。
    try {
        # 复制小型清单到内存以验证编码与行格式。
        $manifestStream.CopyTo($manifestBuffer)
    }
    # 读取完成后立即释放 ZIP entry stream。
    finally {
        # 关闭清单流但保留内存副本。
        $manifestStream.Dispose()
    }
    # 取得清单的原始 ASCII 字节。
    $manifestBytes = $manifestBuffer.ToArray()
    # 内存 owner 已不再需要。
    $manifestBuffer.Dispose()
    # 空清单不能证明任何 payload。
    if ($manifestBytes.Count -eq 0) {
        # 拒绝空摘要文件。
        throw 'Internal release hash manifest is empty.'
    }
    # 清单必须保持可移植的纯 ASCII 编码。
    if (@($manifestBytes | Where-Object { $_ -gt 127 }).Count -gt 0) {
        # 拒绝隐式编码替换或不可见 Unicode。
        throw 'Internal release hash manifest is not ASCII.'
    }
    # 最后一行必须由换行符明确终止。
    if ($manifestBytes[$manifestBytes.Count - 1] -ne 10) {
        # 拒绝不稳定的尾行格式。
        throw 'Internal release hash manifest lacks a final newline.'
    }
    # 将已证明为 ASCII 的字节转换为文本。
    $manifestText = [Text.Encoding]::ASCII.GetString($manifestBytes)
    # 去除唯一允许的尾部 CR/LF 序列。
    $trimmedManifest = $manifestText.TrimEnd([char[]]@([char]13, [char]10))
    # 按 CRLF 或 LF 拆分跨工具可读的摘要行。
    $manifestLines = @([Text.RegularExpressions.Regex]::Split($trimmedManifest, '\r?\n'))
    # 七个 payload 必须一一对应七行摘要。
    if ($manifestLines.Count -ne $expectedPayload.Count) {
        # 拒绝重复、缺失或额外摘要。
        throw "Internal release hash line count mismatch: expected $($expectedPayload.Count), got $($manifestLines.Count)."
    }

    # 按冻结顺序验证摘要语法、名称和实际内容。
    for ($index = 0; $index -lt $expectedPayload.Count; $index++) {
        # 取得当前预期 payload 名称。
        $expectedName = $expectedPayload[$index]
        # 取得对应位置的摘要行。
        $line = $manifestLines[$index]
        # 每行必须是小写 SHA-256、两个空格和规范名称。
        if ($line -notmatch '^([0-9a-f]{64})  (.+)$') {
            # 拒绝大写、错误长度、空名称与分隔漂移。
            throw "Internal release hash line is invalid: $line"
        }
        # 保存正则捕获的声明哈希。
        $declaredHash = $Matches[1]
        # 保存正则捕获的声明名称。
        $declaredName = $Matches[2]
        # 清单名称必须与冻结顺序和大小写完全一致。
        if (-not [StringComparer]::Ordinal.Equals($declaredName, $expectedName)) {
            # 拒绝清单重排、别名或未覆盖 payload。
            throw "Internal release hash entry mismatch: expected $expectedName, got $declaredName."
        }
        # 取得已经验证存在的 payload entry。
        $payloadEntry = $archive.GetEntry($expectedName)
        # 为单项内容创建 SHA-256 owner。
        $sha256 = [Security.Cryptography.SHA256]::Create()
        # 打开当前 payload 的只读 stream。
        $payloadStream = $payloadEntry.Open()
        # 无论计算是否成功都必须释放 stream 与哈希器。
        try {
            # 对解压后的真实 payload 字节计算 SHA-256。
            $actualHashBytes = $sha256.ComputeHash($payloadStream)
            # 转换为与清单相同的小写十六进制格式。
            $actualHash = [BitConverter]::ToString($actualHashBytes).Replace('-', '').ToLowerInvariant()
        }
        # 完成单项计算后释放所有临时 owner。
        finally {
            # 关闭当前 payload stream。
            $payloadStream.Dispose()
            # 释放 SHA-256 实例。
            $sha256.Dispose()
        }
        # 声明哈希必须与解包内容逐字节一致。
        if (-not [StringComparer]::Ordinal.Equals($declaredHash, $actualHash)) {
            # 拒绝损坏、篡改或过期清单。
            throw "Internal release hash mismatch for ${expectedName}: expected $declaredHash, got $actualHash."
        }
    }
}
# 完成或失败时都释放 ZIP reader 与文件句柄。
finally {
    # 关闭只读 archive owner。
    $archive.Dispose()
}

# 在 archive 关闭后计算整个 ZIP 的交付摘要。
$zipHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $resolvedZip).Hash.ToLowerInvariant()
# 输出稳定成功证据供构建器与人工记录。
Write-Output "Verified internal release: $resolvedZip"
# 输出整个 ZIP 的最终 SHA-256。
Write-Output "Verified SHA256: $zipHash"
