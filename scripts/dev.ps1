$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'environment.ps1')
Push-Location $taskRoot
try { & npm.cmd run tauri -- dev; exit $LASTEXITCODE } finally { Pop-Location }
