param([Parameter(ValueFromRemainingArguments=$true)][string[]]$TaskArguments)
. (Join-Path $PSScriptRoot 'environment.ps1')
Push-Location $taskRoot
try { & cargo @TaskArguments; exit $LASTEXITCODE } finally { Pop-Location }
