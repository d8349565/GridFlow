$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$taskTools = Join-Path $taskRoot '.tools'
New-Item -ItemType Directory -Path $taskTools -Force | Out-Null
$taskRepositories = @('mstorsjo/llvm-mingw','Jake-Shadle/xwin')
foreach ($taskRepository in $taskRepositories) {
    $taskRelease = Invoke-RestMethod -Uri "https://api.github.com/repos/$taskRepository/releases/latest"
    $taskPattern = if ($taskRepository -like '*llvm-mingw') { 'ucrt-x86_64.zip$' } else { 'x86_64-pc-windows-msvc.tar.gz$' }
    $taskAsset = $taskRelease.assets | Where-Object name -Match $taskPattern | Select-Object -First 1
    if (!$taskAsset) { throw "No portable toolchain asset: $taskRepository" }
    $taskArchive = Join-Path $taskTools $taskAsset.name
    if (!(Test-Path -LiteralPath $taskArchive)) { Invoke-WebRequest -Uri $taskAsset.browser_download_url -OutFile $taskArchive }
    if ($taskAsset.digest -like 'sha256:*') {
        $taskDigest = (Get-FileHash -LiteralPath $taskArchive -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($taskDigest -ne $taskAsset.digest.Substring(7)) { throw "Checksum mismatch: $($taskAsset.name)" }
    }
    if ($taskArchive.EndsWith('.zip')) { Expand-Archive -LiteralPath $taskArchive -DestinationPath $taskTools -Force }
    else { & tar -xzf $taskArchive -C $taskTools; if ($LASTEXITCODE -ne 0) { throw 'Could not extract xwin' } }
}
$taskXwin = Get-ChildItem -LiteralPath $taskTools -Filter xwin.exe -Recurse -File | Select-Object -First 1
if (!$taskXwin) { throw 'xwin.exe not found' }
& $taskXwin.FullName --accept-license --cache-dir (Join-Path $taskTools 'xwin-cache') splat --output (Join-Path $taskTools 'msvc') --disable-symlinks --include-debug-libs
if ($LASTEXITCODE -ne 0) {
    # xwin 在 Windows 上可能仅因版本别名 symlink 权限失败；编译器使用真实目录。
    $taskRequired = @('crt/include/stdio.h','crt/lib/x86_64/libcmt.lib','sdk/include/um/Windows.h','sdk/include/ucrt/stdlib.h','sdk/lib/um/x86_64/kernel32.lib','sdk/lib/ucrt/x86_64/libucrt.lib')
    foreach ($taskRequiredFile in $taskRequired) {
        if (!(Test-Path -LiteralPath (Join-Path (Join-Path $taskTools 'msvc') $taskRequiredFile))) { throw "Windows SDK incomplete: $taskRequiredFile" }
    }
}
Write-Output 'Portable toolchain ready. All files are confined to the project .tools directory.'
