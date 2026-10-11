$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path $PSScriptRoot -Parent
$taskReleaseRoot = Join-Path $taskRoot 'release'
New-Item -ItemType Directory -Path $taskReleaseRoot -Force | Out-Null
$taskInstaller = Join-Path $taskReleaseRoot 'MicrosoftEdgeWebview2RuntimeInstallerX64.exe'
Invoke-WebRequest -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124701' -OutFile $taskInstaller
if ((Get-Item -LiteralPath $taskInstaller).Length -lt 50MB) { throw 'Expected full offline installer, received a bootstrapper' }
$taskSignature = Get-AuthenticodeSignature -LiteralPath $taskInstaller
if ($taskSignature.Status -ne 'Valid' -or $taskSignature.SignerCertificate.Subject -notmatch 'Microsoft Corporation') { throw 'WebView2 installer Microsoft signature verification failed' }
Write-Output $taskInstaller
