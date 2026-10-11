$ErrorActionPreference = 'Stop'
function Get-WorkbenchHash([string]$TaskPath) {
    $taskStream = [System.IO.File]::OpenRead($TaskPath)
    $taskHasher = [System.Security.Cryptography.SHA256]::Create()
    try { return [System.BitConverter]::ToString($taskHasher.ComputeHash($taskStream)).Replace('-', '') } finally { $taskStream.Dispose(); $taskHasher.Dispose() }
}
. (Join-Path $PSScriptRoot 'environment.ps1')
Push-Location $taskRoot
try {
    & npm.cmd run build
    if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed' }
    & cargo build --manifest-path src-tauri/Cargo.toml --release --features custom-protocol --bin excel-workbench -j 2
    if ($LASTEXITCODE -ne 0) { throw 'Native build failed' }
    $taskReleaseRoot = Join-Path $taskRoot 'release'
    New-Item -ItemType Directory -Path $taskReleaseRoot -Force | Out-Null
    $taskVersion = (Get-Content -LiteralPath (Join-Path $taskRoot 'package.json') -Raw | ConvertFrom-Json).version
    $taskName = 'GridFlow-v' + $taskVersion + '-win-x64-' + (Get-Date -Format 'yyyyMMdd-HHmmss')
    $taskDirectory = Join-Path $taskReleaseRoot $taskName
    New-Item -ItemType Directory -Path $taskDirectory | Out-Null
    Copy-Item -LiteralPath 'src-tauri/target/release/excel-workbench.exe' -Destination (Join-Path $taskDirectory 'GridFlow.exe')
    Copy-Item -LiteralPath '.tools/duckdb/duckdb.dll' -Destination $taskDirectory
    Copy-Item -LiteralPath 'README.md' -Destination $taskDirectory
    Copy-Item -LiteralPath 'THIRD_PARTY_NOTICES.md' -Destination $taskDirectory
    & (Join-Path $PSScriptRoot 'licenses.ps1') | Out-Null
    Copy-Item -LiteralPath (Join-Path $taskReleaseRoot 'licenses') -Destination $taskDirectory -Recurse
    # Some registry archives use the Unix epoch; ZIP timestamps start in 1980.
    # Normalize the new release copies only, preserving source files and data.
    $taskZipEpoch = [datetime]::new(1980, 1, 1, 0, 0, 0, [DateTimeKind]::Local)
    Get-ChildItem -LiteralPath $taskDirectory -Recurse -Force | ForEach-Object {
        if ($_.LastWriteTime.Year -lt 1980 -or $_.LastWriteTime.Year -gt 2107) {
            $_.LastWriteTime = $taskZipEpoch
        }
    }
    $taskArchive = $taskDirectory + '.zip'
    Compress-Archive -LiteralPath $taskDirectory -DestinationPath $taskArchive
    $taskReport = [ordered]@{
        version = $taskVersion
        createdAt = (Get-Date -Format 'o')
        directory = $taskDirectory
        zip = $taskArchive
        zipBytes = (Get-Item -LiteralPath $taskArchive).Length
        files = @(Get-ChildItem -LiteralPath $taskDirectory -File | ForEach-Object { [ordered]@{name=$_.Name;bytes=$_.Length;sha256=(Get-WorkbenchHash $_.FullName)} })
    }
    $taskReport | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $taskReleaseRoot 'latest.json') -Encoding utf8
    Write-Output $taskArchive
} finally { Pop-Location }
