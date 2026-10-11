$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$taskLicenseRoot = Join-Path $taskRoot 'release/licenses'
New-Item -ItemType Directory -Path $taskLicenseRoot -Force | Out-Null
$taskNodeNames = @('react','react-dom','scheduler','ag-grid-community','ag-grid-react','echarts','zrender','lucide-react','tslib','ssf','frac','@tauri-apps/api','@tauri-apps/plugin-dialog')
foreach ($taskName in $taskNodeNames) {
    $taskPackageRoot = Join-Path (Join-Path $taskRoot 'node_modules') $taskName
    if (Test-Path -LiteralPath $taskPackageRoot) {
        $taskDestination = Join-Path $taskLicenseRoot ('npm-' + ($taskName -replace '[/@]', '_'))
        New-Item -ItemType Directory -Path $taskDestination -Force | Out-Null
        Get-ChildItem -LiteralPath $taskPackageRoot -File | Where-Object Name -Match '^(LICEN[CS]E|NOTICE|COPYING)' | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $taskDestination }
    }
}
$taskMetadata = (& cargo metadata --manifest-path (Join-Path $taskRoot 'src-tauri/Cargo.toml') --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc --features custom-protocol | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) { throw 'Cargo license metadata failed' }
$taskQueue = [System.Collections.Generic.Queue[string]]::new()
$taskSeen = [System.Collections.Generic.HashSet[string]]::new()
$taskQueue.Enqueue($taskMetadata.resolve.root)
while ($taskQueue.Count -gt 0) {
    $taskId = $taskQueue.Dequeue()
    if (!$taskSeen.Add($taskId)) { continue }
    $taskPackage = $taskMetadata.packages | Where-Object id -eq $taskId | Select-Object -First 1
    if ($taskPackage.source) {
        $taskDirectory = Split-Path $taskPackage.manifest_path -Parent
        $taskDestination = Join-Path $taskLicenseRoot ('rust-' + $taskPackage.name + '-' + $taskPackage.version)
        New-Item -ItemType Directory -Path $taskDestination -Force | Out-Null
        Get-ChildItem -LiteralPath $taskDirectory -File | Where-Object Name -Match '^(LICEN[CS]E|NOTICE|COPYING)' | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $taskDestination }
        ('Package: ' + $taskPackage.name + "`nVersion: " + $taskPackage.version + "`nLicense: " + $taskPackage.license + "`nSource: " + $taskPackage.repository) | Set-Content -LiteralPath (Join-Path $taskDestination 'SOURCE.txt') -Encoding utf8
    }
    $taskNode = $taskMetadata.resolve.nodes | Where-Object id -eq $taskId | Select-Object -First 1
    foreach ($taskDependency in $taskNode.deps) { if ($taskDependency.dep_kinds | Where-Object { $_.kind -eq $null }) { $taskQueue.Enqueue($taskDependency.pkg) } }
}
$taskDuckLicense = Join-Path $taskLicenseRoot 'DuckDB-LICENSE.txt'
$taskLocalDuckLicense = Join-Path $taskRoot 'docs/licenses/DuckDB-LICENSE.txt'
$taskToolsDuckLicense = Join-Path $taskRoot '.tools/duckdb/LICENSE.txt'
if (Test-Path -LiteralPath $taskLocalDuckLicense) { Copy-Item -LiteralPath $taskLocalDuckLicense -Destination $taskDuckLicense -Force }
elseif (Test-Path -LiteralPath $taskToolsDuckLicense) { Copy-Item -LiteralPath $taskToolsDuckLicense -Destination $taskDuckLicense -Force }
elseif (!(Test-Path -LiteralPath $taskDuckLicense)) { Invoke-WebRequest -Uri 'https://raw.githubusercontent.com/duckdb/duckdb/v1.5.6/LICENSE' -OutFile $taskDuckLicense }
Write-Output $taskLicenseRoot
