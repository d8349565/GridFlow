$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$taskDirectory = Join-Path $taskRoot '.tools/duckdb'
New-Item -ItemType Directory -Path $taskDirectory -Force | Out-Null
$taskArchive = Join-Path $taskRoot '.tools/libduckdb-windows-amd64.zip'
$taskExpected = '44cf59583f9951d2cb09b1bf115a63ecb2d8901e363903029d86c7d8683fe96a'
if (!(Test-Path -LiteralPath $taskArchive)) {
    Invoke-WebRequest -Uri 'https://github.com/duckdb/duckdb/releases/download/v1.5.6/libduckdb-windows-amd64.zip' -OutFile $taskArchive
}
if ((Get-FileHash -LiteralPath $taskArchive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskExpected) { throw 'DuckDB archive checksum mismatch' }
Expand-Archive -LiteralPath $taskArchive -DestinationPath $taskDirectory -Force
Write-Output 'DuckDB 1.5.6 official Windows library ready.'
